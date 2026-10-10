use crate::{assets::Assets, options::Options};
use chrono::{Local, Timelike};
use eframe::egui;
use time_flows_away_native::{
    config::{self, Alignment, Config, CountdownMode},
    wallpaper::{clock::next_tick, render},
};
pub struct SettingsApp {
    options: Options,
    owner: usize,
    draft: Config,
    saved: Config,
    assets: Assets,
    message: Option<String>,
    #[cfg(windows)]
    lifetime: Option<crate::platform::SettingsLifetime>,
}
impl SettingsApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        let store = config::Store::new(options.config.clone());
        let mut assets = Assets::new();
        assets.sync(&cc.egui_ctx, &store.current);
        Self {
            #[cfg(windows)]
            lifetime: options
                .wallpaper_owner
                .map(crate::platform::SettingsLifetime::attach),
            owner: crate::platform::owner(cc),
            options,
            draft: store.current.clone(),
            saved: store.current,
            assets,
            message: store.error,
        }
    }
    fn save(&mut self) {
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
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let now = Local::now();
        #[cfg(windows)]
        if self
            .lifetime
            .as_ref()
            .is_some_and(|lifetime| lifetime.finished())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
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
                );
            } else {
                ui.painter()
                    .rect_filled(rect, 0.0, render::color(self.draft.background));
            }
            ui.label("Preview follows your edits. Save when ready to update the wallpaper.");
        });
        ctx.request_repaint_after(next_tick(now.nanosecond()));
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
