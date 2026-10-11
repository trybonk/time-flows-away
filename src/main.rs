#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod assets;
mod desktop;
mod diagnostics;
mod options;
mod platform;
mod settings;
#[cfg(windows)]
mod tray;
fn main() -> eframe::Result {
    #[cfg(windows)]
    std::panic::set_hook(Box::new(|panic| {
        diagnostics::event(&format!("Exit: reason=panic details= {panic}"))
    }));
    let result = run()
        .inspect_err(|error| diagnostics::event(&format!("Exit: reason=error details= {error}")));
    if result.is_ok() {
        diagnostics::event("Event loop finished normally");
    }
    result
}

fn run() -> eframe::Result {
    diagnostics::event(&format!(
        "Build: version={} commit={}",
        env!("CARGO_PKG_VERSION"),
        env!("TFA_GIT_COMMIT")
    ));
    let arguments = options::Options::parse(std::env::args_os().skip(1))
        .map_err(|e| eframe::Error::AppCreation(e.into()))?;
    let preview = arguments.preview || arguments.settings;
    let opengl = arguments.opengl;
    #[cfg(windows)]
    let _instance = if !arguments.worker && !arguments.preview {
        match platform::Instance::acquire(arguments.settings)
            .map_err(|e| eframe::Error::AppCreation(e.into()))?
        {
            Some(instance) => Some(instance),
            None => {
                diagnostics::event("Exit: reason=duplicate_instance");
                return Ok(());
            }
        }
    } else {
        None
    };
    #[cfg(windows)]
    if !preview && !arguments.worker {
        match desktop::supervisor::run(&arguments) {
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
    diagnostics::event(if arguments.settings {
        "Starting settings"
    } else if preview {
        "Starting preview"
    } else {
        "Starting wallpaper renderer"
    });
    diagnostics::event(if opengl {
        "Requested renderer: OpenGL"
    } else {
        "Requested renderer: wgpu"
    });
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title(if arguments.settings {
            "Time Flows Away — Settings"
        } else {
            "Time Flows Away — Preview"
        })
        .with_inner_size([1100.0, 750.0])
        .with_min_inner_size(if arguments.settings {
            [800.0, 600.0]
        } else {
            [200.0, 150.0]
        });
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
        renderer: if opengl {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "Time Flows Away",
        options,
        Box::new(move |cc| {
            // Fractional progress widths and supersampled text must not be
            // snapped back to whole pixels before rasterization.
            cc.egui_ctx.tessellation_options_mut(|options| {
                options.feathering = true;
                options.round_rects_to_pixels = false;
                options.round_text_to_pixels = false;
            });
            app::log_adapter(cc);
            if arguments.settings {
                Ok(Box::new(settings::SettingsApp::new(cc, arguments)))
            } else {
                Ok(Box::new(app::WallpaperApp::new(cc, arguments)))
            }
        }),
    )
}
