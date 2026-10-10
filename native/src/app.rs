use crate::desktop::Desktop;
use chrono::{Local, Timelike};
use eframe::egui;
use time_flows_away_native::wallpaper::{
    clock::{Clock, next_tick},
    render,
};
pub struct WallpaperApp {
    desktop: Option<Desktop>,
    status: Option<String>,
}
impl WallpaperApp {
    pub fn new(cc: &eframe::CreationContext<'_>, preview: bool) -> Self {
        if let Some(state) = &cc.wgpu_render_state {
            let adapter = state.adapter.get_info();
            crate::diagnostics::event(&format!(
                "GPU adapter: name={} backend={:?} driver={} driver_info={} format={:?}",
                adapter.name,
                adapter.backend,
                adapter.driver,
                adapter.driver_info,
                state.target_format
            ));
        }
        // System fonts stay on the user's machine; load once, never each frame.
        for path in system_fonts() {
            if let Ok(bytes) = std::fs::read(path) {
                let mut definitions = egui::FontDefinitions::default();
                definitions.font_data.insert(
                    "system-cjk".into(),
                    egui::FontData::from_owned(bytes).into(),
                );
                definitions
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "system-cjk".into());
                cc.egui_ctx.set_fonts(definitions);
                break;
            }
        }
        if preview {
            return Self {
                desktop: None,
                status: Some("Preview — Esc to close".into()),
            };
        }
        match Desktop::attach(cc) {
            Ok(desktop) => Self {
                desktop: Some(desktop),
                status: None,
            },
            Err(e) => {
                crate::diagnostics::event(&format!("Desktop attachment failed: {e}"));
                Self {
                    desktop: None,
                    status: Some(format!("Preview: {e}. Esc to close")),
                }
            }
        }
    }
}
impl eframe::App for WallpaperApp {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        // The surface clear and the panel must agree, including full opacity.
        // eframe's default is a different, partially transparent dark color.
        render::BACKGROUND.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let now = Local::now();
        if let Some(desktop) = self.desktop.as_mut() {
            if desktop.exit_requested() || ctx.input(|i| i.viewport().close_requested()) {
                desktop.prepare_close();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if let Err(e) = desktop.maintain() {
                crate::diagnostics::event(&format!("Desktop recovery: {e}"));
                self.desktop = None;
                self.status = Some(format!("Preview: {e}. Esc to close"));
            }
        } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        render::draw(ctx, &Clock::at(now.naive_local()));
        if let Some(status) = &self.status {
            egui::Area::new(egui::Id::new("status"))
                .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
                .show(ctx, |ui| {
                    ui.label(status);
                });
        }
        ctx.request_repaint_after(next_tick(now.nanosecond()));
    }
}

fn system_fonts() -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
        let fonts = std::path::PathBuf::from(windir).join("Fonts");
        ["msjhbd.ttc", "msjh.ttc", "msyh.ttc"]
            .iter()
            .map(|name| fonts.join(name))
            .collect()
    }
    #[cfg(not(windows))]
    {
        [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
        .iter()
        .map(std::path::PathBuf::from)
        .collect()
    }
}
