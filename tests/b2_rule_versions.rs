//! B2 (the rules part): each change to a rule makes a new version and keeps
//! the old one; a clinic changes only its own rules.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::rules::{RuleError, RuleSettings, Rules};
use common::{clinic, clock};
use sqlx::PgPool;

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b2_a_new_clinic_gets_the_default_rules_at_version_one(pool: PgPool) {
    let family = clinic(&pool, "Example Family Practice", "family").await;
    let rules = Rules::new(pool.clone(), clock());

    rules.install_defaults(family, "operator").await.unwrap();
    rules.install_defaults(family, "operator").await.unwrap();

    let current = rules.current(family).await.unwrap();
    assert_eq!(current.len(), 4, "installing twice adds nothing");
    assert!(current.iter().all(|rule| rule.version == 1));
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b2_changing_a_rule_makes_a_new_version_and_keeps_the_old_one(pool: PgPool) {
    let family = clinic(&pool, "Example Family Practice", "family").await;
    let rules = Rules::new(pool.clone(), clock());
    rules.install_defaults(family, "operator").await.unwrap();
    let discharge = rules
        .current(family)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name.contains("discharge"))
        .unwrap();
    let changed = RuleSettings::AfterDischarge {
        within_days: 10,
        look_back_days: 14,
    };

    let version = rules
        .change(family, discharge.id, &changed, "admin@family.test")
        .await
        .unwrap();

    assert_eq!(version, 2);
    let now = rules
        .current(family)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == discharge.id)
        .unwrap();
    assert_eq!((now.version, &now.settings), (2, &changed));
    assert_eq!(
        rules.version(family, discharge.id, 1).await.unwrap().unwrap(),
        discharge.settings
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b2_a_rule_keeps_its_type_and_another_clinic_cant_change_it(pool: PgPool) {
    let family = clinic(&pool, "Example Family Practice", "family").await;
    let other = clinic(&pool, "Example Other Practice", "other").await;
    let rules = Rules::new(pool.clone(), clock());
    rules.install_defaults(family, "operator").await.unwrap();
    let discharge = rules
        .current(family)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name.contains("discharge"))
        .unwrap();
    let same_type = RuleSettings::AfterDischarge {
        within_days: 10,
        look_back_days: 14,
    };
    let other_type = RuleSettings::AfterResult {
        tests: vec![],
        within_days: 10,
        look_back_days: 14,
    };

    let wrong_type = rules
        .change(family, discharge.id, &other_type, "admin@family.test")
        .await;
    let wrong_clinic = rules.change(other, discharge.id, &same_type, "admin@other.test").await;

    assert!(matches!(wrong_type, Err(RuleError::OtherType)), "{wrong_type:?}");
    assert!(matches!(wrong_clinic, Err(RuleError::NotFound)), "{wrong_clinic:?}");
    assert_eq!(
        rules.current(family).await.unwrap().iter().map(|r| r.version).max(),
        Some(1)
    );
}
