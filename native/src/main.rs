mod app;
mod desktop;
fn main() -> eframe::Result {
    let preview = std::env::args().any(|arg| arg == "--preview");
    #[cfg(windows)]
    if !preview && !std::env::args().any(|arg| arg == "--wallpaper-worker") {
        match desktop::supervisor::run() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(error) => {
                eprintln!("Cannot start wallpaper renderer: {error}");
                std::process::exit(1);
            }
        }
    }
    let setup = eframe::egui_wgpu::WgpuSetupCreateNew {
        power_preference: wgpu::PowerPreference::LowPower,
        #[cfg(windows)]
        instance_descriptor: wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        },
        ..Default::default()
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Time Flows Away — Preview")
            .with_inner_size([1100.0, 750.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup.into(),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "Time Flows Away",
        options,
        Box::new(move |cc| Ok(Box::new(app::WallpaperApp::new(cc, preview)))),
    )
}
