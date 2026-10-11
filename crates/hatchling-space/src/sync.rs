//! Sync: explicit pulls and pushes of one space's items.
//!
//! **Only the named space moves.** A pull returns that space's items
//! changed after a version; a push writes only into that space. The
//! broker checks the endpoint is a member of *this* space before either,
//! as for every space op.
//!
//! **Conflicts resolve last-writer-wins, and nothing is lost silently.**
//! Each change names the version of the item its writer last saw
//! (`base`). If the item has moved on since, someone else wrote first:
//! the push still wins (it is the last writer), but the value it
//! overwrites goes to the space's conflict log, with who wrote it and who
//! overwrote it. A push is checked whole before any of it is applied, so
//! a refused push changes nothing.
//!
//! **Replies page.** A pull, and a read of the conflict log, return what
//! fits [`PAGE_BYTES`] ([`page`]) and say whether there is more.

use std::collections::BTreeSet;

use hatchling_core::{SpaceRefusal, SyncChange};

use crate::record::{ItemMeta, MAX_ITEMS, SpaceRecord, check_key, check_value};

/// Most changes one push may carry.
pub const MAX_CHANGES: usize = 64;

/// The most bytes of entries one paged reply carries, leaving room in its
/// frame for the rest of the reply.
pub const PAGE_BYTES: usize = 48 * 1024;

/// The first of `entries` whose encodings together fit `budget` bytes,
/// and whether any were left out. The first entry is always taken, so a
/// page is never empty while there is something to page.
#[must_use]
pub fn page<T: serde::Serialize>(entries: Vec<T>, budget: usize) -> (Vec<T>, bool) {
    let total = entries.len();
    let mut used = 0usize;
    let taken: Vec<T> = entries
        .into_iter()
        .enumerate()
        .take_while(|(at, entry)| {
            used = used.saturating_add(serde_json::to_vec(entry).map_or(usize::MAX, |b| b.len()));
            *at == 0 || used <= budget
        })
        .map(|(_, entry)| entry)
        .collect();
    let more = taken.len() < total;
    (taken, more)
}

/// Checks a whole push against `record` before any of it is applied:
/// its size, every key and value, and room for the new keys.
///
/// # Errors
///
/// [`SpaceRefusal::TooManyChanges`] past [`MAX_CHANGES`];
/// [`SpaceRefusal::BadKey`] or [`SpaceRefusal::BadValue`] for the first
/// bad change; [`SpaceRefusal::Full`] if its new keys would not fit.
pub fn check_push(record: &SpaceRecord, changes: &[SyncChange]) -> Result<(), SpaceRefusal> {
    if changes.len() > MAX_CHANGES {
        return Err(SpaceRefusal::TooManyChanges);
    }
    let mut new_keys = BTreeSet::new();
    for change in changes {
        check_key(&change.key)?;
        if let Some(value) = &change.value {
            check_value(value)?;
        }
        if record.item(&change.key).is_none() {
            new_keys.insert(change.key.as_str());
        }
    }
    if record.items.len().saturating_add(new_keys.len()) > MAX_ITEMS {
        return Err(SpaceRefusal::Full);
    }
    Ok(())
}

/// The write `change` would overwrite unseen, if any: the item's current
/// record, when it exists and is not the version the writer last saw.
#[must_use]
pub fn overwritten<'a>(record: &'a SpaceRecord, change: &SyncChange) -> Option<&'a ItemMeta> {
    record
        .item(&change.key)
        .filter(|current| current.version != change.base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Principal;

    fn space() -> SpaceRecord {
        SpaceRecord::create(
            String::from("spc-1"),
            "Shared",
            Principal::Agent { uid: 1000 },
            Vec::new(),
            &BTreeSet::new(),
            0,
        )
        .unwrap()
    }

    fn change(key: &str, value: Option<&str>, base: u64) -> SyncChange {
        SyncChange {
            key: key.to_owned(),
            value: value.map(str::to_owned),
            base,
        }
    }

    /// A JSON string of exactly `bytes` encoded bytes.
    fn sized(bytes: usize) -> String {
        "x".repeat(bytes - 2)
    }

    #[test]
    fn a_page_holds_exactly_what_fits_its_budget() {
        let entries = vec![sized(10), sized(20), sized(30)];
        // Exactly at the budget: all of it.
        assert_eq!(page(entries.clone(), 60), (entries.clone(), false));
        // A byte short: the last is left out, and said to be.
        assert_eq!(
            page(entries.clone(), 59),
            (vec![sized(10), sized(20)], true)
        );
        assert_eq!(
            page(entries.clone(), 30),
            (vec![sized(10), sized(20)], true)
        );
        assert_eq!(page(entries.clone(), 29), (vec![sized(10)], true));
        // The first is taken whatever its size.
        assert_eq!(page(entries, 1), (vec![sized(10)], true));
        assert_eq!(page(Vec::<String>::new(), 0), (Vec::new(), false));
        assert_eq!(PAGE_BYTES, 49_152);
    }

    #[test]
    fn a_push_is_checked_whole_before_anything_is_applied() {
        let record = space();
        assert_eq!(check_push(&record, &[]), Ok(()));
        assert_eq!(
            check_push(&record, &[change("a", Some("1"), 0), change("b", None, 0)]),
            Ok(())
        );
        // One bad change refuses the whole push.
        assert_eq!(
            check_push(
                &record,
                &[change("a", Some("1"), 0), change("B", Some("2"), 0)]
            ),
            Err(SpaceRefusal::BadKey)
        );
        let big = "v".repeat(crate::record::MAX_VALUE_BYTES + 1);
        assert_eq!(
            check_push(&record, &[change("a", Some(&big), 0)]),
            Err(SpaceRefusal::BadValue)
        );
        let at_bound: Vec<SyncChange> = (0..MAX_CHANGES)
            .map(|n| change(&format!("k{n}"), Some("v"), 0))
            .collect();
        assert_eq!(check_push(&record, &at_bound), Ok(()));
        let mut over = at_bound;
        over.push(change("one-more", Some("v"), 0));
        assert_eq!(
            check_push(&record, &over),
            Err(SpaceRefusal::TooManyChanges)
        );
        assert_eq!(MAX_CHANGES, 64);
    }

    #[test]
    fn a_push_needs_room_for_its_new_keys_only() {
        let mut record = space();
        for n in 0..MAX_ITEMS - 1 {
            record
                .write_item(&format!("k{n}"), "uid:1000", false)
                .unwrap();
        }
        // One new key fits; the same key twice is still one.
        assert_eq!(
            check_push(
                &record,
                &[change("new", Some("1"), 0), change("new", Some("2"), 0)]
            ),
            Ok(())
        );
        assert_eq!(
            check_push(
                &record,
                &[change("new", Some("1"), 0), change("new2", Some("2"), 0)]
            ),
            Err(SpaceRefusal::Full)
        );
        // Rewriting existing keys needs no room.
        record.write_item("new", "uid:1000", false).unwrap();
        assert_eq!(record.items.len(), MAX_ITEMS);
        assert_eq!(check_push(&record, &[change("k0", Some("x"), 1)]), Ok(()));
        assert_eq!(
            check_push(&record, &[change("fresh", Some("x"), 0)]),
            Err(SpaceRefusal::Full)
        );
    }

    #[test]
    fn a_write_over_a_version_its_writer_never_saw_is_a_conflict() {
        let mut record = space();
        // Nothing there yet: no conflict, whatever the base.
        assert!(overwritten(&record, &change("a", Some("1"), 0)).is_none());
        record.write_item("a", "uid:1000", false).unwrap();
        // The writer saw version 1: no conflict.
        assert!(overwritten(&record, &change("a", Some("2"), 1)).is_none());
        // Someone else wrote version 2 since.
        record.write_item("a", "device:dev-1", false).unwrap();
        let lost = overwritten(&record, &change("a", Some("3"), 1)).unwrap();
        assert_eq!((lost.version, lost.writer.as_str()), (2, "device:dev-1"));
        // A writer that never saw it at all, too.
        assert!(overwritten(&record, &change("a", Some("3"), 0)).is_some());
    }
}
