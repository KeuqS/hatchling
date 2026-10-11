//! Five-field cron expressions, evaluated in an explicit timezone.
//!
//! `minute hour day-of-month month day-of-week`, with the classic
//! forms per field: `*`, a value, a range `a-b`, a step `*/n`, `a-b/n`
//! or `a/n` (from `a` to the field's end), and comma lists of those.
//! Day-of-week is `0`–`7`, where both `0` and `7` are Sunday. Names
//! (`MON`, `JAN`) are not accepted in v0.1.
//!
//! **Day matching** follows the classic rule. When both day-of-month
//! and day-of-week are restricted (neither starts with `*`), a day
//! matches if *either* does. Otherwise both must match, and the `*`
//! field matches everything.
//!
//! **Local time is the schedule's time**, so daylight-saving changes
//! are handled in civil terms:
//!
//! - A local minute that does not exist (inside a spring-forward gap)
//!   is skipped.
//! - A local minute that happens twice (inside a fall-back fold) fires
//!   once, at its first occurrence.
//!
//! Every civil minute fires at most once.

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::{AmbiguousOffset, TimeZone};

/// How far ahead [`CronExpr::next_after`] looks: eight years and a day.
///
/// That is enough for a leap-day-only expression across a skipped
/// century leap year (2096 to 2104). An expression that finds no fire in
/// that window never fires.
pub const MAX_SEARCH_DAYS: u32 = 366 * 8 + 1;

/// Why an expression does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CronError {
    /// Not exactly five whitespace-separated fields.
    #[error("expected 5 fields, found {0}")]
    FieldCount(usize),
    /// A field is malformed or out of range.
    #[error("field {field} ({name}): {reason}")]
    Field {
        /// Which field, 1-based.
        field: usize,
        /// The field's name.
        name: &'static str,
        /// What is wrong.
        reason: String,
    },
}

/// The allowed values of one field, as a bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field(u64);

impl Field {
    fn contains(self, value: u8) -> bool {
        self.0
            .checked_shr(u32::from(value))
            .is_some_and(|bits| bits & 1 == 1)
    }

    fn values(self) -> impl Iterator<Item = u8> {
        (0u8..64).filter(move |value| self.contains(*value))
    }
}

/// A parsed cron expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    minute: Field,
    hour: Field,
    day_of_month: Field,
    month: Field,
    day_of_week: Field,
    /// Whether the day-of-month field starts with `*`.
    any_day_of_month: bool,
    /// Whether the day-of-week field starts with `*`.
    any_day_of_week: bool,
}

/// Each field's name and inclusive range.
const FIELDS: [(&str, u8, u8); 5] = [
    ("minute", 0, 59),
    ("hour", 0, 23),
    ("day-of-month", 1, 31),
    ("month", 1, 12),
    ("day-of-week", 0, 7),
];

impl CronExpr {
    /// Parses a five-field expression.
    ///
    /// # Errors
    ///
    /// Returns [`CronError`] when the expression is malformed.
    pub fn parse(expr: &str) -> Result<Self, CronError> {
        let raw: Vec<&str> = expr.split_whitespace().collect();
        let [minute, hour, day_of_month, month, day_of_week] = raw.as_slice() else {
            return Err(CronError::FieldCount(raw.len()));
        };
        let parsed = [minute, hour, day_of_month, month, day_of_week]
            .into_iter()
            .zip(FIELDS)
            .enumerate()
            .map(|(index, (raw, (name, min, max)))| {
                parse_field(raw, min, max).map_err(|reason| CronError::Field {
                    field: index + 1,
                    name,
                    reason,
                })
            })
            .collect::<Result<Vec<Field>, CronError>>()?;
        let [minutes, hours, month_days, months, week_days] = parsed.as_slice() else {
            return Err(CronError::FieldCount(parsed.len()));
        };
        // Sunday is both 0 and 7: fold 7 into 0.
        let week_days = if week_days.contains(7) {
            Field((week_days.0 | 1) & !(1 << 7))
        } else {
            *week_days
        };
        Ok(Self {
            minute: *minutes,
            hour: *hours,
            day_of_month: *month_days,
            month: *months,
            day_of_week: week_days,
            any_day_of_month: day_of_month.starts_with('*'),
            any_day_of_week: day_of_week.starts_with('*'),
        })
    }

    /// Whether `date` is a day this expression fires on.
    #[must_use]
    pub fn matches_day(&self, date: Date) -> bool {
        let Ok(month) = u8::try_from(date.month()) else {
            return false;
        };
        let (Ok(day), Ok(weekday)) = (
            u8::try_from(date.day()),
            u8::try_from(date.weekday().to_sunday_zero_offset()),
        ) else {
            return false;
        };
        if !self.month.contains(month) {
            return false;
        }
        let by_date = self.day_of_month.contains(day);
        let by_weekday = self.day_of_week.contains(weekday);
        if self.any_day_of_month || self.any_day_of_week {
            by_date && by_weekday
        } else {
            by_date || by_weekday
        }
    }

    /// The first fire strictly after `after`, in `tz`, or `None` if
    /// there is none within [`MAX_SEARCH_DAYS`].
    #[must_use]
    pub fn next_after(&self, after: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
        let start = tz.to_datetime(after);
        let mut date = start.date();
        for _ in 0..MAX_SEARCH_DAYS {
            if self.matches_day(date) {
                for hour in self.hour.values() {
                    for minute in self.minute.values() {
                        let (Ok(hour), Ok(minute)) = (i8::try_from(hour), i8::try_from(minute))
                        else {
                            continue;
                        };
                        let candidate = date.at(hour, minute, 0, 0);
                        if let Some(fire) = resolve(tz, candidate) {
                            if fire > after {
                                return Some(fire);
                            }
                        }
                    }
                }
            }
            date = date.tomorrow().ok()?;
        }
        None
    }
}

/// The instant a local time names: the first occurrence in a fold, and
/// none in a gap.
#[must_use]
pub fn resolve(tz: &TimeZone, local: DateTime) -> Option<Timestamp> {
    let ambiguous = tz.to_ambiguous_timestamp(local);
    match ambiguous.offset() {
        AmbiguousOffset::Gap { .. } => None,
        AmbiguousOffset::Unambiguous { .. } | AmbiguousOffset::Fold { .. } => {
            ambiguous.earlier().ok()
        }
    }
}

fn parse_field(raw: &str, min: u8, max: u8) -> Result<Field, String> {
    let mut bits = 0u64;
    for part in raw.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step: u8 = step.parse().map_err(|_| format!("bad step {step:?}"))?;
                if step == 0 {
                    return Err(String::from("step must be at least 1"));
                }
                (range, Some(step))
            }
            None => (part, None),
        };
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((lo, hi)) = range.split_once('-') {
            (value(lo, min, max)?, value(hi, min, max)?)
        } else {
            let at = value(range, min, max)?;
            // `a/n` runs from `a` to the field's end.
            (at, if step.is_some() { max } else { at })
        };
        if lo > hi {
            return Err(format!("range {lo}-{hi} runs backwards"));
        }
        for v in (lo..=hi).step_by(usize::from(step.unwrap_or(1))) {
            bits |= 1 << v;
        }
    }
    Ok(Field(bits))
}

fn value(raw: &str, min: u8, max: u8) -> Result<u8, String> {
    let v: u8 = raw.parse().map_err(|_| format!("bad value {raw:?}"))?;
    if v < min || v > max {
        return Err(format!("{v} is outside {min}-{max}"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn tz(name: &str) -> TimeZone {
        jiff::tz::db().get(name).unwrap()
    }

    fn at(tz: &TimeZone, y: i16, mo: i8, d: i8, h: i8, mi: i8) -> Timestamp {
        resolve(tz, date(y, mo, d).at(h, mi, 0, 0)).unwrap()
    }

    fn local(tz: &TimeZone, ts: Timestamp) -> String {
        tz.to_datetime(ts).to_string()
    }

    fn next(expr: &str, zone: &str, after: (i16, i8, i8, i8, i8)) -> String {
        let tz = tz(zone);
        let cron = CronExpr::parse(expr).unwrap();
        let (y, mo, d, h, mi) = after;
        cron.next_after(at(&tz, y, mo, d, h, mi), &tz)
            .map_or_else(|| String::from("never"), |ts| local(&tz, ts))
    }

    #[test]
    fn every_minute_is_the_next_minute() {
        assert_eq!(
            next("* * * * *", "UTC", (2026, 1, 1, 0, 0)),
            "2026-01-01T00:01:00"
        );
        assert_eq!(
            next("* * * * *", "UTC", (2026, 1, 1, 23, 59)),
            "2026-01-02T00:00:00"
        );
    }

    #[test]
    fn fields_restrict_the_minute_hour_day_and_month() {
        assert_eq!(
            next("30 9 * * *", "UTC", (2026, 1, 1, 9, 30)),
            "2026-01-02T09:30:00"
        );
        assert_eq!(
            next("30 9 * * *", "UTC", (2026, 1, 1, 9, 29)),
            "2026-01-01T09:30:00"
        );
        assert_eq!(
            next("0 0 1 * *", "UTC", (2026, 1, 15, 0, 0)),
            "2026-02-01T00:00:00"
        );
        assert_eq!(
            next("0 0 1 6 *", "UTC", (2026, 1, 15, 0, 0)),
            "2026-06-01T00:00:00"
        );
        assert_eq!(
            next("15 */6 * * *", "UTC", (2026, 1, 1, 6, 15)),
            "2026-01-01T12:15:00"
        );
        assert_eq!(
            next("0 9-17/4 * * *", "UTC", (2026, 1, 1, 9, 0)),
            "2026-01-01T13:00:00"
        );
        assert_eq!(
            next("0 22/1 * * *", "UTC", (2026, 1, 1, 22, 0)),
            "2026-01-01T23:00:00"
        );
        assert_eq!(
            next("5,10,45 * * * *", "UTC", (2026, 1, 1, 0, 10)),
            "2026-01-01T00:45:00"
        );
    }

    #[test]
    fn day_of_week_counts_sunday_as_0_and_7() {
        // 2026-01-04 is a Sunday.
        assert_eq!(
            next("0 12 * * 0", "UTC", (2026, 1, 1, 0, 0)),
            "2026-01-04T12:00:00"
        );
        assert_eq!(
            next("0 12 * * 7", "UTC", (2026, 1, 1, 0, 0)),
            "2026-01-04T12:00:00"
        );
        assert_eq!(
            next("0 12 * * 1-5", "UTC", (2026, 1, 2, 12, 0)),
            "2026-01-05T12:00:00"
        );
        assert_eq!(
            next("0 12 * * 6", "UTC", (2026, 1, 1, 0, 0)),
            "2026-01-03T12:00:00"
        );
    }

    #[test]
    fn restricted_day_fields_match_either() {
        // The 13th, or any Friday: 2026-01-02 is a Friday.
        assert_eq!(
            next("0 0 13 * 5", "UTC", (2026, 1, 1, 0, 0)),
            "2026-01-02T00:00:00"
        );
        assert_eq!(
            next("0 0 13 * 5", "UTC", (2026, 1, 9, 0, 0)),
            "2026-01-13T00:00:00"
        );
        // With a `*` in one, the other governs alone.
        assert_eq!(
            next("0 0 13 * *", "UTC", (2026, 1, 1, 0, 0)),
            "2026-01-13T00:00:00"
        );
        assert_eq!(
            next("0 0 * * 5", "UTC", (2026, 1, 3, 0, 0)),
            "2026-01-09T00:00:00"
        );
        // `*/2` starts with `*`, so both must match: an odd day that is
        // a Friday (2026-01-02 is even, 2026-01-09 is odd).
        assert_eq!(
            next("0 0 */2 * 5", "UTC", (2026, 1, 2, 0, 0)),
            "2026-01-09T00:00:00"
        );
    }

    #[test]
    fn leap_days_and_never() {
        assert_eq!(
            next("0 0 29 2 *", "UTC", (2026, 1, 1, 0, 0)),
            "2028-02-29T00:00:00"
        );
        // Across the skipped century leap year.
        assert_eq!(
            next("0 0 29 2 *", "UTC", (2096, 3, 1, 0, 0)),
            "2104-02-29T00:00:00"
        );
        assert_eq!(next("0 0 31 2 *", "UTC", (2026, 1, 1, 0, 0)), "never");
        assert_eq!(next("0 0 30 2 *", "UTC", (2026, 1, 1, 0, 0)), "never");
        assert_eq!(MAX_SEARCH_DAYS, 2929);
    }

    #[test]
    fn the_timezone_is_the_schedule_s() {
        let tz = tz("Asia/Tokyo");
        let cron = CronExpr::parse("0 9 * * *").unwrap();
        let fire = cron
            .next_after(Timestamp::from_second(1_790_000_000).unwrap(), &tz)
            .unwrap();
        assert_eq!(local(&tz, fire), "2026-09-22T09:00:00");
        assert_eq!(fire.to_string(), "2026-09-22T00:00:00Z");
    }

    #[test]
    fn a_minute_inside_a_spring_forward_gap_is_skipped() {
        // New York jumps from 02:00 to 03:00 on 2026-03-08.
        assert_eq!(
            next("30 2 * * *", "America/New_York", (2026, 3, 7, 3, 0)),
            "2026-03-09T02:30:00"
        );
        assert_eq!(
            next("*/30 * * * *", "America/New_York", (2026, 3, 8, 1, 30)),
            "2026-03-08T03:00:00"
        );
    }

    #[test]
    fn a_minute_inside_a_fall_back_fold_fires_once() {
        // New York repeats 01:00-01:59 on 2026-11-01.
        let tz = tz("America/New_York");
        let cron = CronExpr::parse("30 1 * * *").unwrap();
        let first = cron.next_after(at(&tz, 2026, 11, 1, 0, 0), &tz).unwrap();
        assert_eq!(first.to_string(), "2026-11-01T05:30:00Z", "the EDT 01:30");
        let then = cron.next_after(first, &tz).unwrap();
        assert_eq!(local(&tz, then), "2026-11-02T01:30:00", "not the EST 01:30");
    }

    #[test]
    fn malformed_expressions_are_refused() {
        for (expr, bad) in [
            ("* * * *", "expected 5"),
            ("* * * * * *", "expected 5"),
            ("60 * * * *", "outside 0-59"),
            ("* 24 * * *", "outside 0-23"),
            ("* * 0 * *", "outside 1-31"),
            ("* * 32 * *", "outside 1-31"),
            ("* * * 0 *", "outside 1-12"),
            ("* * * 13 *", "outside 1-12"),
            ("* * * * 8", "outside 0-7"),
            ("*/0 * * * *", "at least 1"),
            ("*/x * * * *", "bad step"),
            ("5-2 * * * *", "backwards"),
            ("a * * * *", "bad value"),
            ("1,,2 * * * *", "bad value"),
            ("* * * JAN *", "bad value"),
        ] {
            let error = CronExpr::parse(expr).unwrap_err().to_string();
            assert!(error.contains(bad), "{expr:?}: {error}");
        }
        let error = CronExpr::parse("* 99 * * *").unwrap_err();
        assert_eq!(
            error,
            CronError::Field {
                field: 2,
                name: "hour",
                reason: String::from("99 is outside 0-23"),
            }
        );
    }

    #[test]
    fn range_bounds_are_inclusive() {
        let cron = CronExpr::parse("0-59 0-23 1-31 1-12 0-7").unwrap();
        assert_eq!(cron.minute.values().count(), 60);
        assert_eq!(cron.hour.values().count(), 24);
        assert_eq!(cron.day_of_month.values().count(), 31);
        assert_eq!(cron.month.values().count(), 12);
        assert_eq!(
            cron.day_of_week.values().collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5, 6]
        );
        assert!(!cron.any_day_of_month && !cron.any_day_of_week);
        let star = CronExpr::parse("* * */1 * */2").unwrap();
        assert!(star.any_day_of_month && star.any_day_of_week);
    }

    #[test]
    fn a_field_contains_exactly_its_values() {
        let field = parse_field("0,5,59", 0, 59).unwrap();
        assert!(field.contains(0) && field.contains(5) && field.contains(59));
        assert!(!field.contains(1) && !field.contains(58));
        assert!(!field.contains(64), "past the mask is never a member");
        assert!(!field.contains(u8::MAX));
    }

    #[test]
    fn resolve_names_the_first_occurrence_and_no_gap() {
        let tz = tz("America/New_York");
        assert_eq!(resolve(&tz, date(2026, 3, 8).at(2, 30, 0, 0)), None);
        assert_eq!(
            resolve(&tz, date(2026, 11, 1).at(1, 30, 0, 0)).map(|t| t.to_string()),
            Some(String::from("2026-11-01T05:30:00Z"))
        );
        assert_eq!(
            resolve(&tz, date(2026, 6, 1).at(9, 0, 0, 0)).map(|t| t.to_string()),
            Some(String::from("2026-06-01T13:00:00Z"))
        );
    }
}
