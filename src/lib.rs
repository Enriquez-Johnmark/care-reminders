//! Care Reminders: a clinic's patients who are due for follow-up care are
//! found by the clinic's own rules. The model words each reminder, code
//! checks it, a person approves the exact text, the engine sends it once,
//! and every step is audited.

pub mod activity;
pub mod checks;
pub mod demo_data;
pub mod drafting;
pub mod evidence;
pub mod fhir;
pub mod fhir_server;
pub mod follow_ups;
pub mod html;
pub mod made_up;
pub mod model;
pub mod records;
pub mod reminders;
pub mod review;
pub mod rules;
pub mod sending_hours;
pub mod settings;
pub mod web;

/// The engine, Nudgewright, pinned to a release.
pub use nudgewright as engine;

use std::borrow::Cow;
use std::sync::LazyLock;

use sqlx::migrate::Migrator;

/// The engine's migrations, then this project's, run as one set.
///
/// They share sqlx's migrations table, so two separate migrators would each
/// refuse the other's entries. sqlx marks `Migrator`'s fields as exempt from
/// semver; re-check this after upgrading sqlx.
pub static MIGRATOR: LazyLock<Migrator> = LazyLock::new(|| {
    static PROJECT: Migrator = sqlx::migrate!("./migrations");
    let mut migrations: Vec<_> = engine::MIGRATOR.migrations.iter().cloned().collect();
    migrations.extend(PROJECT.migrations.iter().cloned());
    migrations.sort_by_key(|migration| migration.version);
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
});
