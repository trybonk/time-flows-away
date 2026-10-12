# Time Flows Away — native Windows development

Rust edition 2024, egui/eframe + OpenGL on Windows; optional wgpu/DX12 diagnostics.
No browser and no Wallpaper Engine.
Licensed under the root Apache-2.0 LICENSE; see ../NOTICE for attribution.
The original `../legacy/wallpaper-engine/src/` files remain the implementation reference.

## Build and run on Windows 11 x64

Install Rust with the MSVC toolchain and Visual Studio Build Tools (Desktop
Development with C++, including a Windows SDK). From the repository root:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
.\target\release\time-flows-away-native.exe --preview
.\target\release\time-flows-away-native.exe
```

Rust 1.99.0 is pinned in rust-toolchain.toml. Windows binaries use the GUI subsystem
and do not create a terminal. Diagnostics go to
`%LOCALAPPDATA%\TimeFlowsAway\app.log`. The log includes millisecond timestamps,
GPU/driver identification, surface errors, guarded style/layer changes and window
visibility events. Identical surface errors are rate-limited to one entry per
ten seconds; a new error kind is recorded immediately. Preview is a normal window; Esc closes it. In wallpaper mode hold
**Ctrl+Alt+Shift+Q for at least one second** to close gracefully. Use the chord for orderly detachment; closing a terminal is no longer necessary.
No administrator privileges are required.

Windows now uses OpenGL by default after the user's RTX 3060 comparison reported
that the observed flashes disappeared. Run with `--wgpu` to compare DX12;
`--opengl` explicitly selects the default. Renderer/config arguments are forwarded
through Explorer recovery and to the separate settings process. This observation
on one PC does not prove stability on all Windows configurations.

`--settings` opens the English settings window. A worker-owned tray provides
Settings, Pause/Resume and Quit; double-click opens Settings. Preview uses Space
to pause. Settings changes are previewed locally and saved explicitly to
`%LOCALAPPDATA%\TimeFlowsAway\config.toml`. Wallpaper checks the file every two
seconds, keeping its last valid state if TOML/values are invalid. Saves write a
unique temporary file, sync it and replace the destination. The settings file is
not created until Save. See [settings manual checks](SETTINGS-CHECKLIST.md).

The GitHub Actions workflow `Windows build` runs formatting, Clippy, tests,
and a release build on Windows for pull requests, pushes to `feat/**`
or master, and manual dispatch. Download `time-flows-away-windows-x64` from the
run's artifacts and extract the executable. CI compilation does **not** test
desktop embedding. Commands run from the repository root; binaries are under
`target/release/`, and the CI artifact includes the `docs/` directory.
Tags matching the Cargo version (for example `v0.3.2`) also run the full Windows
checks, then publish a GitHub pre-release with a versioned ZIP, its SHA-256 and
the build manifest. Release tags must point to commits in `master` and have
version-specific notes in `docs/releases/`. See [release process](RELEASING.md).

## Assets and saved settings

Images are centered and cropped to fill the display. Layout sizes and offsets
use logical pixels; the layout shrinks to fit small windows. Weekday and automatic
timer labels are English. Timer durations use local civil time, including its
clock/DST changes. Use absolute asset paths; files remain on your PC.

Images are limited to 8192 pixels per side and a 256 MiB decode allocation;
fonts (TTF/OTF/TTC) are limited to 50 MiB. An invalid asset falls back to a solid
background or the system font, with an error in settings. To retry a replaced
asset at the same path, clear and reselect its path.

Missing settings use defaults; the file is created only on Save. Invalid edits
keep the last valid configuration and are logged; the file is preserved for
correction. `--config "C:\path\config.toml"` selects another file and is forwarded
to settings and restarted workers. Reset to defaults changes the draft preview
until saved. Color emoji retain their native colors when the text color or font
changes; input fields use monochrome system glyphs.

## Architecture and safety

- `wallpaper/clock.rs`: pure local calendar calculations and six boundary tests.
- `wallpaper/render.rs`: date, time, two date progress bars, three time bars,
  Chinese heading and English footer. Whole-second clock samples.
- `wallpaper/animation.rs`: one-second linear bar transitions, pause/resume and
  time-jump handling. Settings select off/30/60 FPS; default is 30 FPS.
- `app.rs`: event-driven lifecycle, config refresh, pause state and preview status.
- `config.rs`: versioned TOML settings, validation, atomic save and last-valid reload.
- `assets.rs`: bounded PNG/JPEG decode and validated font loading on path changes.
- `settings.rs`: independent egui window, unsaved draft, live preview and explicit Save.
- `tray.rs`: Windows worker-owned tray; menu callbacks wake egui without busy polling.
- `platform.rs`: session-local instance guards and the native file picker.
- `platform/autostart.rs`: explicit per-user startup switch using only the
  application's value in HKCU's Run key; quotes Unicode paths and preserves
  renderer/config arguments. No worker/owner/settings arguments are registered.
- `diagnostics.rs`: errors and actual placement changes recorded without a console.
- `desktop/windows.rs`: borrowed Shell HWND discovery, checked style/parent/
  placement calls, one-second bounded Shell message, geometry and layer checks.
  Desktop unsafe Win32 calls are isolated here. Only our window is modified.
- `desktop/supervisor.rs`: wallpaper mode starts a renderer child process.
  eframe cannot rebuild an externally destroyed root HWND in place. A sleeping
  lifetime-monitor thread in the renderer detects a lost/recycled HWND and exits
  with code 75; the supervisor restarts only its own child after two seconds.
  Other exit codes stop the supervisor. This extra process/thread is specifically
  for Explorer recovery, not rendering. Destruction teardown is performed by the
  OS; normal closure cancels/joins the monitor and restores our own window styles.

Classic Shell uses a separate WorkerW behind the icon host. Raised desktop is
recognized by Progman's WS_EX_NOREDIRECTIONBITMAP; the application becomes an
opaque layered child of Progman below SHELLDLL_DefView and above its WorkerW.
Unexpected hierarchy, style/parent/DPI mismatch, or failed positioning rolls back
to a visible preview. WS_EX_TOOLWINDOW excludes the wallpaper from task switching;
WS_EX_NOACTIVATE and WS_EX_TRANSPARENT prevent focus/interaction with the wallpaper.
There is no Explorer injection, termination, process modification, wallpaper
registry modification, or change to Shell window styles/z-order.

Primary display resizing, DPI and relative order are checked every two seconds.
Display/DPI/power notifications and the existing lifetime thread's primary-display
snapshot wake rendering after display changes and sleep. Losing the Shell parent
or icon host restarts only the worker through its supervisor, avoiding a temporary
visible top-level window. Transient positioning failures retry for up to 15 seconds
before the existing preview fallback. The supervisor waits up to 30 seconds for
Explorer's desktop before starting a worker, including sign-in autorun.
Unchanged client geometry and valid relative order do not trigger
SetWindowPos, even if unrelated Shell children are between the icon layer and
our window. Runtime repairs neither activate nor show the window, and frame
changes are only requested after style changes. Normal shutdown detaches hidden.

The initial viewport is sized to the primary display before eframe configures
its rendering surface. Attachment preserves eframe's hidden state until the first
painted frame. A subclass of our own HWND preserves child/layered/nonactivating
styles when winit re-applies cached window styles on first show, and prevents
its unrelated position updates from raising the wallpaper. The subclass is
removed on detach and destruction; Shell windows are never subclassed. A fresh renderer that
cannot attach after an Explorer restart falls back to preview; restart the
application once Explorer has recovered. Retry loops are not allowed to hide
permanent composition or hierarchy failures.

## Source analysis and intentional differences

The original HTML groups heading, date plus year/month bars, time plus day/hour/
minute bars, and footer into a centered column. CSS defines #161616 background,
#c8c8c8 text, 100px bold text, 530x10px bars with 5px vertical gaps, a 30px rounded
text panel and 10x30px padding. The manifest overrides several CSS defaults:

| Property | Manifest default used by MVP |
|---|---|
| Heading/footer | 时光飞逝 / Time flows away |
| Date/time | y.m.d / h:m:s, 24-hour |
| Font/sizes | Microsoft JhengHei; 100px, footer 90px |
| Year/month/day/hour/minute colors | #bcbdbe / #c75e53 / #fffd5d / #4790e5 / #84d84b |
| Alignment/background | centered / #161616 |
| Progress width | scroll_size 100 × 5.3 = 530px |

The original year fraction is `(month_index + month_fraction) / 12`, giving every
month equal weight. MVP uses `(day_of_year_zero_based + day_fraction) / days_in_year`.
Month progress uses actual month length; century/leap-year boundaries are tested.
Day/hour/minute progress follows local civil time (86,400 / 3,600 / 60 seconds),
not elapsed UTC duration across DST. Clock changes are reflected on the next tick.

Bars interpolate each whole-second target over one second, matching the original
CSS transition duration, including the backwards transition at a minute rollover.
Resume and large/backwards clock jumps snap to current progress. Animation off
retains the original native one-tick-per-second behavior.
The layout uses centered baselines adjusted against the supplied Wallpaper Engine
reference screenshot and uniformly shrinks on small previews;
it is not a pixel-identical CSS layout. With a solid background the panel is the
same color as the page, so its rounded outline is invisible.
Windows uses Microsoft JhengHei bold through DirectWrite, with Windows glyph
fallback and support for a selected TTF/OTF/TTC file. Complete system font files
are not copied into the wallpaper's egui font definitions. Linux preview uses
a local Noto Sans CJK font.
Missing CJK fonts may show missing glyphs.
Color emoji use Segoe UI Emoji through DirectWrite and a software Direct2D/WIC
bitmap; this creates no helper HWND or GPU surface. Grapheme segmentation keeps
variation selectors, modifiers and ZWJ sequences together. Textures (including
failed glyphs) are cached, bounded and removed when no longer used. Wallpaper
and live preview share this renderer. Settings input fields use the Segoe outline
fallback only when needed. Linux preview uses installed Noto Color Emoji bitmaps for individual
emoji; full Windows sequence shaping must be tested on Windows.
High quality Windows text uses DirectWrite grayscale antialiasing, up to 3x
supersampling and Lanczos downsampling to its physical display size. Color emoji
are also filtered to their physical size, keeping premultiplied alpha to avoid
edge fringes. Rasterization happens on cache misses. Text layouts, scaled egui
geometry in Linux previews, and ordinary whole-second strings are cached;
changing DPI, fonts or an atlas reset invalidates the affected data. Fractional
date/time formats continue updating within a second. Text/emoji textures share
a 32 MiB budget and unused entries are released after painting.
Geometry feathering and fractional text/rectangle positions remain enabled.
This is grayscale antialiasing, not Windows ClearType, and does not enable MSAA
or alter the OpenGL context/surface configuration. The GPU uses linear filtering
on the already downsampled textures. Fonts and kerning can differ from browser rendering. egui logical
points scale with display DPI; physical measurements need checking on Windows.
Both the GPU clear color and the painted panel use the same opaque #161616
background. This avoids eframe's different translucent default clear color;
it does not establish the cause of any transient compositor flash.

Stage 4 settings now expose colors, position, text/bar sizes, 12/24h presets,
weekday, date/time formats, headings, font, background image and panel opacity,
plus countdown/elapsed display in days, detailed time or seconds. Countdown uses
local civil time rather than UTC elapsed time across DST. The settings UI keeps
egui's normal fonts, loading CJK/emoji outline fallbacks only when needed for its
editable text. Large background images are cropped and filtered to the physical
viewport, respecting the 4096-pixel texture limit, rather than the GPU's maximum.

The first part of stage 5 provides a tray and session-local single-instance guards
for wallpaper and settings; preview is independent. Autorun is opt-in through
Settings / System and affects only HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run's
TimeFlowsAway value. Startup tests use an isolated test key, never the real Run
entry. Additional-monitor rendering remains deferred. Pausing freezes clock
digits and the current interpolated bars while
maintenance/config checks continue every two seconds. Pause is temporary and
resets after a renderer restart. Running requests 30/60 FPS for smooth bars, or
one repaint per second with animation off. When a visible foreground window covers
the primary work area, wallpaper reduces to one-second updates; occlusion is
checked every two seconds. This is a rectangle-based optimization, not complete
occlusion detection. Windows lock/display-power notifications reduce the worker
to a two-second heartbeat; the existing lifetime monitor wakes it after a state
change. Settings preview reduces to clock ticks while its window is unfocused.
Frame deadlines compensate egui's predicted-frame subtraction, preventing the
60 FPS request from becoming a continuous immediate-repaint loop.
Interactive egui events can request additional paints.

Use `time-flows-away-native.exe --performance` for aggregated diagnostics every
30 seconds in app.log: actual UI-update FPS, target FPS/mode, average and maximum
update time, interval p95/p99/max, slow 60 FPS intervals (>20/>25 ms), layout/raster
misses and elapsed layout/raster/filter/texture-preparation stages. These intervals
are measured between UI updates, not SwapBuffers or DWM presentation. The backend
time includes the previous App::update; do not add it to update_ms or interpret it
as process CPU. Stage totals are elapsed time, not a CPU attribution. Glyph filter
is separate; emoji/whole-line fallback filtering is included in raster_total_ms.
Metrics windows reset on changes of activity/rate. Startup logs contain version,
commit and applied FPS/HQ; supervisor logs child PID, exit status and restart decision.
CI artifacts include build-manifest.json with the executable SHA-256.

Windows text is shaped by DirectWrite into actual glyph IDs, fallback font faces,
advances, offsets and bidi positions. Grayscale glyph bitmaps retain the 3x HQ
supersampling and premultiplied Lanczos filtering; shared texture pages avoid a
separate draw call/texture per digit. A maximum of 512 glyphs and a shared 32 MiB
text/emoji texture budget bound the cache. Font, quality, DPI and configured size
changes clear it. Sideways/inline/decorated/unsupported layouts retain a whole-line
fallback. Color emoji remain on the existing Windows color-font path. Arbitrary
Unicode is shaped as a string, never measured one character at a time.

Background decode/filter runs on one sleeping worker with one pending request and
one result. Superseded results are discarded; a prior texture stays until a new
image is ready. JPEG uses native 1/2, 1/4 or 1/8 DCT scaling before final Lanczos
filtering. Non-interlaced PNG is decoded by rows, with horizontal downsampling
before allocating the intermediate crop. Alpha is filtered premultiplied. EXIF
orientation and central cover crop are preserved. Adam7/oriented PNG and uncommon
JPEG formats use the bounded image decoder fallback; their peak memory can remain
higher, as can coefficient buffers in progressive JPEG. There is no universal
150 MiB limit. Source dimensions remain bounded to 8192 and textures to 4096.

Use `Measure-Performance.ps1` for per-PID/role and TOTAL CPU/RAM/GPU readings,
with settings closed and Wallpaper Engine stopped for the baseline. CPU uses
monotonic intervals and fractional accumulated CPU time normalized across logical
processors. Initial samples/new PIDs are unavailable, not zero; totals are complete
only when all current PIDs have a CPU baseline. GPU columns are empty if counters
are missing. CSV numbers use invariant culture and UTF-8 BOM in PS 5.1/7. Default
sampling interval is one second. Discard startup samples and compare identical
resolution, DPI, content and actual frame rates; use TOTAL rows for overall load.

The normal build preserves eframe's Windows context-rebinding workaround for
https://github.com/emilk/egui/issues/4289. `Build-GlContextPrototype.ps1` copies the
source and eframe into a fresh directory and builds an isolated experimental fast
path. Compilation is insufficient: compare native CPU and validate desktop/preview,
settings/file picker, resize/minimize/restore, DPI and Explorer recovery before
considering this change for the normal build. See OPTIMIZATION-CHECKLIST.md.
`Inspect-TrayAccessibility.ps1` records native menu UIA Raw/Control and MSAA data;
Narrator and keyboard activation still require Windows acceptance. Menu mnemonics
are Settings/Pause/Resume/Quit (S/P/R/Q); string menu items retain native accessibility.

Settings launched from the tray track an opened handle to the persistent
supervisor, so they survive renderer recovery but close when the application
exits. Quit also posts WM_CLOSE to the settings HWND after verifying its executable
path, including a window started with --settings. Standalone settings continue
working when no wallpaper was started. No process termination is used.

## Windows manual acceptance checklist (not run in Linux)

Use Windows 11 **24H2 or newer** and record the build number and GPU/driver:

1. Confirm double-clicking the executable creates no terminal. On several starts,
   verify the first visible frame is already centered at full desktop size;
   watch for a temporary small clock in the corner or activation of another window.
   With another application in front, observe for at least a minute: the wallpaper
   must not flash over it. If it does, keep `app.log` for diagnosis.
2. Run `--preview`: heading, date, ticking seconds, five colored progress bars,
   footer. Capture a screenshot at 100% DPI and compare against the web reference.
3. Run without arguments: content behind icons, icons visible and clickable;
   select/drag icons, use desktop context menu, and toggle icon visibility.
   Verify wallpaper is absent from Alt+Tab/taskbar and does not take focus.
4. Press Win+D twice; wallpaper remains on desktop and never covers normal apps.
5. Restart Windows Explorer manually in Task Manager. Confirm old renderer exits,
   a new one attaches, no duplicate renderer accumulates, icons remain usable.
   If preview appears before Shell is ready, restart the app and record the result.
6. Change primary resolution, DPI (100%, 125%, 150%) and primary display assignment.
   Verify bounds, crisp text, bar proportions and layering; other displays are
   intentionally outside MVP scope.
7. Hold Ctrl+Alt+Shift+Q for >=1s. Both supervisor and renderer should exit, no
   leftover window or desktop artifacts, no modified wallpaper setting.
8. Observe CPU/RAM/GPU in Task Manager for at least 60 seconds at idle and report
   both processes together. The user's current measurements are recorded in
   VALIDATION.md; no independent cloud Windows measurements are available.

**Unverified risks:** DX12 flip-model presentation into a layered child on raised
Shell may be black/invisible despite successful Win32 calls; compilation cannot
establish correct composition. If seen, stop acceptance and investigate a native
D3D11/DXGI blit renderer or a verified Shell-compatible surface path. No D3D11
fallback is implemented yet. Explorer destruction/recreation timing, DPI-awareness
compatibility with SetParent, click-through and icon-hidden hierarchy changes
also require these manual tests. The latest user OpenGL comparison is recorded in VALIDATION.md; prolonged
observation and stage 4/5 Windows checks remain pending.

Current automatic results are recorded in VALIDATION.md; they are separate from
this manual checklist and do not certify Windows desktop behavior.

## Research references

Lively Wallpaper `WinDesktopCore.cs` at commit
`25a0a5f4ed1988ff42117d3a17feb321062237fe`:
https://github.com/rocksdanister/lively/blob/25a0a5f4ed1988ff42117d3a17feb321062237fe/src/Lively/Lively/Core/WinDesktopCore.cs

Its raised-desktop commentary and observed hierarchy informed this independently
written integration. Its GPLv3 code is not copied into this Apache-2.0 project.
Microsoft Win32 API contracts:
https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setparent
https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowpos
https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setlayeredwindowattributes
https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw
