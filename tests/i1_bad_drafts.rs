//! I1: all 40 hand-written bad drafts are blocked, each by the check it was
//! written to trip, and only that one.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use care_reminders::checks::{Check, check};
use serde::Deserialize;

#[derive(Deserialize)]
struct BadDraft {
    id: String,
    trips: Check,
    text: String,
}

#[derive(Deserialize)]
struct File {
    drafts: Vec<BadDraft>,
}

fn bad_drafts() -> Vec<BadDraft> {
    let path = format!("{}/evalset/bad-drafts.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str::<File>(&std::fs::read_to_string(path).unwrap())
        .unwrap()
        .drafts
}

#[test]
fn i1_all_forty_bad_drafts_are_blocked_each_by_the_check_it_was_written_to_trip() {
    let drafts = bad_drafts();
    assert_eq!(drafts.len(), 40);
    for draft in drafts {
        let mut checks: Vec<Check> = check(&draft.text).problems.iter().map(|p| p.check).collect();
        checks.dedup();
        assert_eq!(checks, [draft.trips], "{}: {}", draft.id, draft.text);
    }
}

#[test]
fn i1_the_bad_drafts_cover_every_check_on_text() {
    let drafts = bad_drafts();
    for wanted in [
        Check::FillIns,
        Check::NumbersAndDates,
        Check::ClinicalWords,
        Check::ReadingLevel,
    ] {
        assert!(drafts.iter().filter(|d| d.trips == wanted).count() >= 6, "{wanted:?}");
    }
}
