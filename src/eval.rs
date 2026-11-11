//! Measuring Claude's drafts (I2, I3). Each request the demo can make (every
//! kind, every reminder, both tones) is asked once, and the answer goes
//! through the same checks a coordinator's queue relies on. Its cost is
//! counted at list price. Replayed from recordings, every run gives the same
//! result.

use crate::checks::check;
use crate::drafting::{Tone, parse_answer, request, template};
use crate::engine::cost_log::cost_nano_usd;
use crate::engine::llm::{LanguageModel, Usage};
use crate::fhir::Kind;

/// The models compared: Sonnet, which drafts by default, then Haiku.
pub const MODELS: [&str; 2] = ["claude-sonnet-5-5", "claude-haiku-5-5"];

/// The name the clinic's fixed template goes by in the report.
pub const TEMPLATE: &str = "the clinic's template";

/// One request: which kind of follow-up, which reminder, which tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked {
    /// The kind of follow-up.
    pub kind: Kind,
    /// Which reminder, 1 to 3.
    pub number: u8,
    /// The clinic's tone.
    pub tone: Tone,
}

impl Asked {
    fn label(&self) -> String {
        let tone = match self.tone {
            Tone::Warm => "warm",
            Tone::Plain => "plain",
        };
        format!("{}, reminder {}, {tone}", self.kind.code(), self.number)
    }
}

/// Every request the demo can make.
pub fn requests() -> Vec<Asked> {
    let mut all = Vec::new();
    for kind in [Kind::AfterResult, Kind::ScreeningRecall, Kind::AfterDischarge] {
        for number in 1..=3 {
            for tone in [Tone::Warm, Tone::Plain] {
                all.push(Asked { kind, number, tone });
            }
        }
    }
    all
}

/// What became of one request's draft.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// What was asked.
    pub asked: Asked,
    /// Whether the draft passed every check.
    pub passed: bool,
    /// Each problem, as the coordinator would see it.
    pub failures: Vec<String>,
    /// Its reading grade, if there was a draft.
    pub grade: Option<f64>,
    /// The call's cost in billionths of a US dollar, if known; zero for the
    /// template.
    pub cost_nano_usd: Option<i64>,
    /// Why there was no draft at all, e.g. no recording.
    pub error: Option<String>,
}

/// One drafter's results.
#[derive(Debug, Clone, PartialEq)]
pub struct DrafterResult {
    /// The model, or [`TEMPLATE`].
    pub drafter: String,
    /// One outcome per request.
    pub outcomes: Vec<Outcome>,
}

impl DrafterResult {
    /// The share of drafts that passed every check.
    pub fn pass_rate(&self) -> f64 {
        let passed = self.outcomes.iter().filter(|o| o.passed).count();
        passed as f64 / self.outcomes.len().max(1) as f64
    }

    /// The average cost of one draft over the calls whose cost is known.
    pub fn cost_per_draft_usd(&self) -> Option<f64> {
        let costs: Vec<i64> = self.outcomes.iter().filter_map(|o| o.cost_nano_usd).collect();
        (!costs.is_empty()).then(|| costs.iter().sum::<i64>() as f64 / 1e9 / costs.len() as f64)
    }

    fn mean_grade(&self) -> Option<f64> {
        let grades: Vec<f64> = self.outcomes.iter().filter_map(|o| o.grade).collect();
        (!grades.is_empty()).then(|| grades.iter().sum::<f64>() / grades.len() as f64)
    }
}

fn checked(asked: Asked, text: &str, cost_nano_usd: Option<i64>) -> Outcome {
    let result = check(text);
    Outcome {
        asked,
        passed: result.problems.is_empty(),
        failures: result
            .problems
            .iter()
            .map(|p| {
                let name = serde_json::to_value(p.check)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                format!("{name}: {}", p.detail)
            })
            .collect(),
        grade: Some(result.grade),
        cost_nano_usd,
        error: None,
    }
}

/// Asks `model` for every request and checks each draft.
pub async fn evaluate<M: LanguageModel>(model: &M) -> DrafterResult {
    let mut outcomes = Vec::new();
    for asked in requests() {
        let reply = match model.complete(&request(asked.kind, asked.number, asked.tone)).await {
            Ok(reply) => reply,
            Err(err) => {
                outcomes.push(Outcome {
                    asked,
                    passed: false,
                    failures: Vec::new(),
                    grade: None,
                    cost_nano_usd: None,
                    error: Some(err.to_string()),
                });
                continue;
            }
        };
        let usage = Usage {
            input_tokens: reply.input_tokens,
            output_tokens: reply.output_tokens,
        };
        let cost = cost_nano_usd(model.model(), usage);
        outcomes.push(match parse_answer(&reply.text) {
            Ok(text) => checked(asked, &text, cost),
            Err(err) => Outcome {
                asked,
                passed: false,
                failures: vec![err.to_string()],
                grade: None,
                cost_nano_usd: cost,
                error: None,
            },
        });
    }
    DrafterResult {
        drafter: model.model().to_owned(),
        outcomes,
    }
}

/// The clinic's template for every request, which costs nothing.
pub fn evaluate_template() -> DrafterResult {
    DrafterResult {
        drafter: TEMPLATE.to_owned(),
        outcomes: requests()
            .into_iter()
            .map(|asked| checked(asked, &template(asked.number), Some(0)))
            .collect(),
    }
}

/// The report, as Markdown: a table of the drafters, then every draft that
/// failed and why.
pub fn report(results: &[DrafterResult]) -> String {
    let mut text = String::from(
        "# Measurement\n\nClaude's draft of every reminder the made-up demo can ask for (3 kinds of follow-up × \
         reminders 1–3 × 2 tones = 18), each put through the checks every draft must pass before a coordinator can \
         approve it (`src/checks.rs`), beside the clinic's fixed template. Replayed from `evalset/recorded/`; \
         `cargo run -- eval --record` asks Claude again with the owner's key. Costs are at list price. The model \
         sees only the kind, which reminder and the tone, never a patient.\n\n\
         | Drafter | Pass rate | Passed | Average reading grade | Cost per draft |\n|---|---|---|---|---|\n",
    );
    for result in results {
        let passed = result.outcomes.iter().filter(|o| o.passed).count();
        let cost = match result.cost_per_draft_usd() {
            Some(0.0) => "free".to_owned(),
            Some(usd) => format!("US${usd:.5}"),
            None => "unknown".to_owned(),
        };
        let grade = result
            .mean_grade()
            .map_or_else(|| "–".to_owned(), |g| format!("{g:.1}"));
        text.push_str(&format!(
            "| {} | {:.0}% | {passed} of {} | {grade} | {cost} |\n",
            result.drafter,
            result.pass_rate() * 100.0,
            result.outcomes.len()
        ));
    }
    for result in results {
        let failed: Vec<&Outcome> = result.outcomes.iter().filter(|o| !o.passed).collect();
        text.push_str(&format!("\n## {}: drafts that failed\n\n", result.drafter));
        if failed.is_empty() {
            text.push_str("None.\n");
        }
        for outcome in failed {
            text.push_str(&format!("- **{}**\n", outcome.asked.label()));
            for failure in outcome.failures.iter().chain(&outcome.error) {
                text.push_str(&format!("  - {failure}\n"));
            }
        }
    }
    text
}
