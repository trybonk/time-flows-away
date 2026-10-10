#[cfg(windows)]
pub struct Instance(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Instance {
    pub fn acquire(settings: bool) -> Result<Option<Self>, String> {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
            System::Threading::CreateMutexW,
        };
        let name = if settings {
            r"Local\TimeFlowsAway.Settings"
        } else {
            r"Local\TimeFlowsAway.Wallpaper"
        };
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // A session-local named mutex is a presence guard; no ownership or wait.
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                Ok(None)
            } else {
                Ok(Some(Self(handle)))
            }
        }
    }
}
#[cfg(windows)]
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
pub fn browse(font: bool, owner: usize) -> Result<Option<String>, String> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let filter = if font {
        "Fonts (TTF, OTF, TTC)\0*.ttf;*.otf;*.ttc\0All files\0*.*\0\0"
    } else {
        "Images (PNG, JPEG)\0*.png;*.jpg;*.jpeg\0All files\0*.*\0\0"
    };
    let filter: Vec<u16> = filter.encode_utf16().collect();
    let mut file = vec![0u16; 32768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner as windows_sys::Win32::Foundation::HWND,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // Borrowed buffers remain alive for the synchronous native dialog.
    unsafe {
        if GetOpenFileNameW(&mut dialog) != 0 {
            let end = file.iter().position(|v| *v == 0).unwrap_or(file.len());
            return Ok(Some(String::from_utf16_lossy(&file[..end])));
        }
        let error = CommDlgExtendedError();
        if error == 0 {
            Ok(None)
        } else {
            Err(format!("File dialog error: 0x{error:x}"))
        }
    }
}
#[cfg(not(windows))]
pub fn browse(_: bool, _: usize) -> Result<Option<String>, String> {
    Err("Enter an absolute path to the file (file picker is available on Windows)".into())
}
pub fn owner(cc: &eframe::CreationContext<'_>) -> usize {
    #[cfg(windows)]
    {
        use raw_window_handle::HasWindowHandle;
        if let Ok(handle) = cc.window_handle()
            && let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw()
        {
            return handle.hwnd.get() as usize;
        }
    }
    #[cfg(not(windows))]
    let _ = cc;
    0
}
#[cfg(windows)]
fn settings_window() -> Option<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId},
    };
    let title: Vec<u16> = "Time Flows Away — Settings"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // Validate the owner executable before interacting with the found window.
    // Handles from FindWindow are borrowed; only our opened query handle is closed.
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if window.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len) != 0;
        CloseHandle(process);
        if !queried {
            return None;
        }
        let found = String::from_utf16_lossy(&buffer[..len as usize]);
        let Ok(own) = std::env::current_exe() else {
            return None;
        };
        if !found
            .trim_start_matches(r"\\?\")
            .eq_ignore_ascii_case(own.to_string_lossy().trim_start_matches(r"\\?\"))
        {
            return None;
        }
        Some(window)
    }
}
#[cfg(windows)]
pub fn focus_settings() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    if let Some(window) = settings_window() {
        unsafe {
            ShowWindow(window, SW_RESTORE);
            SetForegroundWindow(window);
        }
        true
    } else {
        false
    }
}
#[cfg(windows)]
pub fn close_settings() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    if let Some(window) = settings_window() {
        // Ask the verified settings window to close normally; never terminate
        // processes. This also handles a window opened via --settings.
        unsafe {
            PostMessageW(window, WM_CLOSE, 0, 0);
        }
    }
}
#[cfg(windows)]
pub struct SettingsLifetime(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl SettingsLifetime {
    pub fn attach(pid: u32) -> Self {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        // A parent that already exited during GUI initialization must still
        // close its settings window, rather than leaving an orphan.
        Self(handle)
    }
    pub fn finished(&self) -> bool {
        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
        };
        self.0.is_null() || unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }
}
#[cfg(windows)]
impl Drop for SettingsLifetime {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
}

#[cfg(all(windows, test))]
mod tests {
    use super::SettingsLifetime;
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    #[test]
    fn settings_lifetime_tracks_owner_exit_and_missing_owner() {
        let own = SettingsLifetime::attach(std::process::id());
        assert!(!own.finished());
        assert!(SettingsLifetime::attach(0).finished());
        let mut child = Command::new("cmd.exe")
            .args(["/C", "exit", "0"])
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Child keeps a process handle even if exit raced with attach. Either
        // an opened signaled handle or a missing owner must close settings.
        let lifetime = SettingsLifetime::attach(child.id());
        assert!(child.wait().unwrap().success());
        assert!(lifetime.finished());
    }
}
