//! Frame deadlines and small font atlases, independent of the driver's VSync.
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct FramePacer {
    deadline: Option<Instant>,
    period: Duration,
}
impl FramePacer {
    pub fn schedule(
        &mut self,
        ctx: &egui::Context,
        started: Instant,
        period: Duration,
        probe: Duration,
    ) {
        let deadline = self.next_deadline(started, Instant::now(), period);
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .min(probe);
        // egui subtracts predicted_dt internally. Compensate it explicitly so
        // 60 FPS never becomes an immediate, driver-dependent repaint loop.
        let prediction = Duration::from_secs_f32(ctx.input(|i| i.predicted_dt).max(0.0));
        ctx.request_repaint_after(remaining + prediction);
    }
    fn next_deadline(&mut self, started: Instant, now: Instant, period: Duration) -> Instant {
        let period = period.max(Duration::from_millis(1));
        let mut deadline = match self.deadline {
            Some(deadline) if self.period == period => deadline,
            _ => started + period,
        };
        if deadline <= now {
            let missed = now.duration_since(deadline).as_nanos() / period.as_nanos() + 1;
            deadline += period.saturating_mul(missed.min(u32::MAX as u128) as u32);
        }
        self.period = period;
        self.deadline = Some(deadline);
        deadline
    }
}

pub fn limit_atlas(input: &mut egui::RawInput) {
    // Avoid choosing the driver's 16K maximum for a tiny font atlas.
    // Images also respect this cap and are resized to the physical viewport.
    input.max_texture_side = Some(input.max_texture_side.unwrap_or(4096).min(4096));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadlines_keep_cadence_skip_missed_frames_and_reset_on_rate_change() {
        let t = Instant::now();
        let mut pacer = FramePacer::default();
        let period = Duration::from_nanos(16_666_667);
        assert_eq!(pacer.next_deadline(t, t, period), t + period);
        assert_eq!(
            pacer.next_deadline(t + period, t + period + Duration::from_millis(2), period),
            t + period * 2
        );
        assert_eq!(
            pacer.next_deadline(t + period * 10, t + period * 10, period),
            t + period * 11
        );
        assert_eq!(
            pacer.next_deadline(t, t, Duration::from_secs(2)),
            t + Duration::from_secs(2)
        );
    }
    #[test]
    fn egui_receives_a_nonzero_delay_at_sixty_fps() {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |_| {});
        }
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Duration::ZERO));
        let target = seen.clone();
        ctx.set_request_repaint_callback(move |info| *target.lock().unwrap() = info.delay);
        let mut pacer = FramePacer::default();
        let _ = ctx.run(Default::default(), |ctx| {
            pacer.schedule(
                ctx,
                Instant::now(),
                Duration::from_secs_f64(1.0 / 60.0),
                Duration::from_secs(2),
            )
        });
        assert!(*seen.lock().unwrap() > Duration::from_millis(5));
    }
}
