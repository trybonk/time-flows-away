use crate::{assets::Assets, options::Options};
use chrono::Local;
use eframe::egui;
use time_flows_away_native::{
    config::{self, Alignment, Config, CountdownMode},
    wallpaper::{animation::repaint_delay, performance::FramePacer, render},
};
pub struct SettingsApp {
    options: Options,
    owner: usize,
    draft: Config,
    saved: Config,
    assets: Assets,
    pacer: FramePacer,
    performance: crate::diagnostics::Performance,
    message: Option<String>,
    closing: bool,
    #[cfg(windows)]
    startup_probe: std::time::Instant,
    #[cfg(windows)]
    lifetime: Option<crate::platform::SettingsLifetime>,
    #[cfg(windows)]
    startup: Result<crate::platform::autostart::Status, String>,
}
impl SettingsApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        let store = config::Store::new(options.config.clone());
        let mut assets = Assets::for_settings();
        assets.prepare(&cc.egui_ctx, &store.current);
        Self {
            performance: crate::diagnostics::Performance::new(options.performance, "settings"),
            #[cfg(windows)]
            startup: crate::platform::autostart::status(&options),
            #[cfg(windows)]
            lifetime: options
                .wallpaper_owner
                .map(crate::platform::SettingsLifetime::attach),
            owner: crate::platform::owner(cc),
            options,
            draft: store.current.clone(),
            saved: store.current,
            assets,
            pacer: FramePacer::default(),
            message: store.error,
            closing: false,
            #[cfg(windows)]
            startup_probe: std::time::Instant::now(),
        }
    }
    fn save(&mut self) {
        if self.assets.loading() {
            self.message =
                Some("Background is still loading. Save when the preview is ready.".into());
            return;
        }
        if let Some(error) = self.assets.error() {
            self.message = Some(error);
            return;
        }
        match config::save(&self.options.config, &self.draft) {
            Ok(()) => {
                self.saved = self.draft.clone();
                self.message = Some("Saved. Wallpaper updates within two seconds.".into());
            }
            Err(error) => self.message = Some(error),
        }
    }
}
impl eframe::App for SettingsApp {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        time_flows_away_native::wallpaper::performance::limit_atlas(input);
    }
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_started = std::time::Instant::now();
        let now = Local::now();
        #[cfg(windows)]
        if self
            .lifetime
            .as_ref()
            .is_some_and(|lifetime| lifetime.finished())
        {
            if !self.closing {
                crate::diagnostics::event("Exit: reason=owner_exit role=settings");
                self.closing = true;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing {
            crate::diagnostics::event("Exit: reason=window_close role=settings");
            self.closing = true;
        }
        self.performance.state(
            if ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
                "active"
            } else {
                "inactive"
            },
            if ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
                self.draft.animation_fps
            } else {
                0
            },
            &self.assets.text,
        );
        #[cfg(windows)]
        if self.startup_probe.elapsed().as_secs() >= 2 {
            self.startup = crate::platform::autostart::status(&self.options);
            self.startup_probe = std::time::Instant::now();
        }
        self.assets.sync(ctx, &self.draft);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        egui::TopBottomPanel::top("settings-title").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("Time Flows Away");
            ui.horizontal(|ui| {
                if ui.button("Save (Ctrl+S)").clicked() {
                    self.save();
                }
                if ui.button("Reload saved").clicked() {
                    match config::read(&self.options.config) {
                        Ok(c) => {
                            self.saved = c.clone();
                            self.draft = c;
                            self.message = Some("Loaded saved settings.".into());
                        }
                        Err(e) => self.message = Some(e),
                    }
                }
                if ui.button("Reset to defaults").clicked() {
                    self.draft = Config::default();
                    self.message = Some("Defaults restored in preview. Save to apply.".into());
                }
                if self.draft != self.saved {
                    ui.label("Unsaved changes");
                }
            });
            ui.add_space(8.0);
        });
        egui::TopBottomPanel::bottom("settings-status").show(ctx, |ui| {
            if self.assets.loading() {
                ui.small("Loading background…");
            }
            if let Some(error) = self.draft.validate().err().or_else(|| self.assets.error()) {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if let Some(message) = &self.message {
                ui.label(message);
            }
            ui.small(format!("Settings: {}", self.options.config.display()));
            ui.small(
                "Save applies changes to the running wallpaper. Closing discards unsaved changes.",
            );
        });
        egui::SidePanel::left("settings-controls")
            .resizable(true)
            .default_width(380.0)
            .width_range(300.0..=600.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    #[cfg(windows)]
                    egui::CollapsingHeader::new("System").show(ui, |ui| {
                        if let Ok(status) = &self.startup {
                            if let Some(warning)=&status.warning {ui.colored_label(egui::Color32::YELLOW,warning);}
                            let mut enabled=status.enabled;
                            let changed=ui.checkbox(&mut enabled, "Start with Windows").changed();
                            let refresh=status.enabled && (!status.current || status.warning.is_some()) && ui.button("Use current application location").clicked();
                            if changed || refresh {
                                match crate::platform::autostart::set(enabled, &self.options) {
                                    Ok(()) => {
                                        self.startup=crate::platform::autostart::status(&self.options);
                                        self.message=Some("Startup preference updated.".into());
                                    }
                                    Err(error) => self.message=Some(format!("Cannot change startup: {error}")),
                                }
                            }
                        } else if let Err(error)=&self.startup {ui.colored_label(egui::Color32::LIGHT_RED,error);}
                        ui.small("Applied immediately. Keep the EXE in this location. Windows startup settings can also disable it.");
                    });
                    let c = &mut self.draft;
                    egui::CollapsingHeader::new("Text and font")
                        .default_open(true)
                        .show(ui, |ui| {
                            field(ui, "Heading", &mut c.header);
                            field(ui, "Footer", &mut c.footer);
                            rgb(ui, "Text color", &mut c.text_color);
                            for (label, value) in [
                                ("Heading size", &mut c.header_size),
                                ("Date size", &mut c.date_size),
                                ("Time size", &mut c.time_size),
                                ("Footer size", &mut c.footer_size),
                            ] {
                                ui.add(egui::Slider::new(value, 8.0..=300.0).text(label));
                            }
                            asset(ui, "Font file (empty = system default)", &mut c.font_path, true, &mut self.message, self.owner);
                        });
                    egui::CollapsingHeader::new("Progress bars").show(ui, |ui| {
                        for (label, color) in ["Year", "Month", "Day", "Hour", "Minute"]
                            .into_iter().zip(&mut c.bar_colors)
                        {
                            rgb(ui, label, color);
                        }
                        ui.add(egui::Slider::new(&mut c.bar_width, 50.0..=2000.0).text("Width"));
                        ui.add(egui::Slider::new(&mut c.bar_height, 1.0..=50.0).text("Height"));
                        ui.add(egui::Slider::new(&mut c.bar_gap, 0.0..=50.0).text("Gap"));
                    });
                    egui::CollapsingHeader::new("Quality and animation").show(ui, |ui| {
                        ui.checkbox(&mut c.high_quality_text, "High quality text");
                        egui::ComboBox::from_label("Animation").selected_text(match c.animation_fps {
                            0 => "Off (low power)", 60 => "Smooth (60 FPS)", _ => "Smooth (30 FPS)",
                        }).show_ui(ui, |ui| {
                            ui.selectable_value(&mut c.animation_fps, 0, "Off (low power)");
                            ui.selectable_value(&mut c.animation_fps, 30, "Smooth (30 FPS)");
                            ui.selectable_value(&mut c.animation_fps, 60, "Smooth (60 FPS)");
                        });
                        ui.small("Smooth bars use more GPU/CPU. Clock digits change once per second.");
                    });
                    egui::CollapsingHeader::new("Position").show(ui, |ui| {
                        alignment(ui, "Horizontal", &mut c.horizontal, ["Left", "Center", "Right"]);
                        alignment(ui, "Vertical", &mut c.vertical, ["Top", "Center", "Bottom"]);
                        ui.horizontal(|ui| {
                            ui.label("X offset");
                            ui.add(egui::DragValue::new(&mut c.offset_x).range(-10000.0..=10000.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Y offset");
                            ui.add(egui::DragValue::new(&mut c.offset_y).range(-10000.0..=10000.0));
                        });
                        ui.small("Sizes and offsets use logical pixels. The layout shrinks to fit.");
                    });
                    egui::CollapsingHeader::new("Date and time").show(ui, |ui| {
                        field(ui, "Date format", &mut c.date_format);
                        field(ui, "Time format", &mut c.time_format);
                        ui.horizontal(|ui| {
                            if ui.button("24 hours").clicked() {
                                c.time_format = "%H:%M:%S".into();
                            }
                            if ui.button("12 hours").clicked() {
                                c.time_format = "%I:%M:%S %p".into();
                            }
                        });
                        ui.checkbox(&mut c.show_weekday, "Show weekday (English)");
                        ui.small("Formats: %Y year, %m month, %d day, %H / %I hour, %M minute, %S second, %p AM/PM.");
                    });
                    egui::CollapsingHeader::new("Background").show(ui, |ui| {
                        rgb(ui, "Solid color", &mut c.background);
                        asset(ui, "Background image (PNG / JPEG)", &mut c.background_image, false, &mut self.message, self.owner);
                        rgb(ui, "Panel color", &mut c.panel_color);
                        ui.add(egui::Slider::new(&mut c.panel_opacity, 0.0..=1.0).text("Panel opacity"));
                        ui.small("Images fill the screen with centered cropping.");
                    });
                    egui::CollapsingHeader::new("Countdown / elapsed timer").show(ui, |ui| {
                        ui.checkbox(&mut c.countdown_enabled, "Enable timer");
                        field(ui, "Target (local time)", &mut c.countdown_target);
                        ui.small("YYYY-MM-DD or YYYY-MM-DD HH:MM:SS");
                        egui::ComboBox::from_label("Display")
                            .selected_text(match c.countdown_mode {
                                CountdownMode::Days => "Days",
                                CountdownMode::Detailed => "Days + HH:MM:SS",
                                CountdownMode::Seconds => "Seconds",
                            })
                            .show_ui(ui, |ui| {
                                for (value, label) in [
                                    (CountdownMode::Days, "Days"),
                                    (CountdownMode::Detailed, "Days + HH:MM:SS"),
                                    (CountdownMode::Seconds, "Seconds"),
                                ] {
                                    ui.selectable_value(&mut c.countdown_mode, value, label);
                                }
                            });
                        field(ui, "Prefix (empty = automatic)", &mut c.countdown_prefix);
                        field(ui, "Suffix", &mut c.countdown_suffix);
                        ui.add(egui::Slider::new(&mut c.countdown_size, 8.0..=300.0).text("Timer size"));
                        ui.small("Future targets count down; past targets show elapsed time.");
                    });
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Live preview");
            let available = ui.available_size();
            let size = egui::vec2(
                available.x,
                (available.x * 9.0 / 16.0).min(available.y).max(1.0),
            );
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            // Draft input can temporarily be invalid while the user types.
            if self.draft.validate().is_ok() {
                render::draw_in(
                    ui,
                    rect,
                    now.naive_local(),
                    &self.draft,
                    self.assets.texture.as_ref(),
                    &mut self.assets.text,
                    self.assets.animation.sample(
                        now.naive_local(),
                        frame_started,
                        self.draft.animation_fps > 0,
                        false,
                    ),
                );
            } else {
                ui.painter()
                    .rect_filled(rect, 0.0, render::color(self.draft.background));
            }
            ui.label("Preview follows your edits. Save when ready to update the wallpaper.");
        });
        self.pacer.schedule(
            ctx,
            frame_started,
            repaint_delay(
                now.naive_local(),
                if ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
                    self.draft.animation_fps
                } else {
                    0
                },
                false,
            ),
            std::time::Duration::from_secs(2),
        );
        self.performance
            .record(ctx, frame, frame_started.elapsed(), &self.assets.text);
    }
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
}
fn rgb(ui: &mut egui::Ui, label: &str, value: &mut [u8; 3]) {
    ui.horizontal(|ui| {
        ui.color_edit_button_srgb(value);
        ui.label(label);
    });
}
fn asset(
    ui: &mut egui::Ui,
    label: &str,
    path: &mut String,
    font: bool,
    message: &mut Option<String>,
    owner: usize,
) {
    field(ui, label, path);
    ui.horizontal(|ui| {
        if ui.button("Browse…").clicked() {
            match crate::platform::browse(font, owner) {
                Ok(Some(value)) => *path = value,
                Ok(None) => {}
                Err(e) => *message = Some(e),
            }
        }
        if ui.button("Clear").clicked() {
            path.clear();
        }
    });
}
fn alignment(ui: &mut egui::Ui, label: &str, value: &mut Alignment, labels: [&str; 3]) {
    egui::ComboBox::from_label(label)
        .selected_text(match value {
            Alignment::Start => labels[0],
            Alignment::Center => labels[1],
            Alignment::End => labels[2],
        })
        .show_ui(ui, |ui| {
            for (alignment, label) in [Alignment::Start, Alignment::Center, Alignment::End]
                .into_iter()
                .zip(labels)
            {
                ui.selectable_value(value, alignment, label);
            }
        });
}
