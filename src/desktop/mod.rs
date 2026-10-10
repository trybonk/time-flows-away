#[cfg(any(windows, test))]
mod order;
#[cfg(windows)]
pub mod supervisor;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{Desktop, shell_ready, wallpaper_viewport};
#[cfg(not(windows))]
pub struct Desktop;
#[cfg(not(windows))]
impl Desktop {
    pub fn attach(_: &eframe::CreationContext<'_>) -> Result<Self, String> {
        Err("Desktop embedding requires Windows; using preview".into())
    }
    pub fn maintain(&mut self) -> Result<(), String> {
        Ok(())
    }
    pub fn prepare_close(&mut self) {}
    pub fn exit_requested(&self) -> bool {
        false
    }
}
