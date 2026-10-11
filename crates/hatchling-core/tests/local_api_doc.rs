//! `docs/local-api.md` is checked against the code, so it cannot
//! drift. Every row of its serde-tag table must decode to the type it
//! names, re-encode to exactly the JSON shown, and be that variant; every
//! row of its grant-label table must parse to that grant and print back as
//! that label. The doc is read at build time: a changed doc rebuilds this.

use std::collections::BTreeSet;
use std::fmt::Debug;

use hatchling_core::subagent::{grant_label, parse_grant_label};
use hatchling_core::ui::{UiAction, UiActionResult, UiRequest};
use hatchling_core::{ScheduleGrant, ScheduleOp, ScheduleResult, ScheduleSpec};
use serde::Serialize;
use serde::de::DeserializeOwned;

const DOC: &str = include_str!("../../../docs/local-api.md");

/// The rows of the table between `begin` and `end`: each row's cells,
/// without their backticks; the header and its rule are skipped.
fn table(begin: &str, end: &str) -> Result<Vec<Vec<String>>, String> {
    let body = DOC
        .split_once(begin)
        .and_then(|(_, rest)| rest.split_once(end))
        .map(|(body, _)| body)
        .ok_or_else(|| format!("no table between {begin} and {end}"))?;
    Ok(body
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            line.trim()
                .trim_matches('|')
                .split(" | ")
                .map(|cell| cell.trim().trim_matches('`').to_owned())
                .collect()
        })
        .collect())
}

/// `json` decodes to `T`, re-encodes to itself, and is `variant`.
fn same<T: Serialize + DeserializeOwned + Debug>(variant: &str, json: &str) -> Result<(), String> {
    let value: T = serde_json::from_str(json).map_err(|e| format!("{variant}: {e}"))?;
    let again = serde_json::to_string(&value).map_err(|e| format!("{variant}: {e}"))?;
    if again != json {
        return Err(format!("{variant}: re-encodes as {again}"));
    }
    let shown = format!("{value:?}");
    let named = shown == variant
        || shown.starts_with(&format!("{variant} "))
        || shown.starts_with(&format!("{variant}("));
    if named {
        Ok(())
    } else {
        Err(format!("{json} is {shown}, not {variant}"))
    }
}

#[test]
fn every_serde_tag_example_is_the_wire_form_of_the_variant_it_names() -> Result<(), String> {
    let rows = table("<!-- serde-tags:begin -->", "<!-- serde-tags:end -->")?;
    assert!(rows.len() >= 15, "{rows:?}");
    for row in &rows {
        let [rust, json] = row.as_slice() else {
            return Err(format!("not a two-cell row: {row:?}"));
        };
        let (ty, variant) = rust
            .split_once("::")
            .ok_or_else(|| format!("{rust} names no variant"))?;
        match ty {
            "UiRequest" => same::<UiRequest>(variant, json)?,
            "UiAction" => same::<UiAction>(variant, json)?,
            "UiActionResult" => same::<UiActionResult>(variant, json)?,
            "ScheduleOp" => same::<ScheduleOp>(variant, json)?,
            "ScheduleSpec" => same::<ScheduleSpec>(variant, json)?,
            "ScheduleGrant" => same::<ScheduleGrant>(variant, json)?,
            "ScheduleResult" => same::<ScheduleResult>(variant, json)?,
            other => return Err(format!("{other} is not a type this check knows")),
        }
    }
    Ok(())
}

#[test]
fn every_grant_label_names_exactly_its_json_grant() -> Result<(), String> {
    let rows = table("<!-- grant-labels:begin -->", "<!-- grant-labels:end -->")?;
    let mut kinds = BTreeSet::new();
    for row in &rows {
        let [label, json] = row.as_slice() else {
            return Err(format!("not a two-cell row: {row:?}"));
        };
        same::<ScheduleGrant>(
            json.split_once("\"grant\":\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(kind, _)| kind)
                .map_or("", |kind| match kind {
                    "vault" => "Vault",
                    "egress" => "Egress",
                    "memory" => "Memory",
                    "goals" => "Goals",
                    "channel-send" => "ChannelSend",
                    _ => "",
                }),
            json,
        )?;
        let grant: ScheduleGrant = serde_json::from_str(json).map_err(|e| e.to_string())?;
        assert_eq!(parse_grant_label(label).as_ref(), Some(&grant), "{label}");
        assert_eq!(&grant_label(&grant), label);
        kinds.insert(label.split(':').next().unwrap_or_default().to_owned());
    }
    assert_eq!(
        kinds,
        BTreeSet::from(["channel-send", "egress", "goals", "memory", "vault"].map(String::from)),
        "every kind of grant has its row"
    );
    Ok(())
}
