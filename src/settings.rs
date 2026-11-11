//! Each clinic's reminder settings: its phone number, which every reminder
//! gives (C2), and the tone the model writes in.

use sqlx::PgPool;

use crate::drafting::Tone;
use crate::records::ClinicId;

/// A clinic's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The number patients call; empty until set.
    pub phone: String,
    /// How reminders sound.
    pub tone: Tone,
    /// The clinic's IANA time zone, e.g. `America/New_York`: its sending
    /// hours and its day.
    pub time_zone: String,
}

/// The time zone every clinic starts in.
pub const DEFAULT_TIME_ZONE: &str = "America/New_York";

/// A time zone that isn't in the tz database.
#[derive(Debug, thiserror::Error)]
#[error("{0:?} isn't a time zone; use a name such as America/New_York")]
pub struct UnknownTimeZone(String);

/// `name`, if it is a known IANA time zone.
pub fn time_zone(name: &str) -> Result<String, UnknownTimeZone> {
    let name = name.trim();
    jiff::tz::TimeZone::get(name)
        .map(|_| name.to_owned())
        .map_err(|_| UnknownTimeZone(name.to_owned()))
}

/// `clinic`'s settings; the defaults (no phone, warm) until set.
pub async fn get(pool: &PgPool, clinic: ClinicId) -> Result<Settings, sqlx::Error> {
    let row = sqlx::query!(
        "select phone, tone, time_zone from clinic_settings where tenant_id = $1",
        clinic.as_uuid()
    )
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some(row) => Settings {
            phone: row.phone,
            tone: if row.tone == "plain" { Tone::Plain } else { Tone::Warm },
            time_zone: row.time_zone,
        },
        None => Settings {
            phone: String::new(),
            tone: Tone::Warm,
            time_zone: DEFAULT_TIME_ZONE.to_owned(),
        },
    })
}

/// Sets `clinic`'s phone number.
pub async fn set_phone(pool: &PgPool, clinic: ClinicId, phone: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "insert into clinic_settings (tenant_id, phone) values ($1, $2)
         on conflict (tenant_id) do update set phone = excluded.phone",
        clinic.as_uuid(),
        phone.trim(),
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Sets `clinic`'s time zone, refusing one that isn't known.
pub async fn set_time_zone(pool: &PgPool, clinic: ClinicId, name: &str) -> anyhow::Result<()> {
    let name = time_zone(name)?;
    sqlx::query!(
        "insert into clinic_settings (tenant_id, time_zone) values ($1, $2)
         on conflict (tenant_id) do update set time_zone = excluded.time_zone",
        clinic.as_uuid(),
        name,
    )
    .execute(pool)
    .await?;
    Ok(())
}
