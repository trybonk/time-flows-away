//! eframe cannot recreate a destroyed root HWND in place. A small supervisor
//! restarts only our renderer process after its shell-owned child is destroyed.
use std::{
    process::{Command, ExitStatus},
    thread,
    time::Duration,
};
pub const SHELL_LOST: i32 = 75;
pub fn run() -> std::io::Result<ExitStatus> {
    let executable = std::env::current_exe()?;
    loop {
        let status = Command::new(&executable)
            .arg("--wallpaper-worker")
            .status()?;
        if status.code() != Some(SHELL_LOST) {
            return Ok(status);
        }
        eprintln!("Explorer destroyed the wallpaper window; restarting renderer in two seconds");
        thread::sleep(Duration::from_secs(2));
    }
}
