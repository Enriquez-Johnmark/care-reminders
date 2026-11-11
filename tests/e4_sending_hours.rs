//! E4: reminders go out only between 8 am and 6 pm on weekdays, clinic
//! time; one outside the window waits for the next.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use care_reminders::sending_hours::{local_day, next_window};
use time::macros::{date, datetime};

const NEW_YORK: &str = "America/New_York";

#[test]
fn e4_inside_the_window_a_reminder_may_go_now() {
    // Wednesday 10:00 in New York (EST, UTC-5).
    let now = datetime!(2026-11-11 15:00 UTC);

    assert_eq!(next_window(now, NEW_YORK).unwrap(), now);
}

#[test]
fn e4_before_eight_or_from_six_it_waits_for_the_next_weekday_morning() {
    // 7:59 am Wednesday → 8:00 am Wednesday.
    assert_eq!(
        next_window(datetime!(2026-11-11 12:59 UTC), NEW_YORK).unwrap(),
        datetime!(2026-11-11 13:00 UTC)
    );
    // 6:00 pm Wednesday → 8:00 am Thursday.
    assert_eq!(
        next_window(datetime!(2026-11-11 23:00 UTC), NEW_YORK).unwrap(),
        datetime!(2026-11-12 13:00 UTC)
    );
}

#[test]
fn e4_on_a_friday_evening_or_a_weekend_it_waits_for_monday_morning() {
    let monday = datetime!(2026-11-16 13:00 UTC);

    assert_eq!(next_window(datetime!(2026-11-13 23:30 UTC), NEW_YORK).unwrap(), monday);
    assert_eq!(next_window(datetime!(2026-11-14 16:00 UTC), NEW_YORK).unwrap(), monday);
    assert_eq!(next_window(datetime!(2026-11-15 20:00 UTC), NEW_YORK).unwrap(), monday);
}

#[test]
fn e4_the_window_follows_the_clinics_clock_across_a_daylight_saving_change() {
    // Clocks go forward on Sunday 2026-03-08: 8 am is 13:00 UTC on Friday
    // (EST) and 12:00 UTC on Monday (EDT).
    assert_eq!(
        next_window(datetime!(2026-03-06 12:00 UTC), NEW_YORK).unwrap(),
        datetime!(2026-03-06 13:00 UTC)
    );
    assert_eq!(
        next_window(datetime!(2026-03-07 15:00 UTC), NEW_YORK).unwrap(),
        datetime!(2026-03-09 12:00 UTC)
    );
}

#[test]
fn e4_the_clinics_day_is_its_own_not_utcs() {
    // 9 pm Wednesday in New York is already Thursday in UTC.
    assert_eq!(
        local_day(datetime!(2026-11-12 02:00 UTC), NEW_YORK).unwrap(),
        date!(2026 - 11 - 11)
    );
}

#[test]
fn e4_an_unknown_time_zone_is_refused() {
    assert!(next_window(datetime!(2026-11-11 15:00 UTC), "Mars/Olympus").is_err());
}
