//! User settings. Invalid edits never replace the last valid configuration.
use chrono::{NaiveDate, NaiveDateTime, format::Item};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub background: [u8; 3],
    pub background_image: String,
    pub text_color: [u8; 3],
    pub bar_colors: [[u8; 3]; 5],
    pub header: String,
    pub footer: String,
    pub header_size: f32,
    pub date_size: f32,
    pub time_size: f32,
    pub footer_size: f32,
    pub font_path: String,
    pub high_quality_text: bool,
    pub animation_fps: u32,
    pub bar_width: f32,
    pub bar_height: f32,
    pub bar_gap: f32,
    pub horizontal: Alignment,
    pub vertical: Alignment,
    pub offset_x: f32,
    pub offset_y: f32,
    pub date_format: String,
    pub time_format: String,
    pub show_weekday: bool,
    pub panel_color: [u8; 3],
    pub panel_opacity: f32,
    pub countdown_enabled: bool,
    pub countdown_target: String,
    pub countdown_mode: CountdownMode,
    pub countdown_prefix: String,
    pub countdown_suffix: String,
    pub countdown_size: f32,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Start,
    #[default]
    Center,
    End,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CountdownMode {
    #[default]
    Days,
    Detailed,
    Seconds,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            background: [22, 22, 22],
            background_image: String::new(),
            text_color: [200, 200, 200],
            bar_colors: [
                [188, 189, 190],
                [199, 94, 83],
                [255, 253, 93],
                [71, 144, 229],
                [132, 216, 75],
            ],
            header: "时光飞逝".into(),
            footer: "Time flows away".into(),
            header_size: 100.0,
            date_size: 100.0,
            time_size: 100.0,
            footer_size: 90.0,
            font_path: String::new(),
            high_quality_text: true,
            animation_fps: 30,
            bar_width: 530.0,
            bar_height: 10.0,
            bar_gap: 5.0,
            horizontal: Alignment::Center,
            vertical: Alignment::Center,
            offset_x: 0.0,
            offset_y: 0.0,
            date_format: "%Y.%m.%d".into(),
            time_format: "%H:%M:%S".into(),
            show_weekday: false,
            panel_color: [22, 22, 22],
            panel_opacity: 0.6,
            countdown_enabled: false,
            countdown_target: "2030-01-01 00:00:00".into(),
            countdown_mode: CountdownMode::Days,
            countdown_prefix: String::new(),
            countdown_suffix: String::new(),
            countdown_size: 40.0,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported settings version".into());
        }
        if ![0, 30, 60].contains(&self.animation_fps) {
            return Err("Animation rate must be 0, 30 or 60 FPS".into());
        }
        for (name, value, min, max) in [
            ("Header size", self.header_size, 8.0, 300.0),
            ("Date size", self.date_size, 8.0, 300.0),
            ("Time size", self.time_size, 8.0, 300.0),
            ("Footer size", self.footer_size, 8.0, 300.0),
            ("Timer size", self.countdown_size, 8.0, 300.0),
            ("Bar width", self.bar_width, 50.0, 2000.0),
            ("Bar height", self.bar_height, 1.0, 50.0),
            ("Bar gap", self.bar_gap, 0.0, 50.0),
            ("Horizontal offset", self.offset_x, -10000.0, 10000.0),
            ("Vertical offset", self.offset_y, -10000.0, 10000.0),
            ("Panel opacity", self.panel_opacity, 0.0, 1.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("{name} must be between {min} and {max}"));
            }
        }
        for (name, value) in [
            ("Date format", &self.date_format),
            ("Time format", &self.time_format),
        ] {
            let sample = NaiveDate::from_ymd_opt(2026, 10, 10)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap();
            if value.is_empty()
                || value.len() > 128
                || chrono::format::StrftimeItems::new(value).any(|i| matches!(i, Item::Error))
                || sample.format(value).write_to(&mut String::new()).is_err()
            {
                return Err(format!("Invalid {name}"));
            }
        }
        for text in [
            &self.header,
            &self.footer,
            &self.countdown_prefix,
            &self.countdown_suffix,
        ] {
            if text.chars().count() > 200 || text.contains(['\n', '\r']) {
                return Err("Labels must fit on one line (up to 200 characters)".into());
            }
        }
        if self.font_path.len() > 32768 || self.background_image.len() > 32768 {
            return Err("Asset path is too long".into());
        }
        if self.countdown_enabled {
            self.target()?;
        }
        Ok(())
    }
    pub fn target(&self) -> Result<NaiveDateTime, String> {
        NaiveDateTime::parse_from_str(&self.countdown_target, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| {
                NaiveDate::parse_from_str(&self.countdown_target, "%Y-%m-%d")
                    .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
            })
            .map_err(|_| "Timer target: use YYYY-MM-DD or YYYY-MM-DD HH:MM:SS".into())
    }
    pub fn timer_text(&self, now: NaiveDateTime) -> Option<String> {
        if !self.countdown_enabled {
            return None;
        }
        let duration = self.target().ok()? - now;
        let remaining = duration > chrono::Duration::zero();
        // Ceiling for a future target prevents "Elapsed" before the last fraction
        // of a second, and prevents future days falling to zero before midnight.
        let seconds = duration.num_seconds().unsigned_abs()
            + u64::from(remaining && duration.subsec_nanos() > 0);
        let value = match self.countdown_mode {
            CountdownMode::Days => {
                let days = if remaining {
                    seconds.div_ceil(86400)
                } else {
                    seconds / 86400
                };
                format!("{days} {}", if days == 1 { "day" } else { "days" })
            }
            CountdownMode::Seconds => format!("{seconds} seconds"),
            CountdownMode::Detailed => format!(
                "{}d {:02}:{:02}:{:02}",
                seconds / 86400,
                seconds / 3600 % 24,
                seconds / 60 % 60,
                seconds % 60
            ),
        };
        let prefix = if self.countdown_prefix.is_empty() {
            if remaining { "Remaining:" } else { "Elapsed:" }
        } else {
            &self.countdown_prefix
        };
        Some(
            format!("{prefix} {value} {}", self.countdown_suffix)
                .trim()
                .to_owned(),
        )
    }
}
pub fn default_path() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("TimeFlowsAway");
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".config")
        })
        .join("time-flows-away");
    base.join("config.toml")
}
pub fn read(path: &Path) -> Result<Config, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if metadata.len() > 256 * 1024 {
        return Err("Settings file exceeds 256 KiB".into());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let config: Config = toml::from_str(&text).map_err(|e| e.to_string())?;
    config.validate()?;
    Ok(config)
}
pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    use std::io::Write;
    config.validate()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(
        ".settings-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    // Only clean up a temporary file that this call actually created.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(
            toml::to_string_pretty(config)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
pub struct Store {
    pub path: PathBuf,
    pub current: Config,
    pub error: Option<String>,
    stamp: Option<(SystemTime, u64)>,
    probe: Instant,
}
impl Store {
    pub fn next_probe(&self) -> std::time::Duration {
        std::time::Duration::from_secs(2).saturating_sub(self.probe.elapsed())
    }
    pub fn new(path: PathBuf) -> Self {
        let (current, error) = if path.exists() {
            match read(&path) {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(e)),
            }
        } else {
            (Config::default(), None)
        };
        let stamp = stamp(&path);
        Self {
            path,
            current,
            error,
            stamp,
            probe: Instant::now(),
        }
    }
    pub fn refresh(&mut self) -> bool {
        if self.probe.elapsed().as_secs() < 2 {
            return false;
        }
        self.probe = Instant::now();
        let stamp = stamp(&self.path);
        if stamp == self.stamp {
            return false;
        }
        self.stamp = stamp;
        match read(&self.path) {
            Ok(c) => {
                self.current = c;
                self.error = None;
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
}
fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let m = fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_and_partial_settings() {
        let c = Config::default();
        assert_eq!(
            toml::from_str::<Config>(&toml::to_string(&c).unwrap()).unwrap(),
            c
        );
        let c: Config = toml::from_str("header = 'Hello'").unwrap();
        assert_eq!(c.header, "Hello");
        assert_eq!(c.bar_width, 530.0);
        assert!(c.high_quality_text);
        assert_eq!(c.animation_fps, 30);
        c.validate().unwrap();
        assert!(toml::from_str::<Config>("typo = true").is_err());
    }
    #[test]
    fn dangerous_ranges_and_invalid_formats_rejected() {
        let mut c = Config {
            time_format: "%Q".into(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.time_format = "%#z".into();
        assert!(c.validate().is_err());
        c.time_format = "%Z".into();
        assert!(c.validate().is_err());
        c.time_format = "%H:%M".into();
        c.bar_width = f32::NAN;
        assert!(c.validate().is_err());
        c.bar_width = 530.0;
        c.animation_fps = 45;
        assert!(c.validate().is_err());
        c.animation_fps = 60;
        c.validate().unwrap();
        c.version = 2;
        assert!(c.validate().is_err());
        c.version = 1;
        c.countdown_enabled = true;
        c.countdown_target = "2026-02-30".into();
        assert!(c.validate().is_err());
    }
    #[test]
    fn timer_crosses_target_and_rounds_future_days_up() {
        let mut c = Config {
            countdown_enabled: true,
            countdown_target: "2026-10-11".into(),
            ..Default::default()
        };
        let now = NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(23, 59, 59)
            .unwrap();
        assert_eq!(c.timer_text(now).unwrap(), "Remaining: 1 day");
        assert_eq!(
            c.timer_text(now + chrono::Duration::milliseconds(500))
                .unwrap(),
            "Remaining: 1 day"
        );
        c.countdown_mode = CountdownMode::Detailed;
        assert_eq!(c.timer_text(now).unwrap(), "Remaining: 0d 00:00:01");
        assert_eq!(
            c.timer_text(now + chrono::Duration::seconds(2)).unwrap(),
            "Elapsed: 0d 00:00:01"
        );
    }
    #[test]
    fn invalid_save_preserves_existing_file_and_reload_keeps_last_valid() {
        let directory = std::env::temp_dir().join(format!(
            "tfa-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("config.toml");
        let mut c = Config::default();
        save(&path, &c).unwrap();
        c.header = "Saved".into();
        save(&path, &c).unwrap();
        assert_eq!(read(&path).unwrap(), c);
        c.panel_opacity = 2.0;
        assert!(save(&path, &c).is_err());
        assert_eq!(read(&path).unwrap().header, "Saved");
        let mut store = Store::new(path.clone());
        fs::write(&path, "broken [").unwrap();
        store.probe = Instant::now() - std::time::Duration::from_secs(3);
        assert!(!store.refresh());
        assert_eq!(store.current.header, "Saved");
        assert!(store.error.is_some());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken [");
        fs::remove_dir_all(directory).unwrap();
    }
}
