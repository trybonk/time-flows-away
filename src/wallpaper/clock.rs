use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use std::time::Duration;
#[derive(Debug, Clone)]
pub struct Clock {
    pub date: String,
    pub time: String,
    /// Calendar fractions: year, month, day, hour, minute.
    pub progress: [f32; 5],
}
impl Clock {
    pub fn at(now: NaiveDateTime) -> Self {
        let d = now.date();
        let next = if d.month() == 12 {
            NaiveDate::from_ymd_opt(d.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1)
        }
        .expect("representable calendar year");
        let month_days = (next - d.with_day(1).expect("first day exists")).num_days() as f64;
        let year_days = if NaiveDate::from_ymd_opt(d.year(), 2, 29).is_some() {
            366.0
        } else {
            365.0
        };
        let second = f64::from(now.second());
        let hour_seconds = f64::from(now.minute()) * 60.0 + second;
        let day_fraction = (f64::from(now.hour()) * 3600.0 + hour_seconds) / 86400.0;
        Self {
            date: now.format("%Y.%m.%d").to_string(),
            time: now.format("%H:%M:%S").to_string(),
            progress: [
                ((f64::from(d.ordinal0()) + day_fraction) / year_days) as f32,
                ((f64::from(d.day0()) + day_fraction) / month_days) as f32,
                day_fraction as f32,
                (hour_seconds / 3600.0) as f32,
                (second / 60.0) as f32,
            ],
        }
    }
}
pub fn next_tick(nanosecond: u32) -> Duration {
    Duration::from_nanos(1_000_000_000 - u64::from(nanosecond.min(999_999_999)))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn at(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> Clock {
        Clock::at(
            NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(h, min, s)
                .unwrap(),
        )
    }
    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.000_001, "{a} != {b}");
    }
    #[test]
    fn new_year_resets() {
        assert_eq!(at(2024, 1, 1, 0, 0, 0).progress, [0.0; 5]);
    }
    #[test]
    fn leap_day_and_month_lengths() {
        let c = at(2024, 2, 29, 12, 0, 0);
        close(c.progress[0], 59.5 / 366.0);
        close(c.progress[1], 28.5 / 29.0);
        close(at(2023, 2, 28, 12, 0, 0).progress[1], 27.5 / 28.0);
        close(at(2024, 4, 30, 12, 0, 0).progress[1], 29.5 / 30.0);
    }
    #[test]
    fn sub_day_and_format() {
        let c = at(2024, 6, 15, 12, 30, 30);
        assert_eq!(c.date, "2024.06.15");
        assert_eq!(c.time, "12:30:30");
        close(c.progress[2], 45030.0 / 86400.0);
        close(c.progress[3], 1830.0 / 3600.0);
        close(c.progress[4], 0.5);
    }
    #[test]
    fn year_is_day_weighted() {
        close(at(2023, 3, 1, 0, 0, 0).progress[0], 59.0 / 365.0);
        close(at(2024, 3, 1, 0, 0, 0).progress[0], 60.0 / 366.0);
    }
    #[test]
    fn december_rollover_and_ranges() {
        for y in [1900, 2000, 2023, 2024] {
            let c = at(y, 12, 31, 23, 59, 59);
            assert!(c.progress.iter().all(|p| *p >= 0.0 && *p < 1.0));
            assert!(c.progress[0] > 0.999);
        }
    }
    #[test]
    fn tick_alignment() {
        assert_eq!(next_tick(0), Duration::from_secs(1));
        assert_eq!(next_tick(250_000_000), Duration::from_millis(750));
        assert_eq!(next_tick(999_999_999), Duration::from_nanos(1));
    }
}
