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
