//! All HWND manipulation is confined here. Never mutate or destroy shell windows.
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ptr::null_mut,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, POINT, RECT, SetLastError},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
        WindowsAndMessaging::*,
    },
};

fn owns(hwnd: HWND) -> bool {
    let mut pid = 0;
    // An HWND value may be recycled by Explorer; never modify a recycled handle.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) != 0 && pid == std::process::id() }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn failure(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}
fn check(result: i32, operation: &str) -> Result<(), String> {
    if result == 0 {
        Err(failure(operation))
    } else {
        Ok(())
    }
}
unsafe fn style(hwnd: HWND, index: i32) -> Result<isize, String> {
    unsafe {
        SetLastError(0);
        let value = GetWindowLongPtrW(hwnd, index);
        if value == 0 && GetLastError() != 0 {
            Err(failure("GetWindowLongPtrW"))
        } else {
            Ok(value)
        }
    }
}
unsafe fn set_style(hwnd: HWND, index: i32, value: isize) -> Result<(), String> {
    unsafe {
        SetLastError(0);
        let old = SetWindowLongPtrW(hwnd, index, value);
        if old == 0 && GetLastError() != 0 {
            Err(failure("SetWindowLongPtrW"))
        } else {
            Ok(())
        }
    }
}
unsafe fn parent(hwnd: HWND, new_parent: HWND) -> Result<(), String> {
    unsafe {
        SetLastError(0);
        let old = SetParent(hwnd, new_parent);
        if old.is_null() && GetLastError() != 0 {
            Err(failure("SetParent"))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy)]
struct Layer {
    parent: HWND,
    icons: HWND,
    worker: HWND,
    raised: bool,
}
struct Search {
    icons: HWND,
    worker: HWND,
}
unsafe extern "system" fn enumerate(hwnd: HWND, param: LPARAM) -> i32 {
    // SAFETY: EnumWindows is synchronous; param points to a live, exclusive Search.
    unsafe {
        let search = &mut *(param as *mut Search);
        let icons = FindWindowExW(
            hwnd,
            null_mut(),
            wide("SHELLDLL_DefView").as_ptr(),
            std::ptr::null(),
        );
        if !icons.is_null() {
            let worker =
                FindWindowExW(null_mut(), hwnd, wide("WorkerW").as_ptr(), std::ptr::null());
            if !worker.is_null() {
                search.icons = icons;
                search.worker = worker;
            }
        }
    }
    1
}
fn discover() -> Result<Layer, String> {
    // SAFETY: handles returned by Shell discovery are borrowed, never freed.
    unsafe {
        let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
        if progman.is_null() {
            return Err("Explorer Progman is unavailable".into());
        }
        let mut result = 0;
        if SendMessageTimeoutW(
            progman,
            0x052c,
            0x0d,
            1,
            SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
            1000,
            &mut result,
        ) == 0
        {
            return Err(failure("Progman wallpaper-layer request (1s timeout)"));
        }
        let raised = style(progman, GWL_EXSTYLE)? & WS_EX_NOREDIRECTIONBITMAP as isize != 0;
        if raised {
            let icons = FindWindowExW(
                progman,
                null_mut(),
                wide("SHELLDLL_DefView").as_ptr(),
                std::ptr::null(),
            );
            let worker = FindWindowExW(
                progman,
                null_mut(),
                wide("WorkerW").as_ptr(),
                std::ptr::null(),
            );
            if icons.is_null() || worker.is_null() {
                return Err(
                    "Unrecognized raised-desktop hierarchy; refusing unsafe attachment".into(),
                );
            }
            return Ok(Layer {
                parent: progman,
                icons,
                worker,
                raised: true,
            });
        }
        let mut search = Search {
            icons: null_mut(),
            worker: null_mut(),
        };
        check(
            EnumWindows(Some(enumerate), &mut search as *mut Search as LPARAM),
            "EnumWindows",
        )?;
        if search.worker.is_null() {
            return Err("No separate WorkerW behind desktop icons".into());
        }
        if !FindWindowExW(
            search.worker,
            null_mut(),
            wide("SHELLDLL_DefView").as_ptr(),
            std::ptr::null(),
        )
        .is_null()
        {
            return Err("Selected WorkerW contains icons; refusing attachment".into());
        }
        Ok(Layer {
            parent: search.worker,
            icons: search.icons,
            worker: search.worker,
            raised: false,
        })
    }
}

pub struct Desktop {
    hwnd: HWND,
    original_parent: HWND,
    original_style: isize,
    original_ex_style: isize,
    original_rect: RECT,
    layer: Layer,
    last_probe: Instant,
    closing: bool,
    monitor: Option<(std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>)>,
}
impl Desktop {
    pub fn attach(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let hwnd = match cc.window_handle().map_err(|e| e.to_string())?.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd.get() as HWND,
            _ => return Err("Renderer did not provide a Win32 HWND".into()),
        };
        let layer = discover()?;
        // SAFETY: eframe owns this live window on this thread. Guard rolls back on failure.
        unsafe {
            let mut rect = std::mem::zeroed();
            check(GetWindowRect(hwnd, &mut rect), "GetWindowRect")?;
            let mut desktop = Self {
                hwnd,
                original_parent: GetParent(hwnd),
                original_style: style(hwnd, GWL_STYLE)?,
                original_ex_style: style(hwnd, GWL_EXSTYLE)?,
                original_rect: rect,
                layer,
                last_probe: Instant::now(),
                monitor: None,
                closing: false,
            };
            desktop.embed()?;
            if std::env::args().any(|arg| arg == "--wallpaper-worker") {
                let (stop, receiver) = std::sync::mpsc::channel();
                let handle = hwnd as usize;
                // Needed only because an externally destroyed HWND receives no repaint.
                let monitor = std::thread::Builder::new()
                    .name("shell-lifetime".into())
                    .spawn(move || {
                        while matches!(
                            receiver.recv_timeout(Duration::from_secs(1)),
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                        ) {
                            if !owns(handle as HWND) {
                                // This renderer's HWND is already gone. OS teardown frees its
                                // remaining GPU/process resources; the supervisor owns restart.
                                std::process::exit(super::supervisor::SHELL_LOST);
                            }
                        }
                    })
                    .map_err(|error| format!("Shell lifetime monitor: {error}"))?;
                desktop.monitor = Some((stop, monitor));
            }
            Ok(desktop)
        }
    }
    fn embed(&mut self) -> Result<(), String> {
        unsafe {
            if !owns(self.hwnd) {
                return Err("Wallpaper HWND no longer exists".into());
            }
            let bits = (self.original_style
                & !((WS_POPUP
                    | WS_CAPTION
                    | WS_THICKFRAME
                    | WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX) as isize))
                | (WS_CHILD | WS_VISIBLE) as isize;
            let mut extended = (self.original_ex_style & !(WS_EX_APPWINDOW as isize))
                | (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT) as isize;
            if self.layer.raised {
                extended |= WS_EX_LAYERED as isize;
            }
            set_style(self.hwnd, GWL_STYLE, bits)?;
            set_style(self.hwnd, GWL_EXSTYLE, extended)?;
            if self.layer.raised {
                check(
                    SetLayeredWindowAttributes(self.hwnd, 0, 255, LWA_ALPHA),
                    "SetLayeredWindowAttributes",
                )?;
            }
            parent(self.hwnd, self.layer.parent)?;
            self.position(true)?;
            Ok(())
        }
    }
    fn position(&self, force: bool) -> Result<(), String> {
        unsafe {
            // Primary display only in MVP. Translate physical screen coordinates to parent client.
            let mut origin = POINT { x: 0, y: 0 };
            check(
                ScreenToClient(self.layer.parent, &mut origin),
                "ScreenToClient",
            )?;
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            if width <= 0 || height <= 0 {
                return Err("Invalid primary monitor dimensions".into());
            }
            let after = if self.layer.raised {
                self.layer.icons
            } else {
                HWND_BOTTOM
            };
            let mut current: RECT = std::mem::zeroed();
            check(GetWindowRect(self.hwnd, &mut current), "GetWindowRect")?;
            if force
                || current.left != 0
                || current.top != 0
                || current.right != width
                || current.bottom != height
                || (self.layer.raised && GetWindow(self.hwnd, GW_HWNDPREV) != after)
            {
                check(
                    SetWindowPos(
                        self.hwnd,
                        after,
                        origin.x,
                        origin.y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
                    ),
                    "SetWindowPos",
                )?;
            }
            if GetParent(self.hwnd) != self.layer.parent {
                return Err("Shell parent changed during attachment".into());
            }
            if self.layer.raised {
                // Refuse a hierarchy in which another shell wallpaper covers our window.
                let mut sibling = GetWindow(self.hwnd, GW_HWNDPREV);
                let mut icons_above = false;
                while !sibling.is_null() {
                    if sibling == self.layer.icons {
                        icons_above = true;
                        break;
                    }
                    sibling = GetWindow(sibling, GW_HWNDPREV);
                }
                if !icons_above {
                    return Err("Unable to place wallpaper below icons".into());
                }
                let mut sibling = GetWindow(self.hwnd, GW_HWNDNEXT);
                let mut worker_below = false;
                while !sibling.is_null() {
                    if sibling == self.layer.worker {
                        worker_below = true;
                        break;
                    }
                    sibling = GetWindow(sibling, GW_HWNDNEXT);
                }
                if !worker_below {
                    return Err("Shell WorkerW would cover wallpaper; using preview".into());
                }
            }
            Ok(())
        }
    }
    pub fn maintain(&mut self) -> Result<(), String> {
        if self.last_probe.elapsed() < Duration::from_secs(2) {
            return Ok(());
        }
        self.last_probe = Instant::now();
        unsafe {
            if !owns(self.hwnd) {
                return Err("Explorer destroyed the child HWND; restart the application".into());
            }
            if IsWindow(self.layer.parent) == 0
                || IsWindow(self.layer.icons) == 0
                || GetParent(self.hwnd) != self.layer.parent
            {
                // Detach before discovery; never attach to an invalid/reused handle blindly.
                self.restore()?;
                self.layer = discover()?;
                self.embed()?;
            } else {
                self.position(false)?;
            }
        }
        Ok(())
    }
    pub fn prepare_close(&mut self) {
        self.closing = true;
    }
    pub fn exit_requested(&self) -> bool {
        // Hold this chord for at least one second. No hooks, injected input or global registration.
        unsafe {
            [VK_CONTROL, VK_MENU, VK_SHIFT, 0x51]
                .iter()
                .all(|key| GetAsyncKeyState(i32::from(*key)) < 0)
        }
    }
    fn restore(&self) -> Result<(), String> {
        unsafe {
            if !owns(self.hwnd) {
                return Ok(());
            }
            parent(self.hwnd, self.original_parent)?;
            set_style(self.hwnd, GWL_STYLE, self.original_style)?;
            set_style(self.hwnd, GWL_EXSTYLE, self.original_ex_style)?;
            let r = self.original_rect;
            check(
                SetWindowPos(
                    self.hwnd,
                    HWND_NOTOPMOST,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
                ),
                "Restore preview window",
            )
        }
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        if !self.closing && self.monitor.is_some() && !owns(self.hwnd) {
            std::process::exit(super::supervisor::SHELL_LOST);
        }
        if let Some((stop, monitor)) = self.monitor.take() {
            let _ = stop.send(());
            let _ = monitor.join();
        }
        if let Err(error) = self.restore() {
            eprintln!("Wallpaper detach failed: {error}");
        }
    }
}
