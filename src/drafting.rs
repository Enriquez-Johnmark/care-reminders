//! What the model is asked, and how its answer becomes a reminder (A2, C1,
//! C6).
//!
//! The model sees only the kind of follow-up, which reminder it is (first,
//! second or third) and the clinic's tone: never a name, a date or a reason.
//! It writes with fill-ins, such as `{first_name}`, and code fills each from
//! the clinic's data, so a name or date in a reminder never comes from it.

use serde::{Deserialize, Serialize};
use serde_json::json;
use time::Date;
use time::macros::format_description;

use crate::checks::FILL_INS;
use crate::engine::llm::LlmRequest;
use crate::fhir::Kind;

/// The instructions. They name no patient, clinic or reason.
pub const SYSTEM: &str = "\
You write short email reminders from a primary-care clinic asking a patient to book a visit. A person at the clinic \
reads and approves every reminder before it is sent.

Write with these fill-ins, exactly as written, and no others: {first_name}, {clinic_name}, {clinic_phone}, {due_by}, \
{opt_out_link}. Code replaces them with the patient's first name, the clinic's name, its phone number, the date to \
book by, and a link to stop these reminders. Always include {clinic_phone} and {opt_out_link}.

Rules:
- Never say why the visit is needed: no test, result, condition, illness, body part, hospital stay, screening, \
specialty, medicine or treatment. Say only \"a follow-up visit\" or \"a check-up\".
- Give no advice of any kind.
- Write no digits, dates, days, months or times of your own; the only date is {due_by}.
- Use short, plain words and short sentences, for a reader at a sixth-grade level. Keep it under eighty words.
- For a second or third reminder, say gently that this follows an earlier message.";

/// How the clinic's reminders sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    /// Friendly.
    Warm,
    /// Brief and neutral.
    Plain,
}

/// The request for reminder `number` (1 to 3) of a follow-up of `kind`. It
/// takes nothing else, so nothing about the patient can reach it (A2).
pub fn request(kind: Kind, number: u8, tone: Tone) -> LlmRequest {
    LlmRequest {
        system: SYSTEM.to_owned(),
        user: json!({ "kind": kind.code(), "reminder": number, "of": 3, "tone": tone }).to_string(),
        schema: json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
            "additionalProperties": false
        }),
        max_tokens: 400,
    }
}

/// The longest answer accepted, in characters.
const MAX_LENGTH: usize = 1_200;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    text: String,
}

/// Why the model's answer couldn't be read (C1).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnswerError {
    /// Not the JSON asked for.
    #[error("the answer isn't the JSON asked for: {0}")]
    NotJson(String),
    /// No text.
    #[error("the answer has no text")]
    Empty,
    /// Far longer than a reminder.
    #[error("the answer is longer than {MAX_LENGTH} characters")]
    TooLong,
}

/// The draft in the model's answer `text`.
pub fn parse_answer(text: &str) -> Result<String, AnswerError> {
    let answer: Answer = serde_json::from_str(text).map_err(|err| AnswerError::NotJson(err.to_string()))?;
    let draft = answer.text.trim();
    if draft.is_empty() {
        return Err(AnswerError::Empty);
    }
    if draft.chars().count() > MAX_LENGTH {
        return Err(AnswerError::TooLong);
    }
    Ok(draft.to_owned())
}

/// The clinic's fixed template for reminder `number`, which stands in when
/// the coordinator prefers it, or no model answers.
pub fn template(number: u8) -> String {
    let opening = match number {
        1 => "It's time to book a follow-up visit with us.",
        2 => "We wrote to you a little while ago. It's still time to book a follow-up visit with us.",
        _ => "This is our last note about this. We'd still like to see you for a follow-up visit.",
    };
    format!(
        "Hello {{first_name}}, this is {{clinic_name}}. {opening} Please call {{clinic_phone}} to pick a time that \
         works for you, by {{due_by}}. If you'd rather not get these messages, you can stop them here: \
         {{opt_out_link}}"
    )
}

/// What the fill-ins are filled with: the clinic's data, never the model's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillIns {
    /// The patient's first name.
    pub first_name: String,
    /// The clinic's name.
    pub clinic_name: String,
    /// The clinic's phone number.
    pub clinic_phone: String,
    /// The date to book by.
    pub due_by: Date,
    /// The clinic's day today: once `due_by` has passed, the patient is
    /// asked to book as soon as they can instead of by a date gone by.
    pub today: Date,
    /// The link to stop these reminders.
    pub opt_out_link: String,
}

/// Why a draft couldn't be filled.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FillError {
    /// Something in braces that isn't a fill-in, or a brace left open.
    #[error("the draft has a fill-in that isn't on the list, or a brace left open")]
    NotAFillIn,
    /// The date has passed, and the draft gives it in words that can't be
    /// changed to "as soon as you can" safely.
    #[error("the date to book by has passed; use the template, or give the date after \"by\"")]
    Overdue,
    /// The clinic's data has no value for a fill-in.
    #[error("there is nothing to fill {{{0}}} with")]
    Empty(&'static str),
}

/// A date as a reminder gives it, e.g. "December 9, 2026".
pub fn date_text(date: Date) -> Option<String> {
    date.format(format_description!("[month repr:long] [day padding:none], [year]"))
        .ok()
}

/// `draft` with each fill-in replaced by its value (C6), in one pass, so a
/// value is never itself filled. A draft with anything else in braces, or a
/// fill-in with no value, is refused: never sent half-filled.
pub fn fill(draft: &str, values: &FillIns) -> Result<String, FillError> {
    if values.due_by < values.today {
        return fill_overdue(draft, values);
    }
    let due_by = date_text(values.due_by).ok_or(FillError::Empty("due_by"))?;
    let mut filled = String::with_capacity(draft.len() + 100);
    let mut rest = draft;
    while let Some(open) = rest.find(['{', '}']) {
        filled.push_str(&rest[..open]);
        let close = rest[open..]
            .find('}')
            .filter(|_| rest[open..].starts_with('{'))
            .ok_or(FillError::NotAFillIn)?;
        let name = FILL_INS
            .iter()
            .find(|name| **name == &rest[open + 1..open + close])
            .ok_or(FillError::NotAFillIn)?;
        let value = match *name {
            "first_name" => values.first_name.as_str(),
            "clinic_name" => values.clinic_name.as_str(),
            "clinic_phone" => values.clinic_phone.as_str(),
            "due_by" => due_by.as_str(),
            _ => values.opt_out_link.as_str(),
        };
        if value.trim().is_empty() {
            return Err(FillError::Empty(name));
        }
        filled.push_str(value);
        rest = &rest[open + close + 1..];
    }
    filled.push_str(rest);
    Ok(filled)
}

/// Words that may come before `{due_by}`; with them it reads "as soon as you
/// can" once the date has passed. Longest first, matched whole and in any case.
const BEFORE_DUE_BY: [&str; 5] = ["prior to", "preceding", "before", "until", "by"];

/// [`fill`] for a date that has passed: "by {due_by}" (or "before", "prior
/// to", "preceding", "until") becomes "as soon as you can"; any other
/// `{due_by}` is refused rather than giving a date gone by.
fn fill_overdue(draft: &str, values: &FillIns) -> Result<String, FillError> {
    let mut reworded = String::with_capacity(draft.len());
    let mut rest = draft;
    while let Some(at) = rest.find("{due_by}") {
        let head = rest[..at].trim_end();
        let lower = head.to_lowercase();
        let word = BEFORE_DUE_BY
            .iter()
            .find(|word| {
                lower.ends_with(*word)
                    && lower[..lower.len() - word.len()]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_alphanumeric())
            })
            .ok_or(FillError::Overdue)?;
        // Lowercasing keeps ASCII lengths, and these words are ASCII.
        let kept = head[..head.len() - word.len()].trim_end_matches([' ', ',']);
        reworded.push_str(kept);
        reworded.push_str(" as soon as you can");
        rest = &rest[at + "{due_by}".len()..];
    }
    reworded.push_str(rest);
    // No date is left to fill; any other fill-in is filled as usual.
    let values = FillIns {
        today: values.due_by,
        ..values.clone()
    };
    fill(&reworded, &values)
}
