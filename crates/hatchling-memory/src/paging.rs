//! One pager for every multi-item session reply.
//!
//! A reply that lists records — memory recall-by-tag, context entries,
//! the feed's goal/tracking/memory queries — carries each record as its
//! JSON re-encoded into a JSON string. [`page`] packs records into one
//! reply up to a byte budget counted at exactly that size
//! ([`reply_encoded_len`](crate::reply_encoded_len)) and reports the
//! cursor the caller resumes from.
//!
//! Budget arithmetic. A frame holds at most 64 KiB
//! (`hatchling-core`'s `MAX_FRAME_BYTES`). A page carries records worth
//! at most [`PAGE_BUDGET_BYTES`] = 48 KiB, each counted with its
//! separating comma — except that the first record always goes in, so a
//! page makes progress, and one record is bounded by
//! [`MAX_ENCODED_ITEM_BYTES`](crate::MAX_ENCODED_ITEM_BYTES) = 56 KiB
//! . The largest page is therefore at most 56 KiB of records
//! plus its envelope: the `kind`/`result` tags, the list brackets, and
//! a cursor of at most 128 id bytes (or a 20-digit sequence number) —
//! under 300 bytes. That is at least 7.7 KiB under the frame, so a paged
//! reply never needs the broker's oversize fallback.

use crate::{Error, reply_encoded_len};

/// Byte budget for the records of one page, counted at their
/// encoded-inside-the-reply size. See the module docs for why a page at
/// this budget, or a single record past it, always fits a frame.
pub const PAGE_BUDGET_BYTES: usize = 48 * 1024;

/// One page of a multi-item reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The records, each in its JSON form.
    pub items_json: Vec<String>,
    /// The cursor to resume from when records remain (the last
    /// included record's), `None` when this page ends the result.
    pub next: Option<String>,
}

/// Packs `records` — each its JSON and the cursor that resumes after it,
/// in order — into one page of at most `budget_bytes`.
///
/// The first record is always included, so a page always makes
/// progress. Records past the page are never pulled from the iterator,
/// so an expensive encoding is only done for records that ship.
///
/// # Errors
///
/// Propagates the first record error, or [`Error::Corrupt`] (converted
/// into `E`) for a record that is not UTF-8.
pub fn page<E>(
    records: impl IntoIterator<Item = Result<(String, String), E>>,
    budget_bytes: usize,
) -> Result<Page, E>
where
    E: From<Error>,
{
    let mut items_json = Vec::new();
    let mut used: usize = 0;
    let mut last_cursor: Option<String> = None;
    for record in records {
        let (json, cursor) = record?;
        let cost = reply_encoded_len(json.as_bytes())?.saturating_add(1);
        if last_cursor.is_some() && used.saturating_add(cost) > budget_bytes {
            return Ok(Page {
                items_json,
                next: last_cursor,
            });
        }
        used = used.saturating_add(cost);
        items_json.push(json);
        last_cursor = Some(cursor);
    }
    Ok(Page {
        items_json,
        next: None,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[allow(
        clippy::unnecessary_wraps,
        reason = "the pager takes fallible records; this builds the success case"
    )]
    fn record(json: &str, cursor: &str) -> Result<(String, String), Error> {
        Ok((json.to_owned(), cursor.to_owned()))
    }

    fn cost(json: &str) -> usize {
        reply_encoded_len(json.as_bytes()).unwrap() + 1
    }

    #[test]
    fn the_budget_is_exact_and_the_cursor_is_the_last_record_shipped() {
        let (a, b) = (r#"{"a":"x"}"#, r#"{"b":"\""}"#);
        let both = cost(a) + cost(b);
        let whole = page([record(a, "1"), record(b, "2")], both).unwrap();
        assert_eq!(whole.items_json, vec![a.to_owned(), b.to_owned()]);
        assert_eq!(whole.next, None);
        let split = page([record(a, "1"), record(b, "2")], both - 1).unwrap();
        assert_eq!(split.items_json, vec![a.to_owned()]);
        assert_eq!(split.next.as_deref(), Some("1"));
    }

    #[test]
    fn the_first_record_always_ships() {
        let big = format!(r#"{{"x":"{}"}}"#, "y".repeat(100));
        let one = page([record(&big, "7"), record("{}", "8")], 1).unwrap();
        assert_eq!(one.items_json, vec![big]);
        assert_eq!(one.next.as_deref(), Some("7"));
        let none: [Result<(String, String), Error>; 0] = [];
        assert_eq!(
            page(none, 1).unwrap(),
            Page {
                items_json: vec![],
                next: None
            }
        );
    }

    #[test]
    fn records_past_the_page_are_never_pulled() {
        let pulled = Cell::new(0);
        let records = (0..10).map(|n| {
            pulled.set(pulled.get() + 1);
            record(r#""0123456789""#, &n.to_string())
        });
        let one = page(records, cost(r#""0123456789""#)).unwrap();
        assert_eq!(one.items_json.len(), 1);
        // The second record was pulled to learn it does not fit; the
        // rest never were.
        assert_eq!(pulled.get(), 2);
    }

    #[test]
    fn a_record_error_propagates() {
        let failed = page([record("{}", "1"), Err(Error::NotFound)], 1024);
        assert!(matches!(failed, Err(Error::NotFound)));
        let first = page([Err(Error::Corrupt), record("{}", "1")], 1024);
        assert!(matches!(first, Err(Error::Corrupt)));
    }

    #[test]
    fn the_shipped_budget_is_48_kib() {
        assert_eq!(PAGE_BUDGET_BYTES, 49_152);
    }
}
