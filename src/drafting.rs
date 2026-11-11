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
    /// The link to stop these reminders.
    pub opt_out_link: String,
}

/// Why a draft couldn't be filled.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("the draft has a fill-in that isn't on the list, or a brace left open")]
pub struct FillError;

/// `draft` with each fill-in replaced by its value (C6). A draft with
/// anything else in braces is refused, never sent half-filled.
pub fn fill(draft: &str, values: &FillIns) -> Result<String, FillError> {
    let due_by = values
        .due_by
        .format(format_description!("[month repr:long] [day padding:none], [year]"))
        .map_err(|_| FillError)?;
    let mut filled = draft.to_owned();
    for name in FILL_INS {
        let value = match name {
            "first_name" => values.first_name.as_str(),
            "clinic_name" => values.clinic_name.as_str(),
            "clinic_phone" => values.clinic_phone.as_str(),
            "due_by" => due_by.as_str(),
            _ => values.opt_out_link.as_str(),
        };
        filled = filled.replace(&format!("{{{name}}}"), value);
    }
    // The values come from the clinic's data, so a brace left now is one the
    // draft had of its own.
    let leftover = FILL_INS
        .iter()
        .fold(draft.to_owned(), |text, name| text.replace(&format!("{{{name}}}"), ""))
        .contains(['{', '}']);
    if leftover { Err(FillError) } else { Ok(filled) }
}
