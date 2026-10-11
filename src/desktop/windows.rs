//! All HWND manipulation is confined here. Never mutate or destroy shell windows.
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError, WPARAM},
    Graphics::Gdi::{
        ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
        ScreenToClient,
    },
    System::{
        Power::{
            HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
            UnregisterPowerSettingNotification,
        },
        RemoteDesktop::{WTSRegisterSessionNotification, WTSUnRegisterSessionNotification},
        SystemServices::GUID_CONSOLE_DISPLAY_STATE,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, GetDpiForSystem, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const SUBCLASS_ID: usize = 1;
static DISPLAY_DIRTY: AtomicBool = AtomicBool::new(false);
static DISPLAY_ON: AtomicBool = AtomicBool::new(true);
static SESSION_ACTIVE: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy, PartialEq, Eq)]
struct PrimaryDisplay {
    monitor: usize,
    rect: [i32; 4],
    dpi: u32,
}
impl PrimaryDisplay {
    fn read() -> Result<Self, String> {
        unsafe {
            let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            check(GetMonitorInfoW(monitor, &mut info), "GetMonitorInfoW")?;
            let mut x = 0;
            let mut y = 0;
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) < 0 {
                x = GetDpiForSystem();
            }
            let r = info.rcMonitor;
            Ok(Self {
                monitor: monitor as usize,
                rect: [r.left, r.top, r.right, r.bottom],
                dpi: x.max(96),
            })
        }
    }
}
pub fn shell_ready() -> bool {
    unsafe extern "system" fn find_icons(hwnd: HWND, data: LPARAM) -> i32 {
        unsafe {
            if !FindWindowExW(
                hwnd,
                null_mut(),
                wide("SHELLDLL_DefView").as_ptr(),
                std::ptr::null(),
            )
            .is_null()
            {
                *(data as *mut bool) = true;
                return 0;
            }
        }
        1
    }
    unsafe {
        if FindWindowW(wide("Progman").as_ptr(), std::ptr::null()).is_null() {
            return false;
        }
        let mut ready = false;
        EnumWindows(Some(find_icons), &mut ready as *mut bool as LPARAM);
        ready
    }
}
fn child_style(bits: u32) -> u32 {
    (bits & !(WS_OVERLAPPEDWINDOW | WS_POPUP)) | WS_CHILD
}
fn child_extended_style(bits: u32, raised: bool) -> u32 {
    let bits = (bits & !(WS_EX_APPWINDOW | WS_EX_TOPMOST | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE))
        | WS_EX_TOOLWINDOW
        | WS_EX_NOACTIVATE
        | WS_EX_TRANSPARENT;
    if raised { bits | WS_EX_LAYERED } else { bits }
}
// winit re-applies cached top-level styles when it first shows the window.
// Preserve our child styles during those changes, rather than repairing a
// visible window afterwards. The callback only subclasses our own HWND.
unsafe extern "system" fn child_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    raised: usize,
) -> LRESULT {
    unsafe {
        if message == WM_WTSSESSION_CHANGE {
            match wparam as u32 {
                WTS_SESSION_LOCK | WTS_SESSION_LOGOFF => {
                    SESSION_ACTIVE.store(false, Ordering::Relaxed);
                }
                WTS_SESSION_UNLOCK | WTS_SESSION_LOGON => {
                    SESSION_ACTIVE.store(true, Ordering::Relaxed);
                }
                _ => (),
            }
            DISPLAY_DIRTY.store(true, Ordering::Relaxed);
        }
        if message == WM_POWERBROADCAST && wparam as u32 == PBT_POWERSETTINGCHANGE && lparam != 0 {
            let state = &*(lparam as *const POWERBROADCAST_SETTING);
            let guid = state.PowerSetting;
            if guid.data1 == GUID_CONSOLE_DISPLAY_STATE.data1
                && guid.data2 == GUID_CONSOLE_DISPLAY_STATE.data2
                && guid.data3 == GUID_CONSOLE_DISPLAY_STATE.data3
                && guid.data4 == GUID_CONSOLE_DISPLAY_STATE.data4
                && state.DataLength >= 4
            {
                let value = (state.Data.as_ptr() as *const u32).read_unaligned();
                DISPLAY_ON.store(value != 0, Ordering::Relaxed);
            }
        }
        if matches!(
            message,
            WM_DISPLAYCHANGE | WM_DPICHANGED | WM_DPICHANGED_AFTERPARENT | WM_POWERBROADCAST
        ) {
            DISPLAY_DIRTY.store(true, Ordering::Relaxed);
            crate::diagnostics::event(&format!(
                "Desktop event: message={message:#x} detail={wparam:#x}"
            ));
        }
        if message == WM_STYLECHANGING && lparam != 0 {
            let styles = &mut *(lparam as *mut STYLESTRUCT);
            let requested = styles.styleNew;
            match wparam as i32 {
                GWL_STYLE => styles.styleNew = child_style(styles.styleNew),
                GWL_EXSTYLE => styles.styleNew = child_extended_style(styles.styleNew, raised != 0),
                _ => (),
            }
            if requested != styles.styleNew {
                crate::diagnostics::event(&format!(
                    "Window style guarded: index={} requested={requested:#x} applied={:#x}",
                    wparam as i32, styles.styleNew
                ));
            }
        } else if message == WM_WINDOWPOSCHANGING && lparam != 0 {
            let position = &mut *(lparam as *mut WINDOWPOS);
            if position.flags & SWP_NOZORDER == 0 {
                crate::diagnostics::event(&format!(
                    "Window layer change blocked: flags={:#x} after={:#x}",
                    position.flags, position.hwndInsertAfter as usize
                ));
            }
            position.flags |= SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
        } else if message == WM_SHOWWINDOW {
            crate::diagnostics::event(&format!(
                "Window visibility: show={} reason={lparam}",
                wparam != 0
            ));
        } else if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(child_window_proc), id);
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

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

/// Called by eframe after the event loop enables DPI awareness, before it
/// creates the window and configures its first wgpu surface. Thus the first
/// frame already has the primary display's client size instead of 1100x750.
pub fn wallpaper_viewport(builder: eframe::egui::ViewportBuilder) -> eframe::egui::ViewportBuilder {
    unsafe {
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let handle = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        if GetMonitorInfoW(handle, &mut monitor) == 0 {
            crate::diagnostics::event("Primary display size unavailable before window creation");
            return builder;
        }
        let dpi = GetDpiForSystem().max(96) as f32;
        let width = (monitor.rcMonitor.right - monitor.rcMonitor.left) as f32 * 96.0 / dpi;
        let height = (monitor.rcMonitor.bottom - monitor.rcMonitor.top) as f32 * 96.0 / dpi;
        builder
            .with_inner_size([width, height])
            .with_position([0.0, 0.0])
            .with_active(false)
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
    dpi: f32,
    repair_failed_since: Option<Instant>,
    animation_visible: bool,
    power_notification: HPOWERNOTIFY,
    session_notification: bool,
    closing: bool,
    subclassed: bool,
    exit: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
                dpi: PrimaryDisplay::read()?.dpi as f32 / 96.0,
                repair_failed_since: None,
                animation_visible: true,
                power_notification: 0,
                session_notification: false,
                monitor: None,
                exit: Default::default(),
                closing: false,
                subclassed: false,
            };
            desktop.embed()?;
            desktop.power_notification = RegisterPowerSettingNotification(
                hwnd,
                &GUID_CONSOLE_DISPLAY_STATE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
            desktop.session_notification = WTSRegisterSessionNotification(hwnd, 0) != 0;
            if desktop.power_notification == 0 || !desktop.session_notification {
                crate::diagnostics::event(
                    "Power/session notification unavailable; using desktop visibility polling",
                );
            }
            {
                let worker = std::env::args().any(|arg| arg == "--wallpaper-worker");
                let exit = desktop.exit.clone();
                let (stop, receiver) = std::sync::mpsc::channel();
                let handle = hwnd as usize;
                let ctx = cc.egui_ctx.clone();
                // Observe hold/release without forcing GPU frames; also detect a destroyed HWND.
                let monitor = std::thread::Builder::new()
                    .name("shell-lifetime".into())
                    .spawn(move || {
                        let mut display = PrimaryDisplay::read().ok();
                        let mut last = Instant::now();
                        let mut hold = super::hold::Hold::default();
                        while matches!(
                            receiver.recv_timeout(Duration::from_millis(50)),
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                        ) {
                            let down = [VK_CONTROL, VK_MENU, VK_SHIFT, 0x51]
                                .iter()
                                .all(|key| GetAsyncKeyState(i32::from(*key)) < 0);
                            if hold.update(down, Instant::now()) {
                                exit.store(true, Ordering::Relaxed);
                                ctx.request_repaint();
                                break;
                            }
                            if last.elapsed() < Duration::from_secs(1) {
                                continue;
                            }
                            if worker && !owns(handle as HWND) {
                                // This renderer's HWND is already gone. OS teardown frees its
                                // remaining GPU/process resources; the supervisor owns restart.
                                crate::diagnostics::event(
                                    "Exit: reason=shell_lost source=lifetime_monitor code=75",
                                );
                                std::process::exit(super::supervisor::SHELL_LOST);
                            }
                            let current = PrimaryDisplay::read().ok();
                            if current != display
                                || DISPLAY_DIRTY.load(Ordering::Relaxed)
                                || last.elapsed() > Duration::from_secs(3)
                            {
                                display = current;
                                DISPLAY_DIRTY.store(true, Ordering::Relaxed);
                                ctx.request_repaint();
                            }
                            last = Instant::now();
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
            check(
                SetWindowSubclass(
                    self.hwnd,
                    Some(child_window_proc),
                    SUBCLASS_ID,
                    usize::from(self.layer.raised),
                ),
                "SetWindowSubclass",
            )?;
            self.subclassed = true;
            let bits = child_style(self.original_style as u32) as isize;
            // Keep eframe's hidden initial state until the first correctly sized frame.
            let extended =
                child_extended_style(self.original_ex_style as u32, self.layer.raised) as isize;
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
    fn raised_order_valid(&self) -> Result<bool, String> {
        if !self.layer.raised {
            return Ok(true);
        }
        // GetWindow siblings are ordered from top to bottom. Bound traversal
        // and reject recycled handles instead of risking an endless Shell walk.
        unsafe {
            let mut siblings = Vec::new();
            let mut hwnd = GetWindow(self.layer.parent, GW_CHILD);
            while !hwnd.is_null() {
                if siblings.len() >= 256 || siblings.contains(&(hwnd as usize)) {
                    return Err("Unstable desktop child hierarchy".into());
                }
                siblings.push(hwnd as usize);
                hwnd = GetWindow(hwnd, GW_HWNDNEXT);
            }
            Ok(super::order::between(
                &siblings,
                self.hwnd as usize,
                self.layer.icons as usize,
                self.layer.worker as usize,
            ))
        }
    }
    fn position(&self, frame_changed: bool) -> Result<(), String> {
        unsafe {
            let mut monitor: MONITORINFO = std::mem::zeroed();
            monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            check(
                GetMonitorInfoW(
                    MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY),
                    &mut monitor,
                ),
                "GetMonitorInfoW",
            )?;
            let screen = monitor.rcMonitor;
            let width = screen.right - screen.left;
            let height = screen.bottom - screen.top;
            if width <= 0 || height <= 0 {
                return Err("Invalid primary monitor dimensions".into());
            }
            let mut desired = POINT {
                x: screen.left,
                y: screen.top,
            };
            check(
                ScreenToClient(self.layer.parent, &mut desired),
                "ScreenToClient",
            )?;
            // Compare client geometry in the same coordinate system. Comparing
            // outer screen rectangles against system metrics could trigger DPI churn.
            let mut actual = POINT { x: 0, y: 0 };
            check(ClientToScreen(self.hwnd, &mut actual), "ClientToScreen")?;
            check(
                ScreenToClient(self.layer.parent, &mut actual),
                "ScreenToClient",
            )?;
            let mut client: RECT = std::mem::zeroed();
            check(GetClientRect(self.hwnd, &mut client), "GetClientRect")?;
            let geometry_changed = actual.x != desired.x
                || actual.y != desired.y
                || client.right - client.left != width
                || client.bottom - client.top != height;
            let order_valid = self.raised_order_valid()?;
            if frame_changed || geometry_changed || !order_valid {
                // Our placement calls bypass the guard, which prevents winit from
                // raising the window when it updates unrelated cached flags.
                let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING;
                if frame_changed {
                    flags |= SWP_FRAMECHANGED;
                }
                if !frame_changed && order_valid {
                    flags |= SWP_NOZORDER;
                }
                if !geometry_changed {
                    flags |= SWP_NOMOVE | SWP_NOSIZE;
                }
                let after = if self.layer.raised {
                    self.layer.icons
                } else {
                    HWND_BOTTOM
                };
                crate::diagnostics::event(&format!(
                    "Desktop placement: frame={frame_changed} geometry={geometry_changed} order={} size={width}x{height}",
                    !order_valid
                ));
                check(
                    SetWindowPos(self.hwnd, after, desired.x, desired.y, width, height, flags),
                    "SetWindowPos",
                )?;
            }
            if GetParent(self.hwnd) != self.layer.parent {
                return Err("Shell parent changed during attachment".into());
            }
            if !self.raised_order_valid()? {
                return Err(
                    "Unable to place wallpaper between desktop icons and Shell background".into(),
                );
            }
            Ok(())
        }
    }
    pub fn maintain(&mut self) -> Result<(), String> {
        if !DISPLAY_DIRTY.swap(false, Ordering::Relaxed)
            && self.last_probe.elapsed() < Duration::from_secs(2)
        {
            return Ok(());
        }
        self.last_probe = Instant::now();
        match self.repair() {
            Ok(()) => {
                if self.repair_failed_since.take().is_some() {
                    crate::diagnostics::event("Desktop recovery completed");
                }
                Ok(())
            }
            Err(error) => {
                let since = self.repair_failed_since.get_or_insert_with(|| {
                    crate::diagnostics::event(&format!("Desktop repair pending: {error}"));
                    Instant::now()
                });
                if since.elapsed() < Duration::from_secs(15) {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
    pub fn pixels_per_point(&self) -> f32 {
        self.dpi
    }
    pub fn animation_visible(&self) -> bool {
        self.animation_visible
    }
    pub fn rendering_suspended(&self) -> bool {
        !DISPLAY_ON.load(Ordering::Relaxed) || !SESSION_ACTIVE.load(Ordering::Relaxed)
    }
    fn repair(&mut self) -> Result<(), String> {
        unsafe {
            if !owns(self.hwnd) {
                return Err("Explorer destroyed the child HWND; restart the application".into());
            }
            if IsWindow(self.layer.parent) == 0
                || IsWindow(self.layer.icons) == 0
                || GetParent(self.hwnd) != self.layer.parent
            {
                if std::env::args().any(|arg| arg == "--wallpaper-worker") {
                    // Recreate only our renderer. Do not restore a visible
                    // top-level window while Explorer is changing its layers.
                    crate::diagnostics::event(
                        "Exit: reason=shell_lost source=desktop_repair code=75",
                    );
                    std::process::exit(super::supervisor::SHELL_LOST);
                }
                // Detach before discovery; never attach to an invalid/reused handle blindly.
                self.restore(false)?;
                self.layer = discover()?;
                self.embed()?;
                // This HWND already had a painted frame before recovery; its
                // eframe first-frame show hook is not run for a second time.
                ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            } else {
                self.position(false)?;
            }
            let dpi = PrimaryDisplay::read()?.dpi as f32 / 96.0;
            let foreground = GetForegroundWindow();
            self.animation_visible = true;
            if foreground != self.hwnd
                && foreground != self.layer.parent
                && foreground != self.layer.icons
                && !foreground.is_null()
                && IsWindowVisible(foreground) != 0
                && IsIconic(foreground) == 0
            {
                let mut bounds: RECT = std::mem::zeroed();
                let mut info: MONITORINFO = std::mem::zeroed();
                info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetWindowRect(foreground, &mut bounds) != 0
                    && GetMonitorInfoW(
                        MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY),
                        &mut info,
                    ) != 0
                {
                    let work = info.rcWork;
                    self.animation_visible = !(bounds.left <= work.left
                        && bounds.top <= work.top
                        && bounds.right >= work.right
                        && bounds.bottom >= work.bottom);
                }
            }
            if (dpi - self.dpi).abs() > 0.01 {
                crate::diagnostics::event(&format!(
                    "Desktop DPI changed: {} -> {}",
                    self.dpi * 96.0,
                    dpi * 96.0
                ));
                self.dpi = dpi;
            }
        }
        Ok(())
    }
    pub fn prepare_close(&mut self) {
        self.closing = true;
        if let Some((stop, monitor)) = self.monitor.take() {
            let _ = stop.send(());
            let _ = monitor.join();
        }
    }
    pub fn exit_requested(&self) -> bool {
        self.exit.load(Ordering::Relaxed)
    }
    fn restore(&mut self, show_preview: bool) -> Result<(), String> {
        unsafe {
            if !owns(self.hwnd) {
                return Ok(());
            }
            if self.subclassed {
                check(
                    RemoveWindowSubclass(self.hwnd, Some(child_window_proc), SUBCLASS_ID),
                    "RemoveWindowSubclass",
                )?;
                self.subclassed = false;
            }
            if !show_preview {
                ShowWindow(self.hwnd, SW_HIDE);
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
                    SWP_NOACTIVATE
                        | SWP_FRAMECHANGED
                        | if show_preview { SWP_SHOWWINDOW } else { 0 },
                ),
                "Restore preview window",
            )
        }
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        unsafe {
            if self.power_notification != 0 {
                UnregisterPowerSettingNotification(self.power_notification);
            }
            if self.session_notification && owns(self.hwnd) {
                WTSUnRegisterSessionNotification(self.hwnd);
            }
        }
        if !self.closing && self.monitor.is_some() && !owns(self.hwnd) {
            crate::diagnostics::event("Exit: reason=shell_lost source=desktop_drop code=75");
            std::process::exit(super::supervisor::SHELL_LOST);
        }
        if let Some((stop, monitor)) = self.monitor.take() {
            let _ = stop.send(());
            let _ = monitor.join();
        }
        if let Err(error) = self.restore(!self.closing) {
            crate::diagnostics::event(&format!("Wallpaper detach failed: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_show_retains_child_styles_without_early_visibility() {
        let hidden = child_style(WS_OVERLAPPEDWINDOW);
        assert_eq!(hidden & WS_VISIBLE, 0);
        let shown = child_style(WS_OVERLAPPEDWINDOW | WS_VISIBLE);
        assert_ne!(shown & WS_CHILD, 0);
        assert_ne!(shown & WS_VISIBLE, 0);
        assert_eq!(shown & (WS_CAPTION | WS_THICKFRAME | WS_POPUP), 0);
    }
    #[test]
    fn toolkit_style_reset_cannot_restore_taskbar_activation_or_borders() {
        let bits = child_extended_style(
            WS_EX_APPWINDOW | WS_EX_TOPMOST | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
            true,
        );
        assert_eq!(
            bits & (WS_EX_APPWINDOW | WS_EX_TOPMOST | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE),
            0
        );
        assert_eq!(
            bits & (WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TOOLWINDOW),
            WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TOOLWINDOW
        );
    }
}
