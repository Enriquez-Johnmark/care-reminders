//! A3: CI fails if the repository names a real company or holds a real
//! patient record.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::path::Path;

use care_reminders::made_up::problems;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const MARKED: &str = r#"{"resourceType": "Bundle", "meta": {"security": [{"code": "HTEST"}]}, "entry": [
  {"resource": {"resourceType": "Patient", "meta": {"security": [{"code": "HTEST"}]}}}]}"#;

#[test]
fn a3_this_repository_holds_no_real_company_or_patient_record() {
    assert_eq!(problems(Path::new(env!("CARGO_MANIFEST_DIR"))), Vec::<String>::new());
}

#[test]
fn a3_a_fhir_file_not_marked_as_test_data_is_found_anywhere() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "demo/ok.json", MARKED);
    write(root.path(), "notes/real.json", &MARKED.replacen("HTEST", "OTHER", 2));

    assert_eq!(
        problems(root.path()),
        ["notes/real.json: a FHIR resource not marked as made-up test data (HTEST)"]
    );
}

#[test]
fn a3_a_real_company_named_in_code_or_data_is_found() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "src/lab.rs", "// results arrive from Quest Diagnostics\n");

    assert_eq!(
        problems(root.path()),
        ["src/lab.rs: names a real company (Quest Diagnostics)"]
    );
}

#[test]
fn a3_research_notes_may_name_real_companies() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "docs/research/README.md",
        "Epic and Quest Diagnostics sell this.\n",
    );

    assert!(problems(root.path()).is_empty());
}

#[test]
fn a3_a_json_file_that_cant_be_read_is_found_not_passed() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "data.json", r#"{"resourceType": "Patient", "name": "#);

    assert_eq!(
        problems(root.path()),
        ["data.json: not valid JSON, so it can't be checked"]
    );
}

#[test]
fn a3_ndjson_and_xml_fhir_files_are_checked_too() {
    let root = tempfile::tempdir().unwrap();
    let one_line = MARKED.replace('\n', "");
    write(root.path(), "ok.ndjson", &format!("{one_line}\n{one_line}\n"));
    write(
        root.path(),
        "real.ndjson",
        &format!("{one_line}\n{{\"resourceType\": \"Patient\"}}\n"),
    );
    write(
        root.path(),
        "real.xml",
        r#"<Patient xmlns="http://hl7.org/fhir"><id value="p1"/></Patient>"#,
    );

    assert_eq!(
        problems(root.path()),
        [
            "real.ndjson: a FHIR resource not marked as made-up test data (HTEST)",
            "real.xml: a FHIR resource not marked as made-up test data (HTEST)",
        ]
    );
}

#[test]
fn a3_a_configuration_file_with_comments_is_not_a_patient_record() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "web/tsconfig.json",
        "{\n  // comments are allowed here\n  \"compilerOptions\": {}\n}\n",
    );

    assert!(problems(root.path()).is_empty());
}

#[test]
fn a3_only_research_documents_may_name_real_companies() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "docs/research/README.md",
        "Phreesia and Luma Health sell this.\n",
    );
    write(root.path(), "docs/product/brief.md", "Like Phreesia, but approved.\n");

    assert_eq!(
        problems(root.path()),
        ["docs/product/brief.md: names a real company (Phreesia)"]
    );
}
