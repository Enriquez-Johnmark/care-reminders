//! The checks every reminder draft passes before a person sees it (C2–C5,
//! C7), whoever wrote it: the model, a template or a coordinator (D4).
//!
//! A draft is the text with its fill-ins, such as `{first_name}`, not yet
//! filled. Any problem blocks it; a warning doesn't.

use serde::{Deserialize, Serialize};

/// The fill-ins a draft may use; code fills each from the clinic's data.
pub const FILL_INS: [&str; 5] = ["first_name", "clinic_name", "clinic_phone", "due_by", "opt_out_link"];

/// Fill-ins every draft must have: a way to reach a person, and a way to stop.
pub const REQUIRED: [&str; 2] = ["clinic_phone", "opt_out_link"];

/// Above this reading grade a draft is blocked; above [`WARN_GRADE`] it warns.
pub const MAX_GRADE: f64 = 8.0;
/// Above this reading grade, a warning.
pub const WARN_GRADE: f64 = 6.0;

/// Which check found a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    /// The model's answer couldn't be read (C1).
    Answer,
    /// A fill-in not on the list, or a required one missing (C2).
    FillIns,
    /// A digit, a date or a time of the draft's own (C3).
    NumbersAndDates,
    /// A condition, test, specialty, medication or advice (C4).
    ClinicalWords,
    /// Too hard to read (C5).
    ReadingLevel,
}

/// One problem: which check, and what it found (C7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// The check.
    pub check: Check,
    /// What it found, for the coordinator.
    pub detail: String,
}

/// What the checks found in a draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checked {
    /// Problems: any one blocks the draft.
    pub problems: Vec<Problem>,
    /// Warnings, which don't block it.
    pub warnings: Vec<String>,
    /// Its reading grade (Flesch–Kincaid).
    pub grade: f64,
}

/// Runs every check on `draft`.
pub fn check(draft: &str) -> Checked {
    let mut problems = Vec::new();
    let (fill_ins, plain) = split_fill_ins(draft, &mut problems);
    for required in REQUIRED {
        if !fill_ins.iter().any(|name| name == required) {
            problems.push(Problem {
                check: Check::FillIns,
                detail: format!("{{{required}}} is missing"),
            });
        }
    }
    numbers_and_dates(&plain, &mut problems);
    clinical_words(&plain, &mut problems);
    let grade = reading_grade(draft);
    let mut warnings = Vec::new();
    if grade > MAX_GRADE {
        problems.push(Problem {
            check: Check::ReadingLevel,
            detail: format!("reading grade {grade:.1}, above {MAX_GRADE}"),
        });
    } else if grade > WARN_GRADE {
        warnings.push(format!(
            "reading grade {grade:.1}, above {WARN_GRADE}: simpler words may help"
        ));
    }
    Checked {
        problems,
        warnings,
        grade,
    }
}

/// The fill-ins named in `draft`, and its text with each replaced by a
/// space. A fill-in not on the list, or a brace left open, is a problem.
fn split_fill_ins(draft: &str, problems: &mut Vec<Problem>) -> (Vec<String>, String) {
    let mut names = Vec::new();
    let mut plain = String::with_capacity(draft.len());
    let mut rest = draft;
    while let Some(open) = rest.find(['{', '}']) {
        plain.push_str(&rest[..open]);
        if rest[open..].starts_with('}') {
            problems.push(Problem {
                check: Check::FillIns,
                detail: "a \"}\" with no \"{\" before it".to_owned(),
            });
            rest = &rest[open + 1..];
            continue;
        }
        let Some(close) = rest[open..].find('}') else {
            problems.push(Problem {
                check: Check::FillIns,
                detail: format!(
                    "an unclosed fill-in: {}",
                    rest[open..].chars().take(20).collect::<String>()
                ),
            });
            rest = "";
            break;
        };
        let name = &rest[open + 1..open + close];
        if FILL_INS.contains(&name) {
            names.push(name.to_owned());
        } else {
            problems.push(Problem {
                check: Check::FillIns,
                detail: format!("{{{name}}} is not a fill-in"),
            });
        }
        plain.push(' ');
        rest = &rest[open + close + 1..];
    }
    plain.push_str(rest);
    (names, plain)
}

/// Words that name a number, a day, a month or a time, matched in lower
/// case in any of a word's forms (see [`forms`]). "May", "Mar", "Sat" and
/// "Sun" are left out, being more often other words, as are "first" and
/// "second".
const NUMBER_WORDS: &[&str] = &[
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
    "hundred",
    "thousand",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "twentieth",
    "thirtieth",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "mon",
    "tue",
    "tues",
    "wed",
    "thu",
    "thur",
    "thurs",
    "fri",
    "january",
    "february",
    "march",
    "april",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "oct",
    "nov",
    "dec",
    "today",
    "tonight",
    "tomorrow",
    "yesterday",
    "noon",
    "midnight",
    "o'clock",
    "a.m",
    "p.m",
];
const DATE_PHRASES: &[&str] = &["next week", "this week", "next month", "this month"];

fn numbers_and_dates(plain: &str, problems: &mut Vec<Problem>) {
    let mut found: Vec<String> = Vec::new();
    // Any script's digits, not only 0 to 9.
    if let Some(digit) = plain.split_whitespace().find(|word| word.chars().any(char::is_numeric)) {
        found.push(trim(digit).to_owned());
    }
    for word in plain.split_whitespace().map(trim) {
        if forms(word).iter().any(|form| NUMBER_WORDS.contains(&form.as_str())) {
            found.push(word.to_owned());
        }
    }
    found.extend(phrases(plain, DATE_PHRASES));
    for word in found {
        problems.push(Problem {
            check: Check::NumbersAndDates,
            detail: format!("\"{word}\" is a number, date or time of the draft's own; only {{due_by}} gives a date"),
        });
    }
}

/// Conditions, tests, specialties, medications and hospital words: anything
/// that would tell a reader why the patient is being reminded.
const CLINICAL_WORDS: &[&str] = &[
    "cholesterol",
    "diabetes",
    "diabetic",
    "thyroid",
    "cancer",
    "tumor",
    "tumour",
    "infection",
    "chlamydia",
    "hiv",
    "std",
    "sti",
    "sexual",
    "pregnancy",
    "pregnant",
    "heart",
    "kidney",
    "liver",
    "blood",
    "potassium",
    "glucose",
    "a1c",
    "hemoglobin",
    "anemia",
    "test",
    "tests",
    "tested",
    "testing",
    "result",
    "results",
    "lab",
    "labs",
    "screening",
    "screenings",
    "colonoscopy",
    "mammogram",
    "mammography",
    "scan",
    "x-ray",
    "biopsy",
    "cardiology",
    "oncology",
    "gynecology",
    "gynaecology",
    "urology",
    "psychiatry",
    "psychiatric",
    "mental",
    "addiction",
    "rehab",
    "fertility",
    "hospital",
    "hospitalized",
    "discharge",
    "discharged",
    "admitted",
    "admission",
    "surgery",
    "emergency",
    "diagnosis",
    "diagnosed",
    "condition",
    "disease",
    "symptom",
    "symptoms",
    "medication",
    "medications",
    "medicine",
    "medicines",
    "pill",
    "pills",
    "prescription",
    "dose",
    "dosage",
    "treatment",
    "therapy",
    "diet",
    "exercise",
    "insulin",
    "statin",
    "abnormal",
];
/// Advice, however it's worded.
const ADVICE: &[&str] = &[
    "you should",
    "you must",
    "make sure to",
    "be sure to",
    "keep taking",
    "stop taking",
    "start taking",
];

fn clinical_words(plain: &str, problems: &mut Vec<Problem>) {
    let mut found: Vec<String> = plain
        .split_whitespace()
        .map(trim)
        .filter(|word| forms(word).iter().any(|form| CLINICAL_WORDS.contains(&form.as_str())))
        .map(str::to_owned)
        .collect();
    found.extend(phrases(plain, ADVICE));
    for word in found {
        problems.push(Problem {
            check: Check::ClinicalWords,
            detail: format!("\"{word}\" tells the reader about the patient's care or gives advice"),
        });
    }
}

/// The phrases of `list` that appear in `plain` as whole words.
fn phrases(plain: &str, list: &[&str]) -> Vec<String> {
    let words: Vec<String> = plain.split_whitespace().map(|w| trim(w).to_lowercase()).collect();
    let text = format!(" {} ", words.join(" "));
    list.iter()
        .filter(|phrase| text.contains(&format!(" {phrase} ")))
        .map(|phrase| (*phrase).to_owned())
        .collect()
}

/// The forms a word is matched in: lower case, without a possessive, and
/// each part of a hyphenated word, each also as a singular. So "Tuesday's",
/// "blood-test", "surgeries" and "x-rays" are found as "tuesday", "blood",
/// "surgery" and "x-ray".
fn forms(word: &str) -> Vec<String> {
    let lower = word.to_lowercase().replace('\u{2019}', "'");
    let lower = lower
        .trim_end_matches('.')
        .trim_end_matches("'s")
        .trim_end_matches('\'')
        .to_owned();
    let mut bases = vec![lower.clone()];
    if lower.contains('-') {
        bases.extend(lower.split('-').filter(|part| !part.is_empty()).map(str::to_owned));
    }
    let mut all = Vec::new();
    for base in bases {
        if let Some(stem) = base.strip_suffix("ies") {
            all.push(format!("{stem}y"));
        }
        if let Some(stem) = base.strip_suffix("es") {
            all.push(stem.to_owned());
        }
        if let Some(stem) = base.strip_suffix('s') {
            all.push(stem.to_owned());
        }
        all.push(base);
    }
    all
}

/// A word without the punctuation around it.
fn trim(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .trim_end_matches('\'')
}

/// The Flesch–Kincaid grade of `draft`, with each fill-in read as a fixed
/// short word, so the same draft always scores the same.
pub fn reading_grade(draft: &str) -> f64 {
    let mut text = draft.to_owned();
    for (name, sample) in [
        ("first_name", "Ana"),
        ("clinic_name", "the clinic"),
        ("clinic_phone", "us"),
        ("due_by", "soon"),
        ("opt_out_link", "here"),
    ] {
        text = text.replace(&format!("{{{name}}}"), sample);
    }
    let words: Vec<&str> = text.split_whitespace().map(trim).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return 0.0;
    }
    let sentences = text
        .split(['.', '!', '?'])
        .filter(|part| part.chars().any(char::is_alphabetic))
        .count()
        .max(1);
    let syllables: usize = words.iter().map(|word| syllables(word)).sum();
    let (words, sentences, syllables) = (words.len() as f64, sentences as f64, syllables as f64);
    let grade = 0.39 * (words / sentences) + 11.8 * (syllables / words) - 15.59;
    (grade * 10.0).round() / 10.0
}

/// About how many syllables `word` has: its groups of vowels, less a silent
/// final "e", and at least one.
fn syllables(word: &str) -> usize {
    let word = word.to_lowercase();
    let mut count = 0;
    let mut in_vowels = false;
    for c in word.chars() {
        let vowel = "aeiouy".contains(c);
        if vowel && !in_vowels {
            count += 1;
        }
        in_vowels = vowel;
    }
    if word.ends_with('e') && !word.ends_with("le") && count > 1 {
        count -= 1;
    }
    count.max(1)
}
