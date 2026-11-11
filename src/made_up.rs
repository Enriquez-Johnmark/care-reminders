//! The repository check behind A3: every FHIR file anywhere in the
//! repository is marked as made-up test data, and nothing outside the
//! research documents (`docs/research/`, which cite sources) names a real
//! lab, record-system, reminder or clinic company.

use std::path::Path;

/// Real companies that the code, data and docs must not name: labs, record
/// systems, reminder sellers and large health systems. Matched ignoring case.
const REAL_COMPANIES: &[&str] = &[
    "Quest Diagnostics",
    "Labcorp",
    "Epic Systems",
    "Cerner",
    "Oracle Health",
    "athenahealth",
    "eClinicalWorks",
    "NextGen Healthcare",
    "Allscripts",
    "Veradigm",
    "Kaiser Permanente",
    "Mayo Clinic",
    "Cleveland Clinic",
    "Luma Health",
    "Artera",
    "Phreesia",
    "Relatient",
    "NexHealth",
    "ModMed",
    "Weave Communications",
];

/// Folders the check skips: build output, dependencies and version control.
const SKIPPED: &[&str] = &["target", "node_modules", ".git"];

/// The research documents, which may name real companies as cited sources
/// (decided 2026-11-11).
const RESEARCH: &str = "docs/research/";

/// Every problem found under `root`, one line each, as `path: problem`.
pub fn problems(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    visit(root, root, &mut found);
    found.sort();
    found
}

fn visit(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !SKIPPED.iter().any(|skip| name == *skip) {
                visit(root, &path, found);
            }
            continue;
        }
        let shown = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        // This check and its tests name what they look for.
        if shown == "src/made_up.rs" || shown == "tests/a3_made_up.rs" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(problem) = fhir_problem(&shown, &text) {
            found.push(format!("{shown}: {problem}"));
        }
        let lower = text.to_lowercase();
        if shown.starts_with(RESEARCH) {
            continue;
        }
        if let Some(company) = REAL_COMPANIES.iter().find(|c| lower.contains(&c.to_lowercase())) {
            found.push(format!("{shown}: names a real company ({company})"));
        }
    }
}

const UNMARKED: &str = "a FHIR resource not marked as made-up test data (HTEST)";

/// What's wrong with a file that may hold FHIR resources: JSON, NDJSON (one
/// resource per line) or XML in the FHIR namespace. Each resource must carry
/// the `HTEST` label; a file that can't be read can't be checked.
fn fhir_problem(path: &str, text: &str) -> Option<&'static str> {
    let unmarked = |json: &str| match serde_json::from_str::<serde_json::Value>(json) {
        Ok(json) => (!every_resource_marked(&json)).then_some(UNMARKED),
        // Configuration files such as tsconfig.json allow comments; only
        // unreadable JSON that holds a FHIR resource matters.
        Err(_) if json.contains("resourceType") => Some("not valid JSON, so it can't be checked"),
        Err(_) => None,
    };
    if path.ends_with(".json") {
        unmarked(text)
    } else if path.ends_with(".ndjson") {
        text.lines().filter(|line| !line.trim().is_empty()).find_map(unmarked)
    } else if path.ends_with(".xml") && text.contains("http://hl7.org/fhir") && !text.contains("HTEST") {
        Some(UNMARKED)
    } else {
        None
    }
}

fn every_resource_marked(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            let labelled = map
                .get("meta")
                .and_then(|meta| meta.get("security"))
                .and_then(serde_json::Value::as_array)
                .is_some_and(|labels| labels.iter().any(|label| label["code"] == "HTEST"));
            (!map.contains_key("resourceType") || labelled) && map.values().all(every_resource_marked)
        }
        serde_json::Value::Array(items) => items.iter().all(every_resource_marked),
        _ => true,
    }
}
