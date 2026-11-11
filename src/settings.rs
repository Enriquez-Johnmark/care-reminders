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
}

/// `clinic`'s settings; the defaults (no phone, warm) until set.
pub async fn get(pool: &PgPool, clinic: ClinicId) -> Result<Settings, sqlx::Error> {
    let row = sqlx::query!(
        "select phone, tone from clinic_settings where tenant_id = $1",
        clinic.as_uuid()
    )
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some(row) => Settings {
            phone: row.phone,
            tone: if row.tone == "plain" { Tone::Plain } else { Tone::Warm },
        },
        None => Settings {
            phone: String::new(),
            tone: Tone::Warm,
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
