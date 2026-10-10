use crate::{assets::Assets, desktop::Desktop, options::Options};
use chrono::{Local, NaiveDateTime, Timelike};
use eframe::egui;
use time_flows_away_native::{
    config::Store,
    wallpaper::{clock::next_tick, render},
};
pub struct WallpaperApp {
    desktop: Option<Desktop>,
    status: Option<String>,
    store: Store,
    assets: Assets,
    paused: Option<NaiveDateTime>,
    #[cfg(windows)]
    options: Options,
    #[cfg(windows)]
    tray: Option<crate::tray::Tray>,
    #[cfg(windows)]
    settings: Option<std::process::Child>,
}
impl WallpaperApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        let store = Store::new(options.config.clone());
        if let Some(error) = &store.error {
            crate::diagnostics::event(&format!("Invalid settings; using defaults: {error}"));
        }
        let mut assets = Assets::new();
        assets.sync(&cc.egui_ctx, &store.current);
        if let Some(error) = assets.error() {
            crate::diagnostics::event(&format!("Asset fallback: {error}"));
        }
        let (desktop, status) = if options.preview {
            (None, Some("Preview — Esc to close; Space to pause".into()))
        } else {
            match Desktop::attach(cc) {
                Ok(desktop) => (Some(desktop), None),
                Err(error) => {
                    crate::diagnostics::event(&format!("Desktop attachment failed: {error}"));
                    (None, Some(format!("Preview: {error}. Esc to close")))
                }
            }
        };
        #[cfg(windows)]
        let tray = if desktop.is_some() {
            match crate::tray::Tray::new(&cc.egui_ctx) {
                Ok(tray) => Some(tray),
                Err(error) => {
                    crate::diagnostics::event(&format!("Cannot create tray: {error}"));
                    None
                }
            }
        } else {
            None
        };
        Self {
            desktop,
            status,
            store,
            assets,
            paused: None,
            #[cfg(windows)]
            options,
            #[cfg(windows)]
            tray,
            #[cfg(windows)]
            settings: None,
        }
    }
    fn close(&mut self, ctx: &egui::Context) {
        if let Some(desktop) = self.desktop.as_mut() {
            desktop.prepare_close();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn pause(&mut self, now: NaiveDateTime) {
        self.paused = if self.paused.is_some() {
            None
        } else {
            Some(now)
        };
        crate::diagnostics::event(if self.paused.is_some() {
            "Wallpaper paused"
        } else {
            "Wallpaper resumed"
        });
        #[cfg(windows)]
        if let Some(tray) = &self.tray {
            tray.paused(self.paused.is_some());
        }
    }
    #[cfg(windows)]
    fn open_settings(&mut self) {
        // Reap a completed child and also find an existing settings window after
        // an Explorer restart, when the new worker has no child handle for it.
        if let Some(child) = self.settings.as_mut() {
            let _ = child.try_wait();
        }
        if crate::platform::focus_settings() {
            return;
        }
        let result = std::env::current_exe().and_then(|exe| {
            std::process::Command::new(exe)
                .arg("--settings")
                .args(self.options.child_args())
                .spawn()
        });
        match result {
            Ok(child) => self.settings = Some(child),
            Err(error) => crate::diagnostics::event(&format!("Cannot open settings: {error}")),
        }
    }
}
impl eframe::App for WallpaperApp {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        render::color(self.store.current.background).to_normalized_gamma_f32()
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let now = Local::now();
        #[cfg(windows)]
        if let Some(tray) = &self.tray {
            for command in tray.poll() {
                match command {
                    crate::tray::Command::Settings => self.open_settings(),
                    crate::tray::Command::TogglePause => self.pause(now.naive_local()),
                    crate::tray::Command::Quit => self.close(ctx),
                }
            }
        }
        if let Some(desktop) = self.desktop.as_mut() {
            if desktop.exit_requested() || ctx.input(|i| i.viewport().close_requested()) {
                self.close(ctx);
                return;
            }
            if let Err(error) = desktop.maintain() {
                crate::diagnostics::event(&format!("Desktop recovery: {error}"));
                self.desktop = None;
                self.status = Some(format!("Preview: {error}. Esc to close"));
                #[cfg(windows)]
                {
                    self.tray = None;
                }
            }
        } else {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.close(ctx);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.pause(now.naive_local());
            }
        }
        let previous_error = self.store.error.clone();
        if self.store.refresh() {
            crate::diagnostics::event("Settings reloaded");
        }
        if self.store.error != previous_error
            && let Some(error) = &self.store.error
        {
            crate::diagnostics::event(&format!("Settings reload ignored: {error}"));
        }
        let previous_asset_error = self.assets.error();
        self.assets.sync(ctx, &self.store.current);
        if self.assets.error() != previous_asset_error
            && let Some(error) = self.assets.error()
        {
            crate::diagnostics::event(&format!("Asset fallback: {error}"));
        }
        render::draw(
            ctx,
            self.paused.unwrap_or(now.naive_local()),
            &self.store.current,
            self.assets.texture.as_ref(),
        );
        if let Some(status) = &self.status {
            egui::Area::new(egui::Id::new("status"))
                .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
                .show(ctx, |ui| {
                    ui.label(status);
                    if self.paused.is_some() {
                        ui.label("Paused");
                    }
                });
        }
        let next_frame = if self.paused.is_some() {
            std::time::Duration::from_secs(2)
        } else {
            next_tick(now.nanosecond())
        };
        // Keep config polling on its own deadline; rounding it to clock ticks
        // can otherwise turn the two-second interval into three seconds.
        ctx.request_repaint_after(next_frame.min(self.store.next_probe()));
    }
}
pub fn log_adapter(cc: &eframe::CreationContext<'_>) {
    if let Some(state) = &cc.wgpu_render_state {
        let adapter = state.adapter.get_info();
        crate::diagnostics::event(&format!(
            "GPU adapter: name={} backend={:?} driver={} driver_info={} format={:?}",
            adapter.name, adapter.backend, adapter.driver, adapter.driver_info, state.target_format
        ));
    }
    if let Some(gl) = &cc.gl {
        use eframe::glow::HasContext;
        unsafe {
            crate::diagnostics::event(&format!(
                "OpenGL adapter: renderer={} vendor={} version={}",
                gl.get_parameter_string(eframe::glow::RENDERER),
                gl.get_parameter_string(eframe::glow::VENDOR),
                gl.get_parameter_string(eframe::glow::VERSION)
            ));
        }
    }
}
