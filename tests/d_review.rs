//! D1–D6: the care coordinator's review of each reminder.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::sync::Arc;

use care_reminders::engine::accounts::{Role, Session};
use care_reminders::engine::clock::FixedClock;
use care_reminders::reminders::{Drafter, LatestDraft, Reminders};
use care_reminders::review::{ReviewError, Reviews, content_hash};
use common::{Fake, GOOD_DRAFT, clock, practice};
use sqlx::PgPool;

const BLOCKED: &str = "Hello {first_name}, your cholesterol is due. Call {clinic_phone}. {opt_out_link}";

struct Setup {
    pool: PgPool,
    reviews: Reviews,
    coordinator: Session,
    administrator: Session,
}

/// The practice with its first reminders drafted by `model`, and its
/// coordinator and administrator.
async fn setup(pool: PgPool, model: &'static str) -> Setup {
    let clock: Arc<FixedClock> = clock();
    let family = practice(&pool, clock.clone()).await;
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(model))))
        .draft_due()
        .await
        .unwrap();
    care_reminders::settings::set_phone(&pool, family, "(555) 010-0100")
        .await
        .unwrap();
    let reviews = Reviews::new(pool.clone(), clock);
    let member = reviews
        .add_coordinator(family, "carla@family.test", "Carla Reyes", "admin@family.test")
        .await
        .unwrap();
    let owner: uuid::Uuid = sqlx::query_scalar("select id from account_members where role = 'owner'")
        .fetch_one(&pool)
        .await
        .unwrap();
    Setup {
        coordinator: Session {
            tenant: family,
            member,
            email: "carla@family.test".into(),
            role: Role::Staff,
        },
        administrator: Session {
            tenant: family,
            member: owner.into(),
            email: "admin@family.test".into(),
            role: Role::Owner,
        },
        reviews,
        pool,
    }
}

impl Setup {
    async fn first(&self) -> LatestDraft {
        let mut drafts = Reminders::new(self.pool.clone())
            .latest_drafts(self.coordinator.tenant)
            .await
            .unwrap();
        drafts.sort_by_key(|d| d.reminder.as_uuid());
        drafts.remove(0)
    }
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d1_only_the_coordinator_can_approve_and_the_administrators_approval_is_refused(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;
    let hash = content_hash(&draft.text);

    let by_admin = s
        .reviews
        .approve(&s.administrator, draft.reminder, draft.draft, &hash)
        .await;
    let by_coordinator = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &hash)
        .await;

    assert!(matches!(by_admin, Err(ReviewError::NotACoordinator)), "{by_admin:?}");
    assert!(by_coordinator.is_ok(), "{by_coordinator:?}");
    let approval = s
        .reviews
        .current_approval(s.coordinator.tenant, draft.reminder)
        .await
        .unwrap();
    assert_eq!(approval.unwrap().approved_by, s.coordinator.member);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d2_a_blocked_draft_cant_be_approved(pool: PgPool) {
    let s = setup(pool, BLOCKED).await;
    let draft = s.first().await;

    let approved = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await;

    assert!(matches!(approved, Err(ReviewError::Blocked)), "{approved:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d3_an_approval_binds_the_exact_text_and_a_later_change_needs_approving_again(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;

    let wrong_text = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash("other text"))
        .await;
    s.reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await
        .unwrap();
    let edited = GOOD_DRAFT.replace("It's time", "Now is a good time");
    s.reviews.edit(&s.coordinator, draft.reminder, &edited).await.unwrap();

    assert!(matches!(wrong_text, Err(ReviewError::Changed)), "{wrong_text:?}");
    let current = s
        .reviews
        .current_approval(s.coordinator.tenant, draft.reminder)
        .await
        .unwrap();
    assert!(current.is_none(), "the edit needs approving again");
    let stale = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await;
    assert!(matches!(stale, Err(ReviewError::Changed)), "{stale:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d4_a_coordinators_edit_goes_through_the_same_checks(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;

    let clinical = s
        .reviews
        .edit(
            &s.coordinator,
            draft.reminder,
            &format!("{GOOD_DRAFT} It's about your cholesterol."),
        )
        .await
        .unwrap();
    let digit = s
        .reviews
        .edit(&s.coordinator, draft.reminder, &format!("{GOOD_DRAFT} Come at 3."))
        .await
        .unwrap();

    assert!(clinical.blocked && digit.blocked);
    let latest = s.first().await;
    assert_eq!(latest.source, "coordinator");
    let approved = s
        .reviews
        .approve(
            &s.coordinator,
            latest.reminder,
            latest.draft,
            &content_hash(&latest.text),
        )
        .await;
    assert!(matches!(approved, Err(ReviewError::Blocked)), "{approved:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d5_the_clinics_template_can_be_approved_instead_of_a_blocked_draft(pool: PgPool) {
    let s = setup(pool, BLOCKED).await;
    let draft = s.first().await;

    s.reviews.use_template(&s.coordinator, draft.reminder).await.unwrap();

    let latest = s.first().await;
    assert_eq!(latest.source, "template");
    s.reviews
        .approve(
            &s.coordinator,
            latest.reminder,
            latest.draft,
            &content_hash(&latest.text),
        )
        .await
        .unwrap();
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d5_two_new_drafts_can_be_asked_for_and_a_third_is_refused(pool: PgPool) {
    let s = setup(pool, BLOCKED).await;
    let draft = s.first().await;
    let model = Fake(Some(GOOD_DRAFT));

    s.reviews
        .new_draft(&s.coordinator, draft.reminder, Some(&model))
        .await
        .unwrap();
    s.reviews
        .new_draft(&s.coordinator, draft.reminder, Some(&model))
        .await
        .unwrap();
    let third = s.reviews.new_draft(&s.coordinator, draft.reminder, Some(&model)).await;

    assert!(matches!(third, Err(ReviewError::NoMoreDrafts)), "{third:?}");
    let latest = s.first().await;
    assert!(!latest.blocked() && latest.source == "model", "{latest:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d6_a_skipped_reminder_is_finished_with_its_reason_and_cant_be_approved(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;

    let no_reason = s.reviews.skip(&s.coordinator, draft.reminder, "  ").await;
    s.reviews
        .skip(&s.coordinator, draft.reminder, "Called the patient instead")
        .await
        .unwrap();

    assert!(matches!(no_reason, Err(ReviewError::NoReason)), "{no_reason:?}");
    let (outcome, reason): (String, String) =
        sqlx::query_as("select outcome, skip_reason from reminders where id = $1")
            .bind(draft.reminder.as_uuid())
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(
        (outcome.as_str(), reason.as_str()),
        ("skipped", "Called the patient instead")
    );
    let approved = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await;
    assert!(matches!(approved, Err(ReviewError::Finished)), "{approved:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn a4_a_coordinator_cant_review_another_clinics_reminder(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;
    let other = common::clinic(&s.pool, "Example Other Practice", "other").await;
    let member = s
        .reviews
        .add_coordinator(other, "dan@other.test", "Dan Cruz", "admin@other.test")
        .await
        .unwrap();
    let outsider = Session {
        tenant: other,
        member,
        email: "dan@other.test".into(),
        role: Role::Staff,
    };

    let approved = s
        .reviews
        .approve(&outsider, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await;
    let skipped = s.reviews.skip(&outsider, draft.reminder, "no").await;

    assert!(matches!(approved, Err(ReviewError::NotFound)), "{approved:?}");
    assert!(matches!(skipped, Err(ReviewError::NotFound)), "{skipped:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d3_a_reminder_that_cant_be_filled_cant_be_approved(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;
    // This clinic hasn't set its phone, so {clinic_phone} can't be filled.
    sqlx::query("delete from clinic_settings")
        .execute(&s.pool)
        .await
        .unwrap();

    let approved = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await;

    assert!(matches!(approved, Err(ReviewError::CantFill(_))), "{approved:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d6_a_stopped_follow_ups_reminder_cant_be_approved_or_changed(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;
    let draft = s.first().await;
    sqlx::query("update follow_ups set stopped_at = now(), stop = 'booked'")
        .execute(&s.pool)
        .await
        .unwrap();

    let approved = s
        .reviews
        .approve(&s.coordinator, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await;
    let edited = s.reviews.edit(&s.coordinator, draft.reminder, GOOD_DRAFT).await;

    assert!(matches!(approved, Err(ReviewError::Stopped)), "{approved:?}");
    assert!(matches!(edited, Err(ReviewError::Stopped)), "{edited:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d5_with_no_model_a_new_draft_is_refused_rather_than_another_template(pool: PgPool) {
    let s = setup(pool, BLOCKED).await;
    let draft = s.first().await;

    let asked = s.reviews.new_draft::<Fake>(&s.coordinator, draft.reminder, None).await;

    assert!(matches!(asked, Err(ReviewError::NoModel)), "{asked:?}");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d1_a_coordinator_needs_a_name_and_a_bad_one_adds_no_one(pool: PgPool) {
    let s = setup(pool, GOOD_DRAFT).await;

    let added = s
        .reviews
        .add_coordinator(s.coordinator.tenant, "dan@family.test", "  ", "admin@family.test")
        .await;

    assert!(matches!(added, Err(ReviewError::BadName)), "{added:?}");
    let members: i64 = sqlx::query_scalar("select count(*) from account_members where email = 'dan@family.test'")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(members, 0);
}
