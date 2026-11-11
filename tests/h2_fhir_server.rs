//! H2 and H3: fetching the clinic's patients from its FHIR server, by
//! request and every hour, within limits, and logging every fetch. A fake
//! FHIR server (wiremock) stands in for the clinic's, so no test needs a
//! network.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::fhir_server::ServerAddress;

#[test]
fn h2_only_an_https_address_or_one_on_this_computer_can_be_set() {
    for allowed in [
        "https://fhir.example.test/baseR4",
        "https://fhir.example.test/baseR4/",
        "http://localhost:8090/fhir",
        "http://127.0.0.1:8090/fhir",
        "http://[::1]:8090/fhir",
    ] {
        let address = ServerAddress::parse(allowed).unwrap_or_else(|err| panic!("{allowed}: {err}"));
        assert!(
            !address.as_str().ends_with('/'),
            "kept without a trailing slash: {address}"
        );
    }
    for refused in [
        "http://fhir.example.test/baseR4",
        "ftp://fhir.example.test",
        "https://user:secret@fhir.example.test/baseR4",
        "https://fhir.example.test/baseR4?_format=json",
        "https://fhir.example.test/baseR4#top",
        "not an address",
        "",
    ] {
        assert!(ServerAddress::parse(refused).is_err(), "{refused} was allowed");
    }
}

mod fetching {
    use std::sync::Arc;
    use std::time::Duration;

    use care_reminders::demo_data::{self, DEMO_DAY};
    use care_reminders::engine::clock::FixedClock;
    use care_reminders::fhir_server::{FetchError, FhirServers, Limits, ServerAddress};
    use care_reminders::records::{ClinicId, Records};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::common::{clinic, clock};

    /// The made-up patient `number` (1-based): their bundle's resources.
    fn resources(number: usize) -> Vec<Value> {
        let bundle: Value = serde_json::from_str(&demo_data::generate(DEMO_DAY)[number - 1].json).unwrap();
        bundle["entry"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["resource"].clone())
            .collect()
    }

    /// A FHIR search answer holding `patients`, each with their care
    /// included, as a FHIR server sends them.
    fn searchset(patients: &[Vec<Value>], next: Option<&str>) -> Value {
        let mut entries = Vec::new();
        for resources in patients {
            for resource in resources {
                let mode = if resource["resourceType"] == "Patient" {
                    "match"
                } else {
                    "include"
                };
                entries.push(json!({ "resource": resource, "search": { "mode": mode } }));
            }
        }
        let mut link = vec![json!({ "relation": "self", "url": "https://fhir.example.test" })];
        if let Some(next) = next {
            link.push(json!({ "relation": "next", "url": next }));
        }
        json!({ "resourceType": "Bundle", "type": "searchset", "link": link, "entry": entries })
    }

    fn fhir(body: &Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/fhir+json")
    }

    /// A fake FHIR server answering the patient search with `answer`.
    async fn server(answer: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fhir/Patient"))
            .and(query_param("_revinclude", "Observation:subject"))
            .respond_with(answer)
            .mount(&server)
            .await;
        server
    }

    async fn connected(pool: &PgPool, server: &MockServer, clock: Arc<FixedClock>) -> (FhirServers, ClinicId) {
        let clinic = clinic(pool, "Example Family Practice", "family").await;
        let servers = FhirServers::new(pool.clone(), clock);
        let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
        servers.set(clinic, &address, "operator").await.unwrap();
        (servers, clinic)
    }

    async fn patients(pool: &PgPool, clinic: ClinicId) -> usize {
        Records::new(pool.clone(), clock()).list(clinic).await.unwrap().len()
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_fetching_saves_every_patient_on_the_server_with_their_care(pool: PgPool) {
        let server = server(fhir(&searchset(&[resources(1), resources(6)], None))).await;
        let (servers, clinic) = connected(&pool, &server, clock()).await;

        let outcome = servers.fetch(clinic, "admin@family.test").await.unwrap();

        assert_eq!(
            (outcome.new, outcome.updated, outcome.unchanged),
            (2, 0, 0),
            "{outcome:?}"
        );
        assert!(outcome.refused.is_empty(), "{outcome:?}");
        let records = Records::new(pool.clone(), clock());
        let felix = records
            .list(clinic)
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.fhir_id == "p06")
            .unwrap();
        let record = records.get(clinic, felix.id).await.unwrap().unwrap();
        assert_eq!((record.results.len(), record.appointments.len()), (1, 1));
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_fetching_follows_the_servers_next_pages_and_adds_nothing_twice(pool: PgPool) {
        let server = MockServer::start().await;
        let second = format!("{}/fhir?page=2", server.uri());
        Mock::given(method("GET"))
            .and(path("/fhir/Patient"))
            .respond_with(fhir(&searchset(&[resources(1)], Some(&second))))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/fhir"))
            .and(query_param("page", "2"))
            .respond_with(fhir(&searchset(&[resources(2)], None)))
            .mount(&server)
            .await;
        let (servers, clinic) = connected(&pool, &server, clock()).await;
        servers.fetch(clinic, "operator").await.unwrap();

        let again = servers.fetch(clinic, "operator").await.unwrap();

        assert_eq!((again.new, again.unchanged), (0, 2), "{again:?}");
        assert_eq!(patients(&pool, clinic).await, 2);
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_a_patient_the_reader_refuses_is_skipped_with_the_reason_and_the_others_are_saved(pool: PgPool) {
        let mut broken = resources(2);
        broken[0]["birthDate"] = json!("sometime");
        let server = server(fhir(&searchset(&[resources(1), broken], None))).await;
        let (servers, clinic) = connected(&pool, &server, clock()).await;

        let outcome = servers.fetch(clinic, "operator").await.unwrap();

        assert_eq!(outcome.new, 1, "{outcome:?}");
        assert_eq!(outcome.refused.len(), 1, "{outcome:?}");
        assert_eq!(outcome.refused[0].patient, "p02");
        assert!(outcome.refused[0].reason.contains("birth date"), "{outcome:?}");
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_an_answer_that_is_slow_large_broken_or_not_fhir_saves_nothing_and_says_why(pool: PgPool) {
        let good = searchset(&[resources(1)], None);
        let limits = Limits {
            timeout: Duration::from_millis(300),
            max_bytes: 1 << 20,
            max_pages: 5,
        };
        let cases = [
            ("slow", fhir(&good).set_delay(Duration::from_secs(2)), "time"),
            ("large", fhir(&json!({ "padding": "x".repeat(2 << 20) })), "large"),
            (
                "html",
                ResponseTemplate::new(200).set_body_raw("<html>Sign in</html>", "text/html"),
                "FHIR",
            ),
            (
                "broken",
                ResponseTemplate::new(200).set_body_raw("{\"resourceType\": ", "application/fhir+json"),
                "FHIR",
            ),
            ("error", ResponseTemplate::new(500), "500"),
            (
                "not a search",
                fhir(&json!({ "resourceType": "Patient", "id": "p" })),
                "FHIR",
            ),
        ];
        for (name, answer, says) in cases {
            let server = server(answer).await;
            let clinic = clinic(&pool, &format!("Clinic {name}"), &name.replace(' ', "-")).await;
            let servers = FhirServers::new(pool.clone(), clock()).with_limits(limits.clone());
            let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
            servers.set(clinic, &address, "operator").await.unwrap();

            let error = servers.fetch(clinic, "operator").await.unwrap_err();

            assert!(error.to_string().contains(says), "{name}: {error}");
            assert_eq!(patients(&pool, clinic).await, 0, "{name}");
        }
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_a_next_page_on_another_server_is_not_followed(pool: PgPool) {
        let server = server(fhir(&searchset(
            &[resources(1)],
            Some("https://elsewhere.example.test/fhir?page=2"),
        )))
        .await;
        let (servers, clinic) = connected(&pool, &server, clock()).await;

        let error = servers.fetch(clinic, "operator").await.unwrap_err();

        assert!(matches!(error, FetchError::OutsideServer(_)), "{error}");
        assert_eq!(patients(&pool, clinic).await, 0);
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_the_hourly_fetch_runs_once_an_hour_has_passed_since_the_last(pool: PgPool) {
        let server = server(fhir(&searchset(&[resources(1)], None))).await;
        let clock = clock();
        let (servers, clinic) = connected(&pool, &server, clock.clone()).await;
        servers.fetch(clinic, "admin@family.test").await.unwrap();

        clock.advance(time::Duration::minutes(59));
        let too_soon = servers.fetch_due().await.unwrap();
        clock.advance(time::Duration::minutes(1));
        let due = servers.fetch_due().await.unwrap();

        assert!(too_soon.is_empty(), "{too_soon:?}");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, clinic);
        assert!(due[0].1.is_ok());
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h3_every_fetch_is_in_the_activity_log_with_who_where_and_how_many(pool: PgPool) {
        let mut broken = resources(2);
        broken[0]["birthDate"] = json!("sometime");
        let server = server(fhir(&searchset(&[resources(1), broken], None))).await;
        let (servers, clinic) = connected(&pool, &server, clock()).await;
        servers.fetch(clinic, "admin@family.test").await.unwrap();
        servers.fetch(clinic, "admin@family.test").await.unwrap();

        let entries: Vec<(String, String, Value)> = sqlx::query_as(
            "select actor, action, details from audit_events
             where tenant_id = $1 and subject_type = 'fhir_server' order by at, id",
        )
        .bind(clinic.as_uuid())
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(entries.len(), 2, "{entries:?}");
        let (actor, action, details) = &entries[1];
        assert_eq!((actor.as_str(), action.as_str()), ("admin@family.test", "fhir.fetched"));
        assert_eq!(details["server"], format!("{}/fhir", server.uri()));
        assert_eq!(
            (details["new"].as_i64(), details["unchanged"].as_i64()),
            (Some(0), Some(1))
        );
        assert_eq!(details["refused"][0]["patient"], "p02", "{details}");
    }

    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h3_a_failed_fetch_is_in_the_activity_log_with_its_reason(pool: PgPool) {
        let server = server(ResponseTemplate::new(503)).await;
        let (servers, clinic) = connected(&pool, &server, clock()).await;
        servers.fetch(clinic, "operator").await.unwrap_err();

        let (action, details): (String, Value) = sqlx::query_as(
            "select action, details from audit_events where tenant_id = $1 and subject_type = 'fhir_server'
             order by at desc, id desc limit 1",
        )
        .bind(clinic.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(action, "fhir.fetch_failed");
        assert!(details["reason"].as_str().unwrap().contains("503"), "{details}");
    }
}

mod made_up_server {
    use std::sync::Arc;

    use care_reminders::demo_data::DEMO_DAY;
    use care_reminders::engine::clock::SystemClock;
    use care_reminders::fhir_server::{FhirServers, ServerAddress, load_made_up};
    use care_reminders::records::Records;
    use sqlx::PgPool;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::common::clinic;

    #[tokio::test]
    async fn the_made_up_patients_load_only_into_a_server_on_this_computer() {
        let public = ServerAddress::parse("https://hapi.fhir.org/baseR4").unwrap();

        let refused = load_made_up(&public, DEMO_DAY).await.unwrap_err();

        assert!(refused.to_string().contains("this computer"), "{refused}");
    }

    #[tokio::test]
    async fn loading_sends_each_made_up_patient_as_one_transaction() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/fhir"))
            .and(body_partial_json(
                serde_json::json!({ "resourceType": "Bundle", "type": "transaction" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/fhir+json"))
            .expect(25)
            .mount(&server)
            .await;
        let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();

        assert_eq!(load_made_up(&address, DEMO_DAY).await.unwrap(), 25);
    }

    /// The real thing, when CI runs HAPI FHIR beside the tests
    /// (`HAPI_FHIR_URL`): load the made-up patients, then fetch them back.
    #[sqlx::test(migrator = "care_reminders::MIGRATOR")]
    async fn h2_fetching_from_hapi_fhir_saves_the_made_up_patients(pool: PgPool) {
        let Ok(url) = std::env::var("HAPI_FHIR_URL") else {
            // CI always runs HAPI FHIR beside the tests: a missing server
            // there is a broken setup, not a test to skip.
            assert!(std::env::var("CI").is_err(), "HAPI_FHIR_URL must be set in CI");
            eprintln!("HAPI_FHIR_URL is not set: no HAPI FHIR server to fetch from");
            return;
        };
        let address = ServerAddress::parse(&url).unwrap();
        let metadata = format!("{address}/metadata");
        for _ in 0..90 {
            if reqwest::get(&metadata)
                .await
                .is_ok_and(|answer| answer.status().is_success())
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        load_made_up(&address, DEMO_DAY).await.unwrap();
        let family = clinic(&pool, "Example Family Practice", "family").await;
        let servers = FhirServers::new(pool.clone(), Arc::new(SystemClock));
        servers.set(family, &address, "operator").await.unwrap();

        servers.fetch(family, "operator").await.unwrap();

        // The server may hold other made-up data too; ours must all be there.
        let saved = Records::new(pool.clone(), Arc::new(SystemClock))
            .list(family)
            .await
            .unwrap();
        for number in 1..=25 {
            let id = format!("p{number:02}");
            assert!(saved.iter().any(|p| p.fhir_id == id), "{id} missing: {saved:?}");
        }
    }
}
