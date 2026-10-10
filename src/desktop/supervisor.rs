//! eframe cannot recreate a destroyed root HWND in place. A small supervisor
//! restarts only our renderer process after its shell-owned child is destroyed.
use std::{
    process::{Command, ExitStatus},
    thread,
    time::Duration,
};
pub const SHELL_LOST: i32 = 75;
pub fn run(options: &crate::options::Options) -> std::io::Result<ExitStatus> {
    let executable = std::env::current_exe()?;
    let mut options = options.clone();
    options.wallpaper_owner = Some(std::process::id());
    loop {
        if !super::shell_ready() {
            crate::diagnostics::event("Waiting for Explorer desktop before renderer startup");
            let started = std::time::Instant::now();
            while !super::shell_ready() && started.elapsed() < Duration::from_secs(30) {
                thread::sleep(Duration::from_millis(500));
            }
        }
        let mut worker = Command::new(&executable);
        worker.arg("--wallpaper-worker");
        worker.args(options.child_args());
        let status = worker.status()?;
        if status.code() != Some(SHELL_LOST) {
            crate::platform::close_settings();
            return Ok(status);
        }
        crate::diagnostics::event(
            "Explorer destroyed the wallpaper window; restarting renderer in two seconds",
        );
        thread::sleep(Duration::from_secs(2));
    }
}
