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
