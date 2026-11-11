//! J1: every acceptance criterion is matched to its tests and their results;
//! a criterion of a finished milestone with no passing test fails the build.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use care_reminders::evidence::{Evidence, Status};

const SPEC: &str = "\
### A. Data
- **A1** The generator writes the same files.
- **A2** The model sees only results.
- **B1** The answer is parsed.
";

fn architecture(m1: &str) -> String {
    format!(
        "| Milestone | What | Criteria | Status |\n|---|---|---|---|\n\
         | M1 | Data | A1, A2 | {m1} |\n| M2 | Drafts | B1 | not started |\n"
    )
}

fn junit(cases: &str) -> String {
    format!(r#"<?xml version="1.0"?><testsuites><testsuite name="s">{cases}</testsuite></testsuites>"#)
}

const A1_PASSES: &str = r#"<testcase name="a1_same_files" classname="care::a1"/>"#;
const A2_FAILS: &str = r#"<testcase name="a2_only_results" classname="care::a2"><failure message="boom"/></testcase>"#;

fn evidence(m1: &str, cases: &str) -> Evidence {
    Evidence::build(SPEC, &architecture(m1), &junit(cases)).unwrap()
}

fn status(evidence: &Evidence, id: &str) -> Status {
    evidence.rows.iter().find(|row| row.id == id).unwrap().status
}

#[test]
fn j1_each_criterion_is_matched_to_the_tests_named_by_its_id() {
    let evidence = evidence("not started", A1_PASSES);
    let a1 = evidence.rows.iter().find(|row| row.id == "A1").unwrap();
    assert_eq!(a1.tests, [("care::a1::a1_same_files".to_owned(), true)]);
    assert_eq!(a1.milestone, "M1");
}

#[test]
fn j1_a_criterion_is_proven_missing_failing_or_not_yet_due() {
    let evidence = evidence("done", &format!("{A1_PASSES}{A2_FAILS}"));
    assert_eq!(status(&evidence, "A1"), Status::Proven);
    assert_eq!(status(&evidence, "A2"), Status::Failing);
    assert_eq!(status(&evidence, "B1"), Status::NotYetDue);
}

#[test]
fn j1_a_finished_milestones_criterion_without_a_passing_test_is_a_gap() {
    let evidence = evidence("done", A1_PASSES);
    assert_eq!(status(&evidence, "A2"), Status::Missing);
    assert_eq!(evidence.gaps(), ["A2 (M1): no test"]);
}

#[test]
fn j1_an_unfinished_milestone_has_no_gaps_yet() {
    assert!(evidence("in progress", "").gaps().is_empty());
}

#[test]
fn j1_a_criterion_missing_from_the_build_order_is_a_gap() {
    let spec = format!("{SPEC}- **C1** Only the coordinator approves.\n");
    let evidence = Evidence::build(&spec, &architecture("not started"), &junit("")).unwrap();
    assert_eq!(evidence.gaps(), ["C1: in no milestone"]);
}

#[test]
fn j1_ranges_in_the_build_order_cover_every_criterion_between() {
    let spec = "- **B1** a\n- **B2** b\n- **B3** c\n";
    let arch = "| M2 | Drafts | B1–B3 | done |\n";
    let evidence = Evidence::build(spec, arch, &junit("")).unwrap();
    assert_eq!(
        evidence.gaps(),
        ["B1 (M2): no test", "B2 (M2): no test", "B3 (M2): no test"]
    );
}

#[test]
fn j1_the_page_lists_each_criterion_with_its_tests_and_status() {
    let page = evidence("done", &format!("{A1_PASSES}{A2_FAILS}")).html("abc1234", None);
    assert!(
        page.contains("A1") && page.contains("care::a1::a1_same_files") && page.contains("Proven"),
        "{page}"
    );
    assert!(page.contains("Failing"), "{page}");
}

#[test]
fn j1_the_real_spec_and_build_order_place_every_criterion_in_a_milestone() {
    let root = env!("CARGO_MANIFEST_DIR");
    let spec = std::fs::read_to_string(format!("{root}/docs/product/spec.md")).unwrap();
    let arch = std::fs::read_to_string(format!("{root}/docs/architecture.md")).unwrap();
    let evidence = Evidence::build(&spec, &arch, &junit("")).unwrap();
    assert_eq!(evidence.rows.len(), 43);
    assert!(
        evidence.rows.iter().all(|row| !row.milestone.is_empty()),
        "{:?}",
        evidence.gaps()
    );
}
