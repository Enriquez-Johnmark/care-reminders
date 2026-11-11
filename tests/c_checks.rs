//! C2–C5 and C7: the checks every draft passes before a person sees it.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use care_reminders::checks::{Check, check, reading_grade};

const GOOD: &str = "Hello {first_name}, this is {clinic_name}. It's time to book a follow-up visit with us. \
                    Please call {clinic_phone} to pick a time that works for you, by {due_by}. \
                    If you'd rather not get these messages, you can stop them here: {opt_out_link}";

/// The checks `text` fails, each once, in the order found.
fn failed(text: &str) -> Vec<Check> {
    let mut checks: Vec<Check> = Vec::new();
    for problem in check(text).problems {
        if !checks.contains(&problem.check) {
            checks.push(problem.check);
        }
    }
    checks
}

#[test]
fn c2_a_good_draft_passes_every_check() {
    let checked = check(GOOD);

    assert!(checked.problems.is_empty(), "{:?}", checked.problems);
}

#[test]
fn c2_a_fill_in_not_on_the_list_is_blocked() {
    let text = GOOD.replace("{first_name}", "{full_name}");

    assert_eq!(failed(&text), [Check::FillIns]);
}

#[test]
fn c2_a_draft_missing_the_phone_or_the_opt_out_link_is_blocked() {
    for missing in ["{clinic_phone}", "{opt_out_link}"] {
        let text = GOOD.replace(missing, "us");
        assert_eq!(failed(&text), [Check::FillIns], "{missing}");
    }
}

#[test]
fn c2_an_unclosed_fill_in_is_blocked() {
    let text = GOOD.replace("{first_name}", "{first_name");

    assert!(failed(&text).contains(&Check::FillIns));
}

#[test]
fn c3_a_draft_with_its_own_digit_date_or_time_is_blocked() {
    for own in [
        "Your visit is on the 3rd.",
        "We are open until five.",
        "Come in on Tuesday.",
        "Please call before December.",
        "We can see you tomorrow.",
        "Our desk opens at noon.",
        "Call us any time after nine a.m.",
    ] {
        let text = format!("{GOOD} {own}");
        assert_eq!(failed(&text), [Check::NumbersAndDates], "{own}");
    }
}

#[test]
fn c4_a_draft_with_clinical_words_or_advice_is_blocked() {
    for clinical in [
        "This is about your cholesterol.",
        "We want to talk about your test results.",
        "Your cardiology visit is due.",
        "It's time for your colonoscopy.",
        "This follows your hospital stay.",
        "Keep taking your medication until then.",
        "You should rest and drink water.",
        "This is about your chlamydia result.",
    ] {
        let text = format!("{GOOD} {clinical}");
        assert_eq!(failed(&text), [Check::ClinicalWords], "{clinical}");
    }
}

#[test]
fn c4_words_are_matched_whole_so_harmless_words_pass() {
    // "heartily" holds "heart", "testimony" holds "test".
    let text = format!("{GOOD} We heartily thank you; your testimony matters.");

    assert!(
        !failed(&text).contains(&Check::ClinicalWords),
        "{:?}",
        check(&text).problems
    );
}

#[test]
fn c5_a_draft_above_eighth_grade_is_blocked_and_above_sixth_warns() {
    let hard = "Hello {first_name}. Our organisation's comprehensive administrative procedures necessitate \
                considerable documentation regarding appointment availability and institutional responsibilities. \
                Telephone {clinic_phone} by {due_by}. Unsubscribe: {opt_out_link}";
    let middling = "Hello {first_name}, this is {clinic_name}. We would like you to schedule a follow-up \
                    appointment at your earliest convenience. Please telephone {clinic_phone} before {due_by}. \
                    To stop receiving these messages, visit {opt_out_link}";

    assert!(
        failed(hard).contains(&Check::ReadingLevel),
        "grade {}",
        reading_grade(hard)
    );
    let warned = check(middling);
    let grade = reading_grade(middling);
    assert!(
        warned.problems.is_empty() && (6.0..=8.0).contains(&grade),
        "grade {grade}: {:?}",
        warned.problems
    );
    assert_eq!(warned.warnings.len(), 1, "grade {grade}");
}

#[test]
fn c5_the_reading_level_is_the_same_every_time_for_the_same_draft() {
    assert_eq!(reading_grade(GOOD), reading_grade(GOOD));
    assert!(reading_grade(GOOD) <= 6.0, "{}", reading_grade(GOOD));
}

#[test]
fn c7_every_problem_names_its_check_and_what_was_found() {
    let text = format!("{GOOD} Come in on Tuesday about your cholesterol.");

    let problems = check(&text).problems;

    let found: Vec<(Check, &str)> = problems.iter().map(|p| (p.check, p.detail.as_str())).collect();
    assert!(
        found
            .iter()
            .any(|(c, d)| *c == Check::NumbersAndDates && d.contains("Tuesday")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|(c, d)| *c == Check::ClinicalWords && d.contains("cholesterol")),
        "{found:?}"
    );
}
