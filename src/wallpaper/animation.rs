//! The original CSS interpolates each clock sample over one second.
use super::clock::Clock;
use chrono::{NaiveDateTime, Timelike};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct ProgressAnimation {
    state: Option<State>,
}
struct State {
    second: NaiveDateTime,
    started: Instant,
    from: [f32; 5],
    target: [f32; 5],
    paused: bool,
}
impl State {
    fn value(&self, instant: Instant) -> [f32; 5] {
        let t = instant
            .saturating_duration_since(self.started)
            .as_secs_f32()
            .min(1.0);
        std::array::from_fn(|i| self.from[i] + (self.target[i] - self.from[i]) * t)
    }
}
impl ProgressAnimation {
    pub fn sample(
        &mut self,
        now: NaiveDateTime,
        instant: Instant,
        smooth: bool,
        paused: bool,
    ) -> [f32; 5] {
        let second = now.with_nanosecond(0).unwrap();
        let target = Clock::at(second).progress;
        let Some(state) = self.state.as_mut() else {
            self.state = Some(State {
                second,
                started: instant,
                from: target,
                target,
                paused,
            });
            return target;
        };
        if paused && !state.paused {
            let frozen = if smooth {
                state.value(instant)
            } else {
                state.target
            };
            state.from = frozen;
            state.target = frozen;
        } else if state.paused && !paused {
            // Resume and long sleep/time jumps show current time immediately.
            state.from = target;
            state.target = target;
            state.second = second;
            state.started = instant;
        } else if !paused && second != state.second {
            let consecutive = second - state.second == chrono::Duration::seconds(1)
                && instant.saturating_duration_since(state.started) < Duration::from_secs(3);
            state.from = if smooth && consecutive {
                state.value(instant)
            } else {
                target
            };
            state.target = target;
            state.second = second;
            state.started = instant;
        }
        state.paused = paused;
        if !smooth && !paused {
            state.from = target;
            state.target = target;
        }
        if smooth {
            state.value(instant)
        } else {
            state.target
        }
    }
}

pub fn repaint_delay(now: NaiveDateTime, fps: u32, paused: bool) -> Duration {
    if paused {
        Duration::from_secs(2)
    } else if fps == 0 {
        super::clock::next_tick(now.nanosecond())
    } else {
        Duration::from_secs_f64(1.0 / f64::from(fps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn time(second: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(12, 30, second)
            .unwrap()
    }
    #[test]
    fn bar_changes_between_ticks_and_rollover_rewinds_smoothly() {
        let mut a = ProgressAnimation::default();
        let start = Instant::now();
        let first = a.sample(time(58), start, true, false);
        a.sample(time(59), start + Duration::from_secs(1), true, false);
        let middle = a.sample(time(59), start + Duration::from_millis(1500), true, false);
        assert!(middle[4] > first[4] && middle[4] < 59.0 / 60.0);
        let rollover = time(59) + chrono::Duration::seconds(1);
        let before = a.sample(rollover, start + Duration::from_secs(2), true, false);
        let rewind = a.sample(rollover, start + Duration::from_millis(2500), true, false);
        assert!(rewind[4] > 0.0 && rewind[4] < before[4]);
    }
    #[test]
    fn pause_freezes_current_frame_resume_and_time_jump_snap() {
        let mut a = ProgressAnimation::default();
        let start = Instant::now();
        a.sample(time(10), start, true, false);
        a.sample(time(11), start + Duration::from_secs(1), true, false);
        let frozen = a.sample(time(11), start + Duration::from_millis(1500), true, true);
        assert_eq!(
            frozen,
            a.sample(time(11), start + Duration::from_secs(5), true, true)
        );
        assert_eq!(
            a.sample(time(20), start + Duration::from_secs(6), true, false),
            Clock::at(time(20)).progress
        );
        assert_eq!(
            a.sample(time(50), start + Duration::from_secs(36), true, false),
            Clock::at(time(50)).progress
        );
        assert_eq!(
            a.sample(time(51), start + Duration::from_secs(37), false, false),
            Clock::at(time(51)).progress
        );
    }
}
