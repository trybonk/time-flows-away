//! Monotonic, continuous holding; independent of wallpaper repaint cadence.
use std::time::{Duration, Instant};
#[derive(Default)]
pub(super) struct Hold {
    since: Option<Instant>,
}
impl Hold {
    pub fn update(&mut self, down: bool, now: Instant) -> bool {
        if !down {
            self.since = None;
            return false;
        }
        now.saturating_duration_since(*self.since.get_or_insert(now)) >= Duration::from_secs(1)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_press_release_and_new_instance_never_inherit_hold() {
        let t = Instant::now();
        let mut hold = Hold::default();
        assert!(!hold.update(true, t));
        assert!(!hold.update(true, t + Duration::from_millis(500)));
        assert!(!hold.update(false, t + Duration::from_millis(900)));
        assert!(!hold.update(true, t + Duration::from_secs(1)));
        assert!(hold.update(true, t + Duration::from_secs(2)));
        assert!(!Hold::default().update(true, t + Duration::from_secs(2)));
    }
}
