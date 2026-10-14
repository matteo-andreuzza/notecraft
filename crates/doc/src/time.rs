//! Timestamps and the date/time strings shown under page titles.
//!
//! The document model never reads the clock (the web build traps on `SystemTime::now`, and
//! tests must be deterministic): the engine's host passes "now" in. A [`Timestamp`] is UTC
//! milliseconds since the Unix epoch; [`Timestamp::civil`] converts it to a calendar date with
//! the local UTC offset the host supplies, using Howard Hinnant's public-domain
//! `civil_from_days` algorithm (no time-zone database needed).

use serde::{Deserialize, Serialize};

/// UTC milliseconds since 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

/// Bounds that keep date arithmetic far away from overflow: years −9999…9999. Timestamps from
/// files outside this range are clamped (a hostile `i64::MAX` would otherwise overflow the
/// day arithmetic).
const MIN_MS: i64 = -377_705_116_800_000; // -9999-01-01
const MAX_MS: i64 = 253_402_300_799_999; // 9999-12-31T23:59:59.999

/// A broken-down local date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    /// Year (e.g. 2026).
    pub year: i64,
    /// Month 1–12.
    pub month: u32,
    /// Day of month 1–31.
    pub day: u32,
    /// Hour 0–23.
    pub hour: u32,
    /// Minute 0–59.
    pub minute: u32,
    /// Second 0–59.
    pub second: u32,
    /// Day of week, 0 = Sunday … 6 = Saturday.
    pub weekday: u32,
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

impl Timestamp {
    /// This timestamp clamped into the supported range.
    pub fn clamped(self) -> Timestamp {
        Timestamp(self.0.clamp(MIN_MS, MAX_MS))
    }

    /// The local calendar date and time, `offset_minutes` east of UTC (clamped to ±18 h).
    pub fn civil(self, offset_minutes: i32) -> Civil {
        let off = (offset_minutes.clamp(-18 * 60, 18 * 60) as i64) * 60_000;
        let ms = self.clamped().0.saturating_add(off);
        let days = ms.div_euclid(86_400_000);
        let rem = ms.rem_euclid(86_400_000) / 1000;
        // civil_from_days (H. Hinnant, public domain).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = if m <= 2 { y + 1 } else { y };
        // 1970-01-01 was a Thursday (4).
        let weekday = (days + 4).rem_euclid(7) as u32;
        Civil { year, month: m, day: d, hour: (rem / 3600) as u32, minute: ((rem % 3600) / 60) as u32, second: (rem % 60) as u32, weekday }
    }

    /// The timestamp for a local calendar date and time (inverse of [`Timestamp::civil`];
    /// out-of-range fields are clamped).
    pub fn from_civil(year: i64, month: u32, day: u32, hour: u32, minute: u32, offset_minutes: i32) -> Timestamp {
        let year = year.clamp(-9999, 9999);
        let m = month.clamp(1, 12) as i64;
        let d = day.clamp(1, 31) as i64;
        // days_from_civil (H. Hinnant, public domain).
        let y = if m <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let ms = days * 86_400_000 + (hour.min(23) as i64) * 3_600_000 + (minute.min(59) as i64) * 60_000;
        Timestamp(ms - (offset_minutes.clamp(-18 * 60, 18 * 60) as i64) * 60_000).clamped()
    }

    /// The long date shown under a page title, e.g. `Tuesday, October 14, 2026` (en-US long
    /// date; other locales come with localization in M4).
    pub fn long_date(self, offset_minutes: i32) -> String {
        let c = self.civil(offset_minutes);
        format!(
            "{}, {} {}, {}",
            WEEKDAYS.get(c.weekday as usize).copied().unwrap_or(""),
            MONTHS.get(c.month.saturating_sub(1) as usize).copied().unwrap_or(""),
            c.day,
            c.year
        )
    }

    /// The short date inserted by Insert › Date (`10/14/2026`, en-US).
    pub fn short_date(self, offset_minutes: i32) -> String {
        let c = self.civil(offset_minutes);
        format!("{}/{}/{}", c.month, c.day, c.year)
    }

    /// The time shown under a page title and inserted by Insert › Time (`7:45 PM`, en-US).
    pub fn short_time(self, offset_minutes: i32) -> String {
        let c = self.civil(offset_minutes);
        let (h12, ampm) = match c.hour {
            0 => (12, "AM"),
            1..=11 => (c.hour, "AM"),
            12 => (12, "PM"),
            h => (h - 12, "PM"),
        };
        format!("{h12}:{:02} {ampm}", c.minute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        // 2026-10-14 19:45 UTC was a Wednesday.
        let t = Timestamp::from_civil(2026, 10, 14, 19, 45, 0);
        let c = t.civil(0);
        assert_eq!((c.year, c.month, c.day, c.hour, c.minute, c.weekday), (2026, 10, 14, 19, 45, 3));
        assert_eq!(t.long_date(0), "Wednesday, October 14, 2026");
        assert_eq!(t.short_date(0), "10/14/2026");
        assert_eq!(t.short_time(0), "7:45 PM");
        // The epoch was a Thursday.
        assert_eq!(Timestamp(0).civil(0).weekday, 4);
        // Leap day and offsets across midnight.
        let leap = Timestamp::from_civil(2024, 2, 29, 23, 30, 0);
        assert_eq!(leap.civil(60).month, 3);
        assert_eq!(leap.civil(60).day, 1);
        assert_eq!(Timestamp::from_civil(2026, 1, 1, 0, 5, 0).short_time(0), "12:05 AM");
        assert_eq!(Timestamp::from_civil(2026, 1, 1, 12, 0, 0).short_time(0), "12:00 PM");
    }

    #[test]
    fn round_trip_with_offset() {
        let t = Timestamp::from_civil(1999, 12, 31, 23, 59, 120);
        let c = t.civil(120);
        assert_eq!((c.year, c.month, c.day, c.hour, c.minute), (1999, 12, 31, 23, 59));
    }

    #[test]
    fn hostile_timestamps_do_not_overflow() {
        for ms in [i64::MIN, i64::MAX, -1, 0] {
            let c = Timestamp(ms).civil(i32::MAX);
            assert!((1..=12).contains(&c.month) && (1..=31).contains(&c.day));
            let _ = Timestamp(ms).long_date(i32::MIN);
        }
    }
}
