//! I2 and I3: Claude's drafts of every reminder the demo can ask for, put
//! through the checks; Sonnet beside Haiku, and both beside the template.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::eval::{MODELS, TEMPLATE, evaluate, evaluate_template, report, requests};
use care_reminders::model::{DIR, Recorded};
use common::{Fake, GOOD_DRAFT};

#[test]
fn i2_every_reminder_the_demo_can_ask_for_is_measured() {
    // Three kinds, three reminders each, two tones.
    assert_eq!(requests().len(), 18);
}

#[tokio::test]
async fn i2_a_draft_that_fails_a_check_is_counted_and_listed_with_why() {
    let good = evaluate(&Fake(Some(GOOD_DRAFT))).await;
    let clinical = evaluate(&Fake(Some(
        "Hello {first_name}, this is {clinic_name}, about your cholesterol. Call {clinic_phone} by {due_by}. \
         Stop: {opt_out_link}",
    )))
    .await;
    let silent = evaluate(&Fake(None)).await;

    assert_eq!(good.pass_rate(), 1.0);
    assert_eq!(clinical.pass_rate(), 0.0);
    assert!(clinical.outcomes[0].failures.iter().any(|f| f.contains("cholesterol")));
    assert_eq!(silent.pass_rate(), 0.0);
    assert!(silent.outcomes[0].error.is_some());
    let text = report(&[good, clinical]);
    assert!(text.contains("cholesterol"), "{text}");
}

#[tokio::test]
async fn i2_at_least_nine_in_ten_of_claudes_recorded_drafts_pass() {
    let sonnet = evaluate(&Recorded::new(MODELS[0], std::path::Path::new(DIR))).await;

    assert!(
        sonnet.outcomes.iter().all(|o| o.error.is_none()),
        "every request is recorded"
    );
    assert!(sonnet.pass_rate() >= 0.9, "{}", report(&[sonnet]));
}

#[tokio::test]
async fn i3_the_report_compares_sonnet_haiku_and_the_template_on_pass_rate_and_cost() {
    let mut results = Vec::new();
    for model in MODELS {
        results.push(evaluate(&Recorded::new(model, std::path::Path::new(DIR))).await);
    }
    results.push(evaluate_template());

    let text = report(&results);

    for name in ["claude-sonnet-5-5", "claude-haiku-5-5", TEMPLATE] {
        let row = text
            .lines()
            .find(|line| line.starts_with(&format!("| {name} |")))
            .unwrap();
        assert!(
            row.contains('%') && (row.contains("US$") || row.contains("free")),
            "{row}"
        );
    }
    let committed = std::fs::read_to_string("evalset/report.md").unwrap();
    assert_eq!(
        committed, text,
        "evalset/report.md is out of date: run `cargo run -- eval`"
    );
}
