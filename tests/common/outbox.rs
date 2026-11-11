//! The practice ready to send: its first reminders drafted from the
//! template, a coordinator, and a fake mail provider.

use std::sync::Arc;

use care_reminders::engine::accounts::{Role, Session};
use care_reminders::engine::clock::{Clock, FixedClock};
use care_reminders::engine::mail::MailpitMailer;
use care_reminders::engine::outbox::Sender;
use care_reminders::reminders::{Drafter, LatestDraft, Reminders};
use care_reminders::review::{Reviews, content_hash};
use care_reminders::sending::{SendingRules, finish};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub struct Setup {
    pub pool: PgPool,
    pub clock: Arc<FixedClock>,
    pub reviews: Reviews,
    pub coordinator: Session,
    pub mailpit: MockServer,
}

/// The practice with its first reminders drafted from the template, at 10 am
/// on Tuesday 10 November in New York, and a fake mail provider.
pub async fn setup(pool: PgPool) -> Setup {
    let clock = super::clock();
    clock.advance(Duration::hours(6));
    let family = super::practice(&pool, clock.clone()).await;
    care_reminders::settings::set_phone(&pool, family, "(555) 010-0100")
        .await
        .unwrap();
    Drafter::<super::Fake>::new(pool.clone(), clock.clone(), None)
        .draft_due()
        .await
        .unwrap();
    let reviews = Reviews::new(pool.clone(), clock.clone()).with_sending(super::sending());
    let member = reviews
        .add_coordinator(family, "carla@family.test", "Carla Reyes", "admin@family.test")
        .await
        .unwrap();
    let mailpit = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ID": "mp_1" })))
        .mount(&mailpit)
        .await;
    Setup {
        coordinator: Session {
            tenant: family,
            member,
            email: "carla@family.test".into(),
            role: Role::Staff,
        },
        pool,
        clock,
        reviews,
        mailpit,
    }
}

impl Setup {
    pub async fn drafts(&self) -> Vec<(LatestDraft, String)> {
        let drafts = Reminders::new(self.pool.clone())
            .latest_drafts(self.coordinator.tenant)
            .await
            .unwrap();
        let mut named = Vec::new();
        for draft in drafts {
            let name: String = sqlx::query_scalar(
                "select p.fhir_id from reminders r join follow_ups f on f.id = r.follow_up_id
                 join patients p on p.id = f.patient_id where r.id = $1",
            )
            .bind(draft.reminder.as_uuid())
            .fetch_one(&self.pool)
            .await
            .unwrap();
            named.push((draft, name));
        }
        named.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.reminder.as_uuid().cmp(&b.0.reminder.as_uuid())));
        named
    }

    pub async fn approve(&self, draft: &LatestDraft) {
        self.reviews
            .approve(
                &self.coordinator,
                draft.reminder,
                draft.draft,
                &content_hash(&draft.text),
            )
            .await
            .unwrap();
    }

    /// Sends what is due, as the server does, then marks what went out.
    pub async fn send(&self) -> usize {
        let as_clock: Arc<dyn Clock> = self.clock.clone();
        let mailer = MailpitMailer::new(&self.mailpit.uri(), "reminders@family.example.test").unwrap();
        let rules = Arc::new(SendingRules::new(self.pool.clone(), as_clock.clone()));
        finish(&self.pool, as_clock.clone()).await.unwrap();
        let report = Sender::new(self.pool.clone(), mailer, as_clock.clone(), rules)
            .drain_once(100)
            .await
            .unwrap();
        finish(&self.pool, as_clock).await.unwrap();
        report.sent
    }

    /// The bodies the fake mail provider received.
    pub async fn sent_bodies(&self) -> Vec<String> {
        self.mailpit
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                body["Text"].as_str().unwrap().to_owned()
            })
            .collect()
    }

    pub async fn count(&self, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }
}
