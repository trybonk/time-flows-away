#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod desktop;
mod diagnostics;
fn main() -> eframe::Result {
    #[cfg(windows)]
    std::panic::set_hook(Box::new(|panic| {
        diagnostics::event(&format!("Application panic: {panic}"))
    }));
    run().inspect_err(|error| diagnostics::event(&format!("Application failed: {error}")))
}

fn run() -> eframe::Result {
    let preview = std::env::args().any(|arg| arg == "--preview");
    #[cfg(windows)]
    if !preview && !std::env::args().any(|arg| arg == "--wallpaper-worker") {
        match desktop::supervisor::run() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(error) => {
                diagnostics::event(&format!("Cannot start wallpaper renderer: {error}"));
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
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration {
        wgpu_setup: setup.into(),
        ..Default::default()
    };
    let default_surface_error = wgpu_options.on_surface_error.clone();
    let last_error = std::sync::Mutex::new(None::<(wgpu::SurfaceError, std::time::Instant)>);
    wgpu_options.on_surface_error = std::sync::Arc::new(move |error| {
        // An occluded surface can fail repeatedly; retain the first event and
        // each new error kind without writing a line on every scheduled tick.
        if let Ok(mut last) = last_error.lock() {
            let now = std::time::Instant::now();
            if last.as_ref().is_none_or(|(previous, time)| {
                *previous != error || now.duration_since(*time).as_secs() >= 10
            }) {
                diagnostics::event(&format!("GPU surface error: {error:?}; frame skipped"));
                *last = Some((error.clone(), now));
            }
        }
        default_surface_error(error)
    });
    diagnostics::event(if preview {
        "Starting preview"
    } else {
        "Starting wallpaper renderer"
    });
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Time Flows Away — Preview")
        .with_inner_size([1100.0, 750.0]);
    #[cfg(windows)]
    let viewport = if preview {
        viewport
    } else {
        // Create on the primary monitor without stealing focus. eframe keeps
        // the window hidden until its first painted frame; attachment preserves it.
        viewport.with_active(false).with_position([0.0, 0.0])
    };
    let options = eframe::NativeOptions {
        viewport,
        #[cfg(windows)]
        window_builder: if preview {
            None
        } else {
            Some(Box::new(desktop::wallpaper_viewport))
        },
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "Time Flows Away",
        options,
        Box::new(move |cc| Ok(Box::new(app::WallpaperApp::new(cc, preview)))),
    )
}
