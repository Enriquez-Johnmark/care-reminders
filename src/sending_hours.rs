//! When reminders may go out (E4): 8 am to 6 pm on weekdays, in the clinic's
//! own time zone, daylight saving included.

use jiff::Timestamp;
use jiff::civil::{Time, Weekday};
use jiff::tz::TimeZone;
use time::{Date, OffsetDateTime};

/// The first hour reminders may go out.
pub const OPENS: i8 = 8;
/// The hour from which they may not.
pub const CLOSES: i8 = 18;

/// Why a time couldn't be placed in the clinic's time zone.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct HoursError(String);

fn zone(name: &str) -> Result<TimeZone, HoursError> {
    TimeZone::get(name).map_err(|err| HoursError(format!("unknown time zone {name:?}: {err}")))
}

fn to_jiff(at: OffsetDateTime) -> Result<Timestamp, HoursError> {
    let nanos = at.unix_timestamp_nanos();
    Timestamp::from_nanosecond(nanos).map_err(|err| HoursError(err.to_string()))
}

fn from_jiff(at: Timestamp) -> Result<OffsetDateTime, HoursError> {
    OffsetDateTime::from_unix_timestamp_nanos(at.as_nanosecond()).map_err(|err| HoursError(err.to_string()))
}

/// `now` if a reminder may go out then, else the start of the next window:
/// 8 am on the next weekday that is still to come, clinic time.
pub fn next_window(now: OffsetDateTime, time_zone: &str) -> Result<OffsetDateTime, HoursError> {
    let tz = zone(time_zone)?;
    let local = to_jiff(now)?.to_zoned(tz.clone());
    let weekday = !matches!(local.weekday(), Weekday::Saturday | Weekday::Sunday);
    if weekday && (OPENS..CLOSES).contains(&local.hour()) {
        return Ok(now);
    }
    let mut day = local.date();
    if local.hour() >= OPENS || !weekday {
        day = day.tomorrow().map_err(|err| HoursError(err.to_string()))?;
    }
    while matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday) {
        day = day.tomorrow().map_err(|err| HoursError(err.to_string()))?;
    }
    let opens = day
        .to_datetime(Time::constant(OPENS, 0, 0, 0))
        .to_zoned(tz)
        .map_err(|err| HoursError(err.to_string()))?;
    from_jiff(opens.timestamp())
}

/// The clinic's calendar day at `now`.
pub fn local_day(now: OffsetDateTime, time_zone: &str) -> Result<Date, HoursError> {
    let local = to_jiff(now)?.to_zoned(zone(time_zone)?);
    let day = local.date();
    let month = time::Month::try_from(day.month() as u8).map_err(|err| HoursError(err.to_string()))?;
    Date::from_calendar_date(i32::from(day.year()), month, day.day() as u8).map_err(|err| HoursError(err.to_string()))
}
