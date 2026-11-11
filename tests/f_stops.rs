//! F1–F4: bookings, replies and opt-outs stop a follow-up's reminders.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::fhir::{Appointment, AppointmentStatus};
use care_reminders::follow_ups::FollowUps;
use care_reminders::records::Records;
use common::outbox::setup;
use common::records;
use sqlx::PgPool;
use time::Duration;
use time::macros::datetime;

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f1_a_booking_found_at_the_next_fetch_withdraws_an_approved_unsent_reminder(pool: PgPool) {
    let s = setup(pool).await;
    // Ana's result follow-up, approved on Saturday so it waits for Monday.
    s.clock.advance(Duration::days(4) + Duration::hours(1));
    let result: uuid::Uuid = sqlx::query_scalar(
        "select r.id from reminders r join follow_ups f on f.id = r.follow_up_id
         join patients p on p.id = f.patient_id where p.fhir_id = 'p01' and f.kind = 'after-result'",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();
    let ana = s
        .drafts()
        .await
        .into_iter()
        .find(|(d, _)| d.reminder.as_uuid() == result)
        .unwrap()
        .0;
    s.approve(&ana).await;
    let mut record = records().remove(0);
    record.appointments.push(Appointment {
        fhir_id: "p01-appointment-9".into(),
        status: AppointmentStatus::Booked,
        kind: "FOLLOWUP".into(),
        start: datetime!(2026-11-20 09:30 UTC),
    });
    let store = Records::new(s.pool.clone(), s.clock.clone());
    store.save(s.coordinator.tenant, &record).await.unwrap();

    FollowUps::new(s.pool.clone(), s.clock.clone())
        .refresh(s.coordinator.tenant, "operator")
        .await
        .unwrap();
    s.clock.advance(Duration::days(2));

    let status: String =
        sqlx::query_scalar("select o.status from reminder_messages m join outbox_messages o on o.id = m.message_id")
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!(s.send().await, 0);
    assert_eq!(
        s.count("select count(*) from audit_events where action = 'reminder.withdrawn'")
            .await,
        1
    );
}

/// Ana's two follow-ups' IDs: her result follow-up, then her screening.
async fn anas_follow_ups(pool: &PgPool) -> (uuid::Uuid, uuid::Uuid) {
    let id = |kind: &'static str| {
        sqlx::query_scalar::<_, uuid::Uuid>(
            "select f.id from follow_ups f join patients p on p.id = f.patient_id
             where p.fhir_id = 'p01' and f.kind = $1",
        )
        .bind(kind)
        .fetch_one(pool)
    };
    (id("after-result").await.unwrap(), id("screening-recall").await.unwrap())
}

async fn stop_of(pool: &PgPool, follow_up: uuid::Uuid) -> Option<String> {
    sqlx::query_scalar("select stop from follow_ups where id = $1")
        .bind(follow_up)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f3_opening_the_opt_out_link_alone_changes_nothing(pool: PgPool) {
    let s = setup(pool).await;
    let (result, _) = anas_follow_ups(&s.pool).await;
    let link = common::sending().opt_out_link(result.into());
    let app = common::app(&s.pool, s.clock.clone());

    let page = common::fetch(&app, "GET", link.trim_start_matches(common::BASE_URL)).await;

    assert_eq!(page.0, 200);
    assert!(
        page.1.contains("<button") && page.1.contains("Example Family Practice"),
        "{}",
        page.1
    );
    assert_eq!(stop_of(&s.pool, result).await, None);
    assert_eq!(s.count("select count(*) from opt_outs").await, 0);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f3_pressing_the_button_opts_out_of_that_kind_of_reminder_only(pool: PgPool) {
    let s = setup(pool).await;
    let (result, screening) = anas_follow_ups(&s.pool).await;
    let link = common::sending().opt_out_link(result.into());
    let app = common::app(&s.pool, s.clock.clone());

    let done = common::fetch(&app, "POST", link.trim_start_matches(common::BASE_URL)).await;
    FollowUps::new(s.pool.clone(), s.clock.clone())
        .refresh(s.coordinator.tenant, "operator")
        .await
        .unwrap();

    assert_eq!(done.0, 200);
    assert!(done.1.contains("stopped"), "{}", done.1);
    assert_eq!(stop_of(&s.pool, result).await.as_deref(), Some("opted-out"));
    assert_eq!(stop_of(&s.pool, screening).await, None, "her screening reminders go on");
    assert_eq!(
        s.count("select count(*) from audit_events where action = 'follow_up.opted_out' and actor = 'patient'")
            .await,
        1
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f4_a_changed_link_or_another_links_code_is_refused(pool: PgPool) {
    let s = setup(pool).await;
    let (result, _) = anas_follow_ups(&s.pool).await;
    let link = common::sending().opt_out_link(result.into());
    let token = link.rsplit('/').next().unwrap().to_owned();
    let mut changed = token.clone().into_bytes();
    changed[3] = if changed[3] == b'0' { b'1' } else { b'0' };
    let changed = String::from_utf8(changed).unwrap();
    // A reply code is signed for another purpose; it isn't an opt-out link.
    let reply = common::sending().reply_to(result.into());
    let reply_code = reply.trim_start_matches("reply+").split('@').next().unwrap().to_owned();
    let app = common::app(&s.pool, s.clock.clone());

    for bad in [changed, reply_code, "nothing".to_owned()] {
        let answer = common::fetch(&app, "POST", &format!("/o/{bad}")).await;
        assert_eq!(answer.0, 404, "{bad}");
    }
    assert_eq!(stop_of(&s.pool, result).await, None);
    assert_eq!(s.count("select count(*) from opt_outs").await, 0);
}

/// A reply from Ana to `to`, as her mail program would send it.
fn anas_reply(to: &str, automatic: bool, id: &str) -> String {
    let auto = if automatic {
        "Auto-Submitted: auto-replied\r\n"
    } else {
        ""
    };
    format!(
        "From: Ana Example <ana@example.test>\r\nTo: {to}\r\nSubject: Re: A message from Example Family Practice\r\n\
         Message-ID: <{id}@example.test>\r\n{auto}Content-Type: text/plain\r\n\r\nThanks, I'll call on Monday.\r\n"
    )
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f2_a_reply_stops_the_follow_up_and_is_shown_to_the_coordinator(pool: PgPool) {
    let s = setup(pool).await;
    let (result, screening) = anas_follow_ups(&s.pool).await;
    let to = common::sending().reply_to(result.into());
    let app = common::app(&s.pool, s.clock.clone());

    let first = common::post_inbound(&app, &anas_reply(&to, false, "r1"), common::INBOUND_SECRET).await;
    let again = common::post_inbound(&app, &anas_reply(&to, false, "r1"), common::INBOUND_SECRET).await;
    let cookie = common::log_in(&app, &s.pool, "carla@family.test").await;
    let shown = common::call(&app, "GET", "/api/replies", None, Some(&cookie)).await;

    assert_eq!((first, again), (204, 204));
    assert_eq!(stop_of(&s.pool, result).await.as_deref(), Some("replied"));
    assert_eq!(
        stop_of(&s.pool, screening).await,
        None,
        "only the reminder replied to stops"
    );
    let replies = shown.body.as_array().unwrap();
    assert_eq!(replies.len(), 1, "a redelivered email is stored once");
    assert_eq!(replies[0]["text"], "Thanks, I'll call on Monday.");
    assert_eq!(replies[0]["follow_up"], result.to_string());
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f2_an_automatic_reply_a_changed_code_or_an_unsigned_post_stops_nothing(pool: PgPool) {
    let s = setup(pool).await;
    let (result, _) = anas_follow_ups(&s.pool).await;
    let to = common::sending().reply_to(result.into());
    // One digit of the code changed.
    let mut changed = to.clone().into_bytes();
    changed[9] = if changed[9] == b'0' { b'1' } else { b'0' };
    let changed = String::from_utf8(changed).unwrap();
    let app = common::app(&s.pool, s.clock.clone());

    let automatic = common::post_inbound(&app, &anas_reply(&to, true, "a1"), common::INBOUND_SECRET).await;
    let wrong_code = common::post_inbound(&app, &anas_reply(&changed, false, "c1"), common::INBOUND_SECRET).await;
    let unsigned = common::post_inbound(&app, &anas_reply(&to, false, "u1"), "not-the-secret").await;

    assert_eq!((automatic, wrong_code, unsigned), (204, 204, 401));
    assert_eq!(stop_of(&s.pool, result).await, None);
    assert_eq!(s.count("select count(*) from replies").await, 0);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f2_the_demos_simulated_reply_answers_the_patients_latest_emailed_reminder(pool: PgPool) {
    let s = setup(pool).await;
    for (draft, patient) in s.drafts().await {
        if patient == "p01" {
            s.approve(&draft).await;
        }
    }
    // One a day: only one of Ana's two reminders goes today.
    assert_eq!(s.send().await, 1);
    let emailed: uuid::Uuid = sqlx::query_scalar(
        "select r.follow_up_id from reminder_messages m join outbox_messages o on o.id = m.message_id
         join reminders r on r.id = m.reminder_id where o.status = 'sent'",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();

    let received = care_reminders::sending::simulate_reply(
        &s.pool,
        s.clock.clone(),
        &common::sending(),
        s.coordinator.tenant,
        "p01",
        "I'll call tomorrow.",
    )
    .await
    .unwrap();

    let stopped: Vec<uuid::Uuid> = sqlx::query_scalar("select id from follow_ups where stop = 'replied'")
        .fetch_all(&s.pool)
        .await
        .unwrap();
    assert_eq!(received, care_reminders::sending::Received::Reply(emailed.into()));
    assert_eq!(stopped, [emailed]);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn f3_a_link_with_the_sentences_full_stop_still_works(pool: PgPool) {
    let s = setup(pool).await;
    let (result, _) = anas_follow_ups(&s.pool).await;
    let link = common::sending().opt_out_link(result.into());
    let app = common::app(&s.pool, s.clock.clone());

    // Some mail programs keep the punctuation after a link in the link.
    for stuck in [".", ",", ")", ".)"] {
        let page = common::fetch(
            &app,
            "GET",
            &format!("{}{stuck}", link.trim_start_matches(common::BASE_URL)),
        )
        .await;
        assert_eq!(page.0, 200, "{stuck}");
    }
}
