/// Release Windows binaries use the GUI subsystem, so keep errors available
/// without creating or attaching a console. No credentials or environment dumps.
pub fn event(message: &str) {
    #[cfg(windows)]
    {
        use std::io::Write;
        let directory = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("TimeFlowsAway");
        if std::fs::create_dir_all(&directory).is_ok()
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join("app.log"))
        {
            let _ = writeln!(
                file,
                "{} pid={} {message}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                std::process::id()
            );
        }
    }
    #[cfg(not(windows))]
    eprintln!("{message}");
}

/// These are UI-update intervals, not SwapBuffers or DWM presentation times.
/// backend_ms is eframe elapsed work INCLUDING its previous App::update.
pub struct Performance {
    enabled: bool,
    mode: &'static str,
    state: &'static str,
    fps: u32,
    high_quality: bool,
    started: std::time::Instant,
    frames: u32,
    updates: std::time::Duration,
    update_max: std::time::Duration,
    backend: f64,
    previous: Option<std::time::Instant>,
    intervals: Vec<f64>,
    interval_max: f64,
    slow20: u32,
    slow25: u32,
    baseline: time_flows_away_native::wallpaper::text::Metrics,
}
fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted
        .get((sorted.len() * percent / 100).min(sorted.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default()
}
impl Performance {
    pub fn new(enabled: bool, mode: &'static str) -> Self {
        Self {
            enabled,
            mode,
            state: "initial",
            fps: 0,
            high_quality: true,
            started: std::time::Instant::now(),
            frames: 0,
            updates: std::time::Duration::ZERO,
            update_max: std::time::Duration::ZERO,
            backend: 0.0,
            previous: None,
            intervals: Vec::new(),
            interval_max: 0.0,
            slow20: 0,
            slow25: 0,
            baseline: Default::default(),
        }
    }
    pub fn state(
        &mut self,
        state: &'static str,
        fps: u32,
        text: &time_flows_away_native::wallpaper::text::TextRenderer,
    ) {
        if !self.enabled {
            return;
        }
        // A configured 60 FPS rate is not an active rendering deadline while idle.
        let fps = if matches!(state, "visible" | "active") {
            fps
        } else {
            0
        };
        if self.state != state || self.fps != fps || self.high_quality != text.high_quality {
            self.reset(std::time::Instant::now());
            self.previous = None;
            self.baseline = text.metrics;
            self.state = state;
            self.fps = fps;
            self.high_quality = text.high_quality;
            event(&format!(
                "Performance mode: mode={} state={state} target_fps={fps} hq={}",
                self.mode, self.high_quality
            ));
        }
    }
    fn reset(&mut self, now: std::time::Instant) {
        self.started = now;
        self.frames = 0;
        self.updates = std::time::Duration::ZERO;
        self.update_max = std::time::Duration::ZERO;
        self.backend = 0.0;
        self.intervals.clear();
        self.interval_max = 0.0;
        self.slow20 = 0;
        self.slow25 = 0;
    }
    pub fn record(
        &mut self,
        ctx: &eframe::egui::Context,
        frame: &eframe::Frame,
        update: std::time::Duration,
        text: &time_flows_away_native::wallpaper::text::TextRenderer,
    ) {
        if !self.enabled {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(previous) = self.previous {
            let interval = now.duration_since(previous).as_secs_f64() * 1000.0;
            // Include idle cadence in quantiles, but never label it a slow 60 Hz frame.
            self.interval_max = self.interval_max.max(interval);
            if self.fps == 60 {
                self.slow20 += u32::from(interval > 20.0);
                self.slow25 += u32::from(interval > 25.0);
            }
            if self.intervals.len() < 4096 {
                self.intervals.push(interval);
            }
        }
        self.previous = Some(now);
        self.frames += 1;
        self.updates += update;
        self.update_max = self.update_max.max(update);
        self.backend += f64::from(frame.info().cpu_usage.unwrap_or_default());
        let elapsed = now.duration_since(self.started).as_secs_f64();
        if elapsed >= 30.0 {
            self.intervals.sort_by(f64::total_cmp);
            let atlas = ctx.fonts(|f| f.font_image_size());
            let (layouts, emoji, bytes) = text.cache_stats();
            let m = text.metrics;
            let b = self.baseline;
            event(&format!(
                "Performance: mode={} state={} target_fps={} fps={:.1} update_ms={:.3} update_max_ms={:.3} backend_ms={:.3} interval_p95_ms={:.2} interval_p99_ms={:.2} interval_max_ms={:.2} slow20={} slow25={} layout_misses={} raster_misses={} layout_total_ms={:.3} raster_total_ms={:.3} filter_total_ms={:.3} upload_total_ms={:.3} atlas={}x{} text_cache={} emoji_cache={} glyph_cache={} texture_mib={:.2}",
                self.mode,
                self.state,
                self.fps,
                f64::from(self.frames) / elapsed,
                self.updates.as_secs_f64() * 1000.0 / f64::from(self.frames),
                self.update_max.as_secs_f64() * 1000.0,
                self.backend * 1000.0 / f64::from(self.frames),
                percentile(&self.intervals, 95),
                percentile(&self.intervals, 99),
                self.interval_max,
                self.slow20,
                self.slow25,
                m.layout_misses - b.layout_misses,
                m.raster_misses - b.raster_misses,
                m.layout_ms - b.layout_ms,
                m.raster_ms - b.raster_ms,
                m.filter_ms - b.filter_ms,
                m.upload_ms - b.upload_ms,
                atlas[0],
                atlas[1],
                layouts,
                emoji,
                text.glyph_count(),
                bytes as f64 / 1048576.0
            ));
            self.baseline = m;
            self.reset(now);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_states_disable_sixty_fps_slow_frame_classification() {
        let text = Default::default();
        let mut performance = Performance::new(true, "wallpaper");
        for state in ["paused", "covered", "suspended", "off", "inactive"] {
            performance.state(state, 60, &text);
            assert_eq!(performance.fps, 0);
        }
        performance.state("visible", 60, &text);
        assert_eq!(performance.fps, 60);
        performance.state("active", 30, &text);
        assert_eq!(performance.fps, 30);
    }
    #[test]
    fn p99_exposes_regular_stalls_hidden_by_p95() {
        let mut samples = vec![16.7; 1770];
        samples.extend([43.0; 30]);
        samples.sort_by(f64::total_cmp);
        assert_eq!(percentile(&samples, 95), 16.7);
        assert_eq!(percentile(&samples, 99), 43.0);
        assert_eq!(percentile(&[], 99), 0.0);
    }
}
