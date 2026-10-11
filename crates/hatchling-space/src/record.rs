//! Principals, space records, items, and tasks.
//!
//! **A principal is broker-asserted.** The agent's own session is
//! [`Principal::Agent`], keyed by the kernel uid of its peer. A space
//! task's run session is [`Principal::TaskRun`], bound by the broker to
//! the one space it is a task of. Neither comes from anything the cell
//! says.
//!
//! A paired device is [`Principal::Device`], by the id the broker
//! gave it at pairing, after it proved it holds its key.
//!
//! **A space's identity is broker-minted.** The creator supplies only a
//! display name; the broker assigns the id. The record keeps the owner,
//! the members, the grants the space's tasks may draw on, its items'
//! versions and writers (for sync), its tasks, its conflict log's index,
//! and its media, all bounded.

use std::collections::BTreeSet;

use hatchling_core::{MAX_SKILL_INPUT_BYTES, ScheduleGrant, ScheduleRefusal, SpaceRefusal};
use hatchling_scheduler::{MAX_GRANTS, Schedule, Timestamp, validate_space_task};
use serde::{Deserialize, Serialize};

/// Longest display name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// Longest item key, in bytes.
pub const MAX_KEY_BYTES: usize = 64;

/// Longest item value, in bytes.
pub const MAX_VALUE_BYTES: usize = MAX_SKILL_INPUT_BYTES;

/// Most items one space keeps.
pub const MAX_ITEMS: usize = 64;

/// Most tasks one space keeps.
pub const MAX_TASKS: usize = 16;

/// Most members one space keeps: its owner and paired devices.
pub const MAX_MEMBERS: usize = 17;

/// Most conflict-log entries one space keeps; the oldest go first, and
/// are counted.
pub const MAX_CONFLICTS: usize = 32;

/// Most media one space keeps.
pub const MAX_MEDIA: usize = 16;

/// Who is asking, as the broker asserts it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "principal")]
pub enum Principal {
    /// The agent's session, by its peer's kernel uid.
    #[serde(rename = "agent")]
    Agent {
        /// The kernel uid.
        uid: u32,
    },
    /// A space task's run session, bound to its space.
    #[serde(rename = "task-run")]
    TaskRun {
        /// The space the run is a task of.
        space: String,
    },
    /// A paired device's session.
    #[serde(rename = "device")]
    Device {
        /// The device id.
        id: String,
    },
}

impl Principal {
    /// How the broker names this principal as a writer.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Agent { uid } => format!("uid:{uid}"),
            Self::TaskRun { space } => format!("task:{space}"),
            Self::Device { id } => format!("device:{id}"),
        }
    }
}

/// One item of a space, as its record keeps it: never its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemMeta {
    /// The item's key.
    pub key: String,
    /// The space's change counter when it was last written.
    pub version: u64,
    /// Who last wrote it ([`Principal::label`]).
    pub writer: String,
    /// Whether its last write removed it: a tombstone, so a removal
    /// syncs too.
    pub removed: bool,
}

/// One task of a space: a work item a run does with a subset of the
/// space's grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceTask {
    /// The broker-minted task id.
    pub id: String,
    /// The space it is a task of.
    pub space: String,
    /// The task text the run is given.
    pub task: String,
    /// The grants the run may use.
    pub grants: Vec<ScheduleGrant>,
    /// Where the run's output goes.
    pub channel: String,
}

/// One space, as the broker keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceRecord {
    /// The broker-minted id.
    pub id: String,
    /// The display name the creator gave it.
    pub name: String,
    /// Who created it.
    pub owner: Principal,
    /// Who may join it. v0.1: the owner alone.
    pub members: Vec<Principal>,
    /// The grants its tasks may draw on: a subset of the owner's.
    pub grants: Vec<ScheduleGrant>,
    /// Its items' keys, versions, and writers, tombstones included, so
    /// their number is bounded. Values are stored apart.
    pub items: Vec<ItemMeta>,
    /// Its tasks' ids; each task is stored apart.
    pub tasks: Vec<String>,
    /// When it was created, in Unix seconds.
    pub created_at: i64,
    /// Its change counter: every write moves it on.
    #[serde(default)]
    pub head: u64,
    /// The sequence numbers of the conflict-log entries it keeps; each
    /// entry is stored apart.
    #[serde(default)]
    pub conflicts: Vec<u64>,
    /// The last conflict sequence number given out.
    #[serde(default)]
    pub conflict_seq: u64,
    /// How many conflict-log entries the bound has dropped.
    #[serde(default)]
    pub conflicts_dropped: u64,
    /// Its media's ids; each is stored apart.
    #[serde(default)]
    pub media: Vec<String>,
}

/// Refuses a blank or over-long display name, or one with control
/// characters.
///
/// # Errors
///
/// [`SpaceRefusal::BadName`].
pub fn check_name(name: &str) -> Result<(), SpaceRefusal> {
    if name.trim().is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(SpaceRefusal::BadName);
    }
    Ok(())
}

/// Refuses an item key that is not a lowercase token.
///
/// A token is a letter or digit, then letters, digits, `_`, `-`, at most
/// [`MAX_KEY_BYTES`]. A key is part of its item's store entry name, so it
/// uses only that alphabet.
///
/// # Errors
///
/// [`SpaceRefusal::BadKey`].
pub fn check_key(key: &str) -> Result<(), SpaceRefusal> {
    let mut chars = key.chars();
    let token = key.len() <= MAX_KEY_BYTES
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-'));
    if token {
        Ok(())
    } else {
        Err(SpaceRefusal::BadKey)
    }
}

/// Refuses an item value over [`MAX_VALUE_BYTES`].
///
/// # Errors
///
/// [`SpaceRefusal::BadValue`].
pub const fn check_value(value: &str) -> Result<(), SpaceRefusal> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(SpaceRefusal::BadValue);
    }
    Ok(())
}

/// A scheduler refusal of a task, as a space refusal.
const fn task_refusal(refusal: ScheduleRefusal) -> SpaceRefusal {
    match refusal {
        ScheduleRefusal::ExceedsGrants | ScheduleRefusal::ChannelNotGranted => {
            SpaceRefusal::ExceedsGrants
        }
        ScheduleRefusal::BadTask
        | ScheduleRefusal::TooManyGrants
        | ScheduleRefusal::MissingTimezone
        | ScheduleRefusal::UnknownTimezone
        | ScheduleRefusal::InvalidSpec
        | ScheduleRefusal::NotInFuture
        | ScheduleRefusal::Full
        | ScheduleRefusal::UnknownSource
        | ScheduleRefusal::MaxFollowupDepth
        | ScheduleRefusal::FollowupTaken
        | ScheduleRefusal::BadDelay => SpaceRefusal::BadTask,
    }
}

impl SpaceRecord {
    /// A new space `id` named `name`, owned by `owner`, holding `grants`,
    /// which must all be in `held`.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::BadName`] for a bad name;
    /// [`SpaceRefusal::ExceedsGrants`] for a grant `held` lacks or too
    /// many grants.
    pub fn create(
        id: String,
        name: &str,
        owner: Principal,
        grants: Vec<ScheduleGrant>,
        held: &BTreeSet<ScheduleGrant>,
        now: i64,
    ) -> Result<Self, SpaceRefusal> {
        check_name(name)?;
        if grants.len() > MAX_GRANTS || grants.iter().any(|grant| !held.contains(grant)) {
            return Err(SpaceRefusal::ExceedsGrants);
        }
        Ok(Self {
            id,
            name: name.to_owned(),
            members: vec![owner.clone()],
            owner,
            grants,
            items: Vec::new(),
            tasks: Vec::new(),
            created_at: now,
            head: 0,
            conflicts: Vec::new(),
            conflict_seq: 0,
            conflicts_dropped: 0,
            media: Vec::new(),
        })
    }

    /// Whether `principal` may use this space: an agent or a device that
    /// is a member, or a task run bound to exactly this space.
    #[must_use]
    pub fn admits(&self, principal: &Principal) -> bool {
        match principal {
            Principal::Agent { .. } | Principal::Device { .. } => self.members.contains(principal),
            Principal::TaskRun { space } => *space == self.id,
        }
    }

    /// Adds `member`, if it is not one already.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::Full`] at [`MAX_MEMBERS`].
    pub fn add_member(&mut self, member: Principal) -> Result<(), SpaceRefusal> {
        if self.members.contains(&member) {
            return Ok(());
        }
        if self.members.len() >= MAX_MEMBERS {
            return Err(SpaceRefusal::Full);
        }
        self.members.push(member);
        Ok(())
    }

    /// The item `key`'s record, tombstone or not.
    #[must_use]
    pub fn item(&self, key: &str) -> Option<&ItemMeta> {
        self.items.iter().find(|item| item.key == key)
    }

    /// Whether the space has a live item `key`.
    #[must_use]
    pub fn has_item(&self, key: &str) -> bool {
        self.item(key).is_some_and(|item| !item.removed)
    }

    /// Records a write of `key` by `writer`, a removal when `removed`: the
    /// change counter moves on, and the item takes its value. Returns the
    /// item's new version.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::Full`] for a new key when the space already has
    /// [`MAX_ITEMS`] (tombstones count).
    pub fn write_item(
        &mut self,
        key: &str,
        writer: &str,
        removed: bool,
    ) -> Result<u64, SpaceRefusal> {
        let known = self.items.iter().any(|item| item.key == key);
        if !known && self.items.len() >= MAX_ITEMS {
            return Err(SpaceRefusal::Full);
        }
        self.head = self.head.saturating_add(1);
        let meta = ItemMeta {
            key: key.to_owned(),
            version: self.head,
            writer: writer.to_owned(),
            removed,
        };
        match self.items.iter_mut().find(|item| item.key == key) {
            Some(item) => *item = meta,
            None => self.items.push(meta),
        }
        Ok(self.head)
    }

    /// The items written after version `since`, oldest first.
    #[must_use]
    pub fn changes_since(&self, since: u64) -> Vec<&ItemMeta> {
        let mut changed: Vec<&ItemMeta> = self
            .items
            .iter()
            .filter(|item| item.version > since)
            .collect();
        changed.sort_by_key(|item| item.version);
        changed
    }

    /// Takes the next conflict sequence number and keeps it. Returns it,
    /// and the oldest one dropped to stay within [`MAX_CONFLICTS`], if any.
    pub fn log_conflict(&mut self) -> (u64, Option<u64>) {
        self.conflict_seq = self.conflict_seq.saturating_add(1);
        self.conflicts.push(self.conflict_seq);
        let dropped = if self.conflicts.len() > MAX_CONFLICTS {
            self.conflicts_dropped = self.conflicts_dropped.saturating_add(1);
            Some(self.conflicts.remove(0))
        } else {
            None
        };
        (self.conflict_seq, dropped)
    }

    /// The conflict sequence numbers after `after`, oldest first.
    #[must_use]
    pub fn conflicts_after(&self, after: u64) -> Vec<u64> {
        self.conflicts
            .iter()
            .copied()
            .filter(|seq| *seq > after)
            .collect()
    }

    /// Notes the media `id`.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::Full`] at [`MAX_MEDIA`].
    pub fn add_media(&mut self, id: &str) -> Result<(), SpaceRefusal> {
        if self.media.len() >= MAX_MEDIA {
            return Err(SpaceRefusal::Full);
        }
        self.media.push(id.to_owned());
        Ok(())
    }

    /// Whether the space has the media `id`.
    #[must_use]
    pub fn has_media(&self, id: &str) -> bool {
        self.media.iter().any(|held| held == id)
    }

    /// The space's grants, as a set.
    #[must_use]
    pub fn grant_set(&self) -> BTreeSet<ScheduleGrant> {
        self.grants.iter().cloned().collect()
    }

    /// `task`, checked as its run will be checked: it is this space's,
    /// its text and grant count are in bounds, and every grant is one the
    /// space holds, including sending on its channel.
    fn validate_task(
        &self,
        task: &SpaceTask,
        schedule_id: String,
        now: Timestamp,
    ) -> Result<Schedule, SpaceRefusal> {
        if task.space != self.id {
            return Err(SpaceRefusal::NoSuchTask);
        }
        validate_space_task(
            schedule_id,
            &self.id,
            task.task.clone(),
            task.grants.clone(),
            task.channel.clone(),
            &self.grant_set(),
            now,
        )
        .map_err(task_refusal)
    }

    /// Registers `task` with the space, after checking it as its run will
    /// be checked. The task itself is stored apart; the space keeps its id.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::Full`] at [`MAX_TASKS`];
    /// [`SpaceRefusal::NoSuchTask`] for another space's task;
    /// [`SpaceRefusal::BadTask`] or [`SpaceRefusal::ExceedsGrants`] as
    /// the run's own validation would refuse it.
    pub fn register_task(&mut self, task: &SpaceTask, now: Timestamp) -> Result<(), SpaceRefusal> {
        if self.tasks.len() >= MAX_TASKS {
            return Err(SpaceRefusal::Full);
        }
        self.validate_task(task, String::from("sch-check"), now)?;
        self.tasks.push(task.id.clone());
        Ok(())
    }

    /// Whether the space has the task `id`.
    #[must_use]
    pub fn has_task(&self, id: &str) -> bool {
        self.tasks.iter().any(|held| held == id)
    }

    /// The one-shot schedule `schedule_id` that runs `task`, bound to this
    /// space, validated again against the space's grants at `now`.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::NoSuchTask`] unless `task` is one of this space's;
    /// [`SpaceRefusal::BadTask`] or [`SpaceRefusal::ExceedsGrants`].
    pub fn schedule_task(
        &self,
        task: &SpaceTask,
        schedule_id: String,
        now: Timestamp,
    ) -> Result<Schedule, SpaceRefusal> {
        if !self.has_task(&task.id) {
            return Err(SpaceRefusal::NoSuchTask);
        }
        self.validate_task(task, schedule_id, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send(channel: &str) -> ScheduleGrant {
        ScheduleGrant::ChannelSend {
            channel: channel.to_owned(),
        }
    }

    fn vault(name: &str) -> ScheduleGrant {
        ScheduleGrant::Vault {
            action: String::from("fetch"),
            name: name.to_owned(),
        }
    }

    const fn agent(uid: u32) -> Principal {
        Principal::Agent { uid }
    }

    fn held() -> BTreeSet<ScheduleGrant> {
        [send("local"), vault("token"), vault("other")]
            .into_iter()
            .collect()
    }

    fn space() -> SpaceRecord {
        SpaceRecord::create(
            String::from("spc-1"),
            "Garden",
            agent(1000),
            vec![send("local"), vault("token")],
            &held(),
            7,
        )
        .unwrap()
    }

    fn now() -> Timestamp {
        Timestamp::from_second(1_790_000_000).unwrap()
    }

    fn task(id: &str, grants: Vec<ScheduleGrant>) -> SpaceTask {
        SpaceTask {
            id: id.to_owned(),
            space: String::from("spc-1"),
            task: String::from("water the plants"),
            grants,
            channel: String::from("local"),
        }
    }

    #[test]
    fn a_space_is_created_with_its_owner_as_its_only_member() {
        let record = space();
        assert_eq!(record.id, "spc-1");
        assert_eq!(record.name, "Garden");
        assert_eq!(record.owner, agent(1000));
        assert_eq!(record.members, [agent(1000)]);
        assert_eq!(record.grants, [send("local"), vault("token")]);
        assert!(record.items.is_empty() && record.tasks.is_empty());
        assert_eq!(record.created_at, 7);
        // Grants the creator does not hold, or too many, are refused.
        for grants in [
            vec![vault("elsewhere")],
            vec![send("local"); MAX_GRANTS + 1],
        ] {
            assert_eq!(
                SpaceRecord::create(String::from("s"), "x", agent(1), grants, &held(), 0),
                Err(SpaceRefusal::ExceedsGrants)
            );
        }
        let at_bound = vec![send("local"); MAX_GRANTS];
        assert!(
            SpaceRecord::create(String::from("s"), "x", agent(1), at_bound, &held(), 0).is_ok()
        );
        assert_eq!(
            SpaceRecord::create(String::from("s"), " ", agent(1), Vec::new(), &held(), 0),
            Err(SpaceRefusal::BadName)
        );
    }

    #[test]
    fn only_a_member_or_a_task_run_of_this_space_is_admitted() {
        let record = space();
        assert!(record.admits(&agent(1000)));
        assert!(!record.admits(&agent(1001)), "another caller");
        assert!(record.admits(&Principal::TaskRun {
            space: String::from("spc-1")
        }));
        assert!(!record.admits(&Principal::TaskRun {
            space: String::from("spc-2")
        }));
    }

    #[test]
    fn names_keys_and_values_are_bounded() {
        assert_eq!(check_name("Garden plot 3"), Ok(()));
        assert_eq!(check_name(&"é".repeat(MAX_NAME_CHARS)), Ok(()));
        for bad in [
            String::new(),
            String::from(" \t"),
            "x".repeat(MAX_NAME_CHARS + 1),
            String::from("a\u{7}b"),
            String::from("a\nb"),
        ] {
            assert_eq!(check_name(&bad), Err(SpaceRefusal::BadName), "{bad:?}");
        }
        for good in ["plan", "0", "notes-v2_draft-1", &"k".repeat(MAX_KEY_BYTES)] {
            assert_eq!(check_key(good), Ok(()), "{good}");
        }
        for bad in [
            "",
            "Plan",
            ".hidden",
            "-x",
            "a b",
            "a/b",
            "a.b",
            "é",
            &"k".repeat(MAX_KEY_BYTES + 1),
        ] {
            assert_eq!(check_key(bad), Err(SpaceRefusal::BadKey), "{bad:?}");
        }
        assert_eq!(check_value(&"v".repeat(MAX_VALUE_BYTES)), Ok(()));
        assert_eq!(
            check_value(&"v".repeat(MAX_VALUE_BYTES + 1)),
            Err(SpaceRefusal::BadValue)
        );
        assert_eq!(
            (
                MAX_NAME_CHARS,
                MAX_KEY_BYTES,
                MAX_VALUE_BYTES,
                MAX_ITEMS,
                MAX_TASKS
            ),
            (64, 64, 8192, 64, 16)
        );
    }

    #[test]
    fn every_write_moves_the_change_counter_and_tombstones_count() {
        let mut record = space();
        assert_eq!(record.head, 0);
        assert!(!record.has_item("a") && record.item("a").is_none());
        assert_eq!(record.write_item("a", "uid:1000", false), Ok(1));
        assert_eq!(record.write_item("a", "device:dev-1", false), Ok(2));
        assert_eq!(
            record.items,
            [ItemMeta {
                key: String::from("a"),
                version: 2,
                writer: String::from("device:dev-1"),
                removed: false,
            }],
            "rewriting is no new item"
        );
        assert!(record.has_item("a"));
        // A removal is a tombstone: it syncs, and it still counts.
        assert_eq!(record.write_item("a", "uid:1000", true), Ok(3));
        assert!(!record.has_item("a"));
        assert_eq!(record.item("a").map(|item| item.removed), Some(true));
        assert_eq!(record.head, 3);
        for n in 1..MAX_ITEMS {
            let key = format!("k{n}");
            assert!(record.write_item(&key, "uid:1000", false).is_ok());
        }
        assert_eq!(record.items.len(), MAX_ITEMS);
        assert_eq!(
            record.write_item("one-more", "uid:1000", false),
            Err(SpaceRefusal::Full)
        );
        let head = record.head;
        assert!(
            record.write_item("a", "uid:1000", false).is_ok(),
            "an existing key is fine"
        );
        assert_eq!(record.head, head + 1, "a refused write moved nothing");
    }

    #[test]
    fn changes_since_a_version_come_back_oldest_first() {
        let mut record = space();
        for key in ["a", "b", "c"] {
            record.write_item(key, "uid:1000", false).unwrap();
        }
        record.write_item("a", "uid:1000", false).unwrap();
        let keys = |since: u64| -> Vec<(String, u64)> {
            record
                .changes_since(since)
                .into_iter()
                .map(|item| (item.key.clone(), item.version))
                .collect()
        };
        assert_eq!(
            keys(0),
            [
                (String::from("b"), 2),
                (String::from("c"), 3),
                (String::from("a"), 4)
            ]
        );
        assert_eq!(keys(2), [(String::from("c"), 3), (String::from("a"), 4)]);
        assert!(keys(4).is_empty());
    }

    #[test]
    fn the_conflict_log_is_bounded_and_counts_what_it_drops() {
        let mut record = space();
        assert_eq!(record.log_conflict(), (1, None));
        assert_eq!(record.log_conflict(), (2, None));
        assert_eq!(record.conflicts_after(0), [1, 2]);
        assert_eq!(record.conflicts_after(1), [2]);
        let max = u64::try_from(MAX_CONFLICTS).unwrap();
        for _ in 2..max {
            assert_eq!(record.log_conflict().1, None);
        }
        assert_eq!(record.conflicts.len(), MAX_CONFLICTS);
        assert_eq!(record.conflicts_dropped, 0);
        assert_eq!(record.log_conflict(), (max + 1, Some(1)));
        assert_eq!(record.conflicts_dropped, 1);
        assert_eq!(record.conflicts.first(), Some(&2));
        assert_eq!(record.conflict_seq, max + 1);
    }

    #[test]
    fn members_and_media_are_bounded() {
        let mut record = space();
        let device = |n: usize| Principal::Device {
            id: format!("dev-{n}"),
        };
        assert!(!record.admits(&device(0)));
        assert_eq!(record.add_member(device(0)), Ok(()));
        assert!(record.admits(&device(0)));
        assert_eq!(record.add_member(device(0)), Ok(()), "once is enough");
        assert_eq!(record.members.len(), 2);
        for n in 1..MAX_MEMBERS - 1 {
            assert_eq!(record.add_member(device(n)), Ok(()));
        }
        assert_eq!(record.members.len(), MAX_MEMBERS);
        assert_eq!(record.add_member(device(99)), Err(SpaceRefusal::Full));
        assert!(!record.has_media("m-0"));
        for n in 0..MAX_MEDIA {
            let id = format!("m-{n}");
            assert_eq!(record.add_media(&id), Ok(()));
        }
        assert!(record.has_media("m-0"));
        assert_eq!(record.add_media("m-over"), Err(SpaceRefusal::Full));
        assert_eq!((MAX_MEMBERS, MAX_CONFLICTS, MAX_MEDIA), (17, 32, 16));
    }

    #[test]
    fn a_principal_is_labelled_by_what_the_broker_knows() {
        assert_eq!(agent(1000).label(), "uid:1000");
        assert_eq!(
            Principal::TaskRun {
                space: String::from("spc-1")
            }
            .label(),
            "task:spc-1"
        );
        assert_eq!(
            Principal::Device {
                id: String::from("dev-ab")
            }
            .label(),
            "device:dev-ab"
        );
    }

    #[test]
    fn a_task_holds_only_grants_its_space_holds() {
        let mut record = space();
        record
            .register_task(&task("tsk-1", vec![send("local"), vault("token")]), now())
            .unwrap();
        assert_eq!(record.tasks, ["tsk-1"]);
        assert!(record.has_task("tsk-1"));
        assert!(!record.has_task("tsk-2"));
        // Held by the creator, not by the space: refused.
        assert_eq!(
            record.register_task(&task("tsk-2", vec![send("local"), vault("other")]), now()),
            Err(SpaceRefusal::ExceedsGrants)
        );
        // No send on its channel.
        assert_eq!(
            record.register_task(&task("tsk-2", vec![vault("token")]), now()),
            Err(SpaceRefusal::ExceedsGrants)
        );
        let mut blank = task("tsk-2", vec![send("local")]);
        blank.task = String::from("  ");
        assert_eq!(
            record.register_task(&blank, now()),
            Err(SpaceRefusal::BadTask)
        );
        // Another space's task.
        let mut foreign = task("tsk-2", vec![send("local")]);
        foreign.space = String::from("spc-2");
        assert_eq!(
            record.register_task(&foreign, now()),
            Err(SpaceRefusal::NoSuchTask)
        );
        assert_eq!(record.tasks.len(), 1, "refused tasks are not kept");
        for n in 1..MAX_TASKS {
            record
                .register_task(&task(&format!("tsk-{n}x"), vec![send("local")]), now())
                .unwrap();
        }
        assert_eq!(
            record.register_task(&task("tsk-full", vec![send("local")]), now()),
            Err(SpaceRefusal::Full)
        );
    }

    #[test]
    fn a_task_runs_as_a_one_shot_bound_to_its_space() {
        let mut record = space();
        let water = task("tsk-1", vec![send("local")]);
        // Not registered with this space: no run.
        assert_eq!(
            record.schedule_task(&water, String::from("sch-9"), now()),
            Err(SpaceRefusal::NoSuchTask)
        );
        record.register_task(&water, now()).unwrap();
        let schedule = record
            .schedule_task(&water, String::from("sch-9"), now())
            .unwrap();
        assert_eq!(schedule.id, "sch-9");
        assert_eq!(schedule.space.as_deref(), Some("spc-1"));
        assert_eq!(schedule.next_due, 1_790_000_001);
        assert_eq!(schedule.grants, [send("local")]);
        assert_eq!(schedule.task, "water the plants");
        assert_eq!(schedule.depth, 0);
        // Registered here, but naming another space: no run.
        let mut moved = water;
        moved.space = String::from("spc-2");
        assert_eq!(
            record.schedule_task(&moved, String::from("sch-9"), now()),
            Err(SpaceRefusal::NoSuchTask)
        );
    }

    #[test]
    fn every_scheduler_refusal_maps_to_a_space_refusal() {
        for (refusal, expected) in [
            (ScheduleRefusal::ExceedsGrants, SpaceRefusal::ExceedsGrants),
            (
                ScheduleRefusal::ChannelNotGranted,
                SpaceRefusal::ExceedsGrants,
            ),
            (ScheduleRefusal::BadTask, SpaceRefusal::BadTask),
            (ScheduleRefusal::TooManyGrants, SpaceRefusal::BadTask),
            (ScheduleRefusal::NotInFuture, SpaceRefusal::BadTask),
            (ScheduleRefusal::BadDelay, SpaceRefusal::BadTask),
        ] {
            assert_eq!(task_refusal(refusal), expected, "{refusal:?}");
        }
    }
}
