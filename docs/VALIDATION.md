# Validation

The latest user comparison on Windows 11 25H2 / RTX 3060 reports that the
observed flashes **seem to have disappeared with OpenGL**. Two OpenGL starts
were logged at 16:58:37 and 17:00:33 on 2026-10-10, both NVIDIA OpenGL 3.3.0,
driver 616.64, with successful initial placement and normal style guards.
This is a user observation on one PC; the excerpt does not establish prolonged
stability, the original flash mechanism or a separate resolution of the earlier
one-second black background. OpenGL is now the Windows default; --wgpu retains
DX12 for comparison.

The settings/tray release needs its own Windows acceptance pass. Cloud compilation
and Linux rendering do not validate Windows tray, file picker, single-instance
behavior or desktop composition. See SETTINGS-CHECKLIST.md.

## Prior MVP cloud checks

The following table records validation before the settings/tray increment, using
Rust 1.99.0 and the then-committed Cargo.lock. The original startup/corner-frame
and console faults were corrected; later DX12 flashes prompted the comparison.

| Check | Result |
|---|---|
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed on Linux |
| `cargo test --locked` | 6 clock tests + 4 relative-order regression tests passed on Linux; 0 failed/ignored |
| `cargo build --locked --release` | Passed on Linux |
| `cargo clippy --locked --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | Passed, including Win32 module and supervisor |
| Windows GNU x64 release build | Cross-compiled; this does not prove Windows runtime behavior |
| Release Linux preview in Xvfb + Mesa Vulkan | Rendered background and all 5 bar colors; clock pixels changed; Esc exited with code 0 |
| Optional OpenGL Linux preview | Mesa llvmpipe OpenGL 4.5; same background/bar pixel checks, changing clock and Esc exit passed |
| Frozen setup script replay | Local pinned Rust and extracted Debian tools; release build steps repeatable |
| GitHub Actions workflow | Initial MVP and startup follow-up Windows MSVC runs succeeded; diagnostic follow-up requires its own run |
| Window style regression tests | Two Windows-only tests compiled locally and ran in the successful startup follow-up Windows CI; diagnostic follow-up CI pending |
| Original four web source files | Unchanged from initial checkout |

The preview was tested with real wgpu Vulkan rendering, not only an egui layout
simulation. Screenshot artifacts are outside the checkout under
`/workspace/artifacts/time-flows-away/`. Linux uses system Noto Sans CJK; Windows
loads its system JhengHei font.

Cross-build prerequisites: the pinned Rust Windows GNU target, signed Debian
MinGW packages extracted under `/workspace/toolchains/sysroot`, and
`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` pointing to that local GCC executable.
The inspected executable is PE32+ x86-64 and its import table contains Windows
system DLLs; no extra MinGW runtime DLL is imported. Do not treat that inspection
as an executed Windows smoke test.

Not executed in the cloud: Windows 11 24H2+ composition/layering, icons visible/hidden,
Explorer restart, Win+D, DPI/resolution changes, wallpaper-mode shutdown, or
CPU/RAM/GPU measurements. The exact checklist and known DX12/layered-window risk
are in DEVELOPMENT.md. Settings, tray, autorun and multi-monitor features are not part
of this MVP.

## User's Windows acceptance report — 2026-10-10

The user returned the manual checklist after the startup/console follow-up.
These are user-observed results, not cloud-executed Windows tests. The exact
binary hash and GPU driver version were not included in the report. The user
subsequently supplied the event log; placement entries identify 1920x1080 as the
normal display size and show a 1600x900 resolution test.

Environment: Windows 11 Pro 25H2, OS build 26200.6584; Intel Core i5-9400F,
32 GB RAM, NVIDIA RTX 3060 12 GB.

| Area | Reported result |
|---|---|
| Startup | No console, no small clock in the corner or abrupt scaling; five start/stop cycles consistent; foreground application retains focus |
| Rendering and clock | All text and bars visible, CJK glyphs correct, layout centered, local time/date and minute/hour rollovers correct |
| Desktop interaction | Icons visible/clickable/draggable, selection and context menu work; hiding/showing icons, Win+D, Alt+Tab exclusion, fullscreen return and keyboard focus passed |
| Display changes | Resolution and 100%/125%/150% scaling checks marked passed; restoring the original settings passed |
| Recovery | Explorer restart, lock/unlock and sleep/wake checks marked passed; no failure details reported |
| Preview and shutdown | Preview resizing and Esc passed; wallpaper exit chord terminates both processes; desktop returns normally and relaunch works |
| Resource use | Approximately 147 MB RAM and 0% CPU reported; no continuing memory growth or sustained high load observed; GPU figures not supplied |

Open issues take precedence over checked boxes:

- The user reports one brief flash after startup while another window is in
  front. The initial startup frame itself is now correct. The separate 15–30
  minute no-overlay observation was left unchecked. Timing and logs are needed
  to distinguish a placement/fallback event from a presentation/composition issue.
- While looking for Explorer in Task Manager, the wallpaper background changed
  from dark gray to black for about one second. The report does not establish
  whether this coincided with an Explorer restart or a GPU surface event.
- Changing the primary display with a second monitor was left unchecked.

Desktop acceptance remains incomplete until the two visual issues are resolved
and verified on Windows. These results support keeping the working desktop
integration while investigating the remaining faults; they do not establish
their cause or justify calling the follow-up fully stable.

The supplied log records initial placement at startup, the resolution changes
at 10:16:20 and 10:16:28, and an Explorer-destroyed HWND at 10:17:12 followed by
a fresh renderer/attachment at 10:17:14 (user's local timestamps, 2026-10-10).
There are no logged idle placement repairs or attachment/recovery errors in the
provided excerpt. That evidence supports the recovery/resizing checks but cannot
rule out toolkit window changes or presentation faults: those events were not
logged by that version, and exact flash times were not supplied.

The diagnostic follow-up aligns the GPU clear color with the opaque #161616
panel and records adapter/driver details, rate-limited surface errors, guarded
window-style/layer requests and visibility changes with millisecond timestamps.
The existing surface-error handling policy is preserved. The two visual issues
remain open until this build is retested on Windows; this is not a verified
compositor fix.

The user's diagnostic log at 2026-10-10 16:17 identifies an NVIDIA GeForce
RTX 3060, DX12 backend, driver 32.0.16.1664, and Bgra8Unorm. Initial visibility
and two style guards were recorded within 16 ms of attachment; the applied
styles retained child/nonactivating/layered behavior. The user reports that
flashes remain. No surface errors or subsequent placement events are present
in the supplied excerpt. Matching clear colors did not resolve the reported
flash, and this log does not establish a specific driver/compositor fault.

The next diagnostic step adds an opt-in `--opengl` backend using eframe/glow in
the same executable. Clock/layout, desktop attachment and lifecycle remain
shared; the supervisor preserves the flag when restarting its child. DX12 is
still the default. Windows OpenGL composition and the effect on flashes must
be tested manually; availability of this mode alone does not establish a fix.

The Rust package is now at the repository root. Native source files were
relocated without content changes in the structure commit, and original web
files/preview/README were verified byte-for-byte under legacy/wallpaper-engine/.
The CI workflow now runs root Cargo commands and packages root documents and
docs/. Root metadata, documentation links, formatting, ten Linux tests, Linux
and Windows GNU Clippy, both release builds, and both Linux preview backends
passed after relocation and addition of the comparison mode. The OpenGL preview
check waits for actual rendered pixels rather than assuming the first software
shader compilation finishes within two seconds. Windows runtime comparison
remains pending.

## Settings and tray increment — 0.2.0

Windows now defaults to OpenGL and retains --wgpu for DX12 comparison. Stage 4
adds an English settings window with explicit Save, live preview, versioned TOML,
colors/fonts/layout/date/time/background and countdown/elapsed controls. The
first stage 5 increment adds a worker-owned tray, pause/resume and session-local
wallpaper/settings instance guards. Autorun and multi-monitor selection remain
for later work.

Cloud checks on this increment:

| Check | Result |
|---|---|
| Formatting and Clippy | Passed on Linux and Windows GNU, all targets |
| Linux tests | 16 passed; 0 failed/ignored (clock, relative order, settings/storage/timer, CLI forwarding) |
| Release builds | Linux and Windows GNU x64 cross-builds passed |
| English settings UI, actual OpenGL/Xvfb rendering | Edited heading in the input, confirmed draft did not write a file, Ctrl+S saved it, live preview rendered |
| Pause / resume | Clock-region pixels stayed fixed while paused and changed after resume |
| Config reload | A changed solid color rendered; invalid TOML preserved that color and the invalid file; corrected settings reloaded |
| Assets and timer | PNG background, custom DejaVu font and enabled countdown rendered; missing PNG fell back to solid background without a crash |
| Preview shutdown | Esc exited with status 0 |
| PE inspection | Windows GUI subsystem; imports Windows system DLLs, including OpenGL and native-dialog/tray dependencies; no extra MinGW runtime DLL |
| Legacy and attribution | legacy/wallpaper-engine, LICENSE and NOTICE unchanged from the preceding branch |

The settings helper uses its own file under
/workspace/artifacts/time-flows-away/settings/. It waits for a mapped window
before X11 input and for rendered pixels when software OpenGL needs extra time.
This avoids treating a title/PID or a fixed shader-compilation delay as readiness.
Config polling now requests its own repaint deadline, avoiding an extra delay
from rounding the two-second check to a one-second clock tick.

These are Linux GUI checks and Windows cross-compilation. Windows MSVC CI must
complete on the new feature branch. Windows tray/menu callbacks, native file
picker, duplicate-launch guards, OpenGL desktop layering, Explorer recovery with
a tray, and performance of this release require the user's PC checks in
SETTINGS-CHECKLIST.md. The older Windows report is useful baseline evidence;
it does not certify the settings/tray build.
