//! The evidence pack (J1): each acceptance criterion in the spec, the
//! milestone that builds it, the tests whose names start with its ID and
//! whether they passed, read from `cargo nextest`'s JUnit results.
//!
//! A criterion of a milestone marked done in the build order needs at least
//! one test and no failing one; otherwise it is a gap, and the build fails.

use quick_xml::Reader;

use crate::html::escape;
use quick_xml::events::Event;

/// Where a criterion stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// It has tests, and all of them passed.
    Proven,
    /// A test of it failed.
    Failing,
    /// Its milestone is done but it has no test.
    Missing,
    /// Its milestone isn't done yet.
    NotYetDue,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Proven => "Proven",
            Self::Failing => "Failing",
            Self::Missing => "Missing",
            Self::NotYetDue => "Not yet due",
        }
    }
}

/// One criterion's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Its ID, e.g. `A1`.
    pub id: String,
    /// What it says.
    pub text: String,
    /// The milestone that builds it, or empty if none does.
    pub milestone: String,
    /// Each matching test and whether it passed.
    pub tests: Vec<(String, bool)>,
    /// Where it stands.
    pub status: Status,
}

/// The whole pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// One row per criterion, in the spec's order.
    pub rows: Vec<Row>,
}

/// Why the evidence couldn't be built.
#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    /// The JUnit file isn't valid XML.
    #[error("the test results are not valid JUnit XML: {0}")]
    Junit(String),
}

impl Evidence {
    /// The pack from the spec, the build order (`docs/architecture.md`) and
    /// the JUnit test results.
    pub fn build(spec: &str, architecture: &str, junit: &str) -> Result<Self, EvidenceError> {
        let milestones = milestones(architecture);
        let tests = test_cases(junit)?;
        let rows = criteria(spec)
            .into_iter()
            .map(|(id, text)| {
                let (milestone, done) = milestones
                    .iter()
                    .find(|(_, ids, _)| ids.contains(&id))
                    .map(|(name, _, done)| (name.clone(), *done))
                    .unwrap_or_default();
                let prefix = format!("{}_", id.to_lowercase());
                let tests: Vec<(String, bool)> = tests
                    .iter()
                    .filter(|(name, _)| name.rsplit("::").next().is_some_and(|f| f.starts_with(&prefix)))
                    .cloned()
                    .collect();
                let status = if tests.iter().any(|(_, passed)| !passed) {
                    Status::Failing
                } else if !done {
                    Status::NotYetDue
                } else if tests.is_empty() {
                    Status::Missing
                } else {
                    Status::Proven
                };
                Row {
                    id,
                    text,
                    milestone,
                    tests,
                    status,
                }
            })
            .collect();
        Ok(Self { rows })
    }

    /// Every gap, one line each; empty when the build may pass. A failing
    /// test is always a gap; a missing test only once its milestone is done.
    pub fn gaps(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter_map(|row| match (row.milestone.is_empty(), row.status) {
                (true, _) => Some(format!("{}: in no milestone", row.id)),
                (_, Status::Missing) => Some(format!("{} ({}): no test", row.id, row.milestone)),
                (_, Status::Failing) => Some(format!("{} ({}): a test failed", row.id, row.milestone)),
                _ => None,
            })
            .collect()
    }

    /// The pack as a self-contained HTML page, naming the commit it was built
    /// from and whether that commit is signed (`None`: not known).
    pub fn html(&self, commit: &str, signed: Option<bool>) -> String {
        let signed = match signed {
            Some(true) => "signed and verified by GitHub",
            Some(false) => "not signed",
            None => "signature not checked",
        };
        let proven = self.rows.iter().filter(|row| row.status == Status::Proven).count();
        let mut page = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>Evidence pack</title><style>{STYLE}</style></head><body>\n\
             <h1>Evidence pack</h1>\n<p>Commit <code>{}</code>, {signed}. \
             {proven} of {} acceptance criteria proven.</p>\n<table>\n\
             <tr><th>Criterion</th><th>Milestone</th><th>Tests</th><th>Status</th></tr>\n",
            escape(commit),
            self.rows.len(),
        );
        for row in &self.rows {
            let tests: Vec<String> = row
                .tests
                .iter()
                .map(|(name, passed)| format!("{} {}", if *passed { "✓" } else { "✗" }, escape(name)))
                .collect();
            page.push_str(&format!(
                "<tr><td><b>{}</b> {}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape(&row.id),
                escape(&row.text),
                escape(&row.milestone),
                tests.join("<br>"),
                row.status.label(),
            ));
        }
        page.push_str("</table>\n</body></html>\n");
        page
    }
}

const STYLE: &str = "body{font:16px/1.5 system-ui,sans-serif;margin:2rem auto;max-width:72rem;padding:0 1rem}\
                     table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ccc;padding:.4rem;\
                     text-align:left;vertical-align:top}code{font-size:.9em}";

/// The spec's criteria: lines such as `- **A1** The generator …`.
fn criteria(spec: &str) -> Vec<(String, String)> {
    spec.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("- **")?;
            let (id, text) = rest.split_once("** ")?;
            is_id(id).then(|| (id.to_owned(), text.trim().to_owned()))
        })
        .collect()
}

fn is_id(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && !chars.as_str().is_empty()
        && chars.all(|c| c.is_ascii_digit())
}

/// The build order's rows: `| M1 | What | A1, B1–B7 | done |` gives the
/// milestone's name, its criteria and whether it is done.
fn milestones(architecture: &str) -> Vec<(String, Vec<String>, bool)> {
    architecture
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let [_, name, _, ids, status, ..] = cells.as_slice() else {
                return None;
            };
            let is_milestone = name.strip_prefix('M').is_some_and(|n| n.parse::<u32>().is_ok());
            is_milestone.then(|| (name.to_string(), expand(ids), status.starts_with("done")))
        })
        .collect()
}

/// `A1, B1–B3` becomes `A1, B1, B2, B3`.
fn expand(ids: &str) -> Vec<String> {
    ids.split(',')
        .map(str::trim)
        .flat_map(|part| {
            let range = part.split_once('–').or_else(|| part.split_once(" to "));
            match range {
                Some((from, to)) if is_id(from.trim()) && is_id(to.trim()) => {
                    let (letter, from, to) = (&from.trim()[..1], &from.trim()[1..], &to.trim()[1..]);
                    match (from.parse::<u32>(), to.parse::<u32>()) {
                        (Ok(from), Ok(to)) => (from..=to).map(|n| format!("{letter}{n}")).collect(),
                        _ => Vec::new(),
                    }
                }
                _ if is_id(part) => vec![part.to_owned()],
                _ => Vec::new(),
            }
        })
        .collect()
}

/// Each test case in a JUnit file, as `classname::name`, and whether it
/// passed: no `failure`, `error` or `skipped` inside it.
fn test_cases(junit: &str) -> Result<Vec<(String, bool)>, EvidenceError> {
    let mut reader = Reader::from_str(junit);
    let mut cases = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|err| EvidenceError::Junit(err.to_string()))?
        {
            Event::Empty(tag) if tag.name().as_ref() == "testcase" => cases.push((name_of(&tag)?, true)),
            Event::Start(tag) if tag.name().as_ref() == "testcase" => cases.push((name_of(&tag)?, true)),
            Event::Start(tag) | Event::Empty(tag) if matches!(tag.name().as_ref(), "failure" | "error" | "skipped") => {
                if let Some(last) = cases.last_mut() {
                    last.1 = false;
                }
            }
            Event::Eof => return Ok(cases),
            _ => {}
        }
    }
}

fn name_of(tag: &quick_xml::events::BytesStart<'_>) -> Result<String, EvidenceError> {
    let attribute = |key: &str| -> Result<String, EvidenceError> {
        match tag
            .try_get_attribute(key)
            .map_err(|err| EvidenceError::Junit(err.to_string()))?
        {
            Some(value) => value
                .normalized_value(quick_xml::XmlVersion::default())
                .map(|value| value.into_owned())
                .map_err(|err| EvidenceError::Junit(err.to_string())),
            None => Ok(String::new()),
        }
    };
    Ok(format!("{}::{}", attribute("classname")?, attribute("name")?))
}
