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

pub struct Performance {
    enabled: bool,
    mode: &'static str,
    started: std::time::Instant,
    frames: u32,
    updates: std::time::Duration,
    backend: f64,
    previous: Option<std::time::Instant>,
    intervals: Vec<f64>,
}
impl Performance {
    pub fn new(enabled: bool, mode: &'static str) -> Self {
        Self {
            enabled,
            mode,
            started: std::time::Instant::now(),
            frames: 0,
            updates: std::time::Duration::ZERO,
            backend: 0.0,
            previous: None,
            intervals: Vec::new(),
        }
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
        if let Some(previous) = self.previous
            && self.intervals.len() < 2048
        {
            self.intervals
                .push(now.duration_since(previous).as_secs_f64() * 1000.0);
        }
        self.previous = Some(now);
        self.frames += 1;
        self.updates += update;
        self.backend += f64::from(frame.info().cpu_usage.unwrap_or_default());
        let elapsed = now.duration_since(self.started).as_secs_f64();
        if elapsed >= 30.0 {
            self.intervals.sort_by(f64::total_cmp);
            let p95 = self
                .intervals
                .get(self.intervals.len() * 95 / 100)
                .copied()
                .unwrap_or_default();
            let atlas = ctx.fonts(|f| f.font_image_size());
            let (layouts, emoji, bytes) = text.cache_stats();
            event(&format!(
                "Performance: mode={} fps={:.1} update_ms={:.3} backend_ms={:.3} interval_p95_ms={:.2} atlas={}x{} text_cache={} emoji_cache={} texture_mib={:.2}",
                self.mode,
                f64::from(self.frames) / elapsed,
                self.updates.as_secs_f64() * 1000.0 / f64::from(self.frames),
                self.backend * 1000.0 / f64::from(self.frames),
                p95,
                atlas[0],
                atlas[1],
                layouts,
                emoji,
                bytes as f64 / 1048576.0
            ));
            self.started = now;
            self.frames = 0;
            self.updates = std::time::Duration::ZERO;
            self.backend = 0.0;
            self.intervals.clear();
        }
    }
}
