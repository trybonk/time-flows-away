# Validation

## 0.3.1 optimization and antialiasing

This increment adds frame deadlines, a smaller font atlas, cached text layouts
and whole-second strings, a shared text/emoji texture budget, viewport-sized
background images, and reduced repainting when settings are unfocused or Windows
reports a locked session / powered-off display. Window placement and the OpenGL
surface configuration are preserved.

Windows ordinary text now uses DirectWrite with grayscale antialiasing, up to 3x
supersampling and Lanczos reduction to physical size. Color emoji receive the same
quality reduction instead of relying on bilinear minification of a large texture.
Wallpaper no longer puts complete CJK/Segoe emoji files into egui. Linux still uses
its egui/Noto development fallback, so Linux screenshots cannot establish the
appearance or RAM savings of the Windows text path.

Local checks: formatting, Linux Clippy and 25 executed unit tests passed; Windows
GNU all-target Clippy and release cross-compilation passed. Actual Linux release
checks exercised OpenGL rendering, colored mixed text/emoji, interpolation while
digits stayed fixed, pause/resume, quality toggles, animation off, settings/save,
last-valid reload, custom font/image/timer and clean exit. The background startup
and resize debounce follow-up also passed settings and Vulkan preview checks. Native
DirectWrite pixel coverage, custom-font loading, texture bounds and absence of
system-font copies also passed Windows MSVC tests in
[run 38089263496](https://github.com/trybonk/time-flows-away/actions/runs/38089263496)
for source commit `a16a83f`. Formatting, Clippy, the measurement-helper smoke,
release build and executable/documentation artifact upload all passed in that run.

Actual Linux release measurements used the same Xvfb/Mesa llvmpipe setup and
default scene as 0.3.0, high quality text enabled, eight seconds of warmup followed
by a minute of sampling per mode. CPU below is **percent of one logical CPU**, not
Windows Task Manager's percentage of the whole machine. RSS is process working
set in MiB; software rendering also puts GPU resources into system RAM.

| Mode / configured FPS | 0.3.0 CPU | 0.3.1 CPU | 0.3.0 steady RSS | 0.3.1 steady RSS |
|---|---:|---:|---:|---:|
| Preview / Off | 14.56% | 5.22% | 140.50 | 119.07 |
| Preview / 30 | 177.61% | 91.77% | 144.26 | 120.24 |
| Preview / 60 | 257.06% | 182.38% | 144.11 | 120.07 |
| Settings / 30, focused | 184.37% | 91.86% | 144.71 | 128.58 |

No continuing RSS growth occurred in these short intervals. A separate 30 FPS
check logged 30.0 update FPS after warmup, with interval p95 34.92 ms. The 60 FPS
software-rendered check logged 55.7 FPS and p95 25.33 ms; focused settings logged
29.1 FPS. The preview atlas was 4096x256 (4 MiB), compared with the earlier
16384x256 (16 MiB). CPU comparisons use the same **configured** FPS; the previous
deadline bug overproduced frames, so these are not throughput measurements at
identical actual FPS. These software-driver numbers cannot establish Windows
RTX 3060 CPU/GPU percentages or exact RAM savings. Windows also uses a different
ordinary-text path and avoids the full system-font copies retained on Linux.

A separate warmed release headless probe of the same default CJK scene reduced
scene construction from 24.90 to 9.42 microseconds per update and allocation calls
from 134.2 to 42.6 per update. Tessellation changed from 8.77 to 6.09 microseconds;
requested temporary allocation bytes changed from 67.6 to 49.6 KiB per update.
This supports the text/scene cache change, but those costs were already too small
to explain all of the user's reported CPU load. It is not a Windows GPU benchmark.

Use the focused 0.3.1 checklist and `--performance` alongside the CPU/RAM CSV
helper for hardware measurements and a 30-minute stability check. User validation
of 0.3.0 does not validate the new text appearance, lock/power notifications,
desktop composition, actual frame rate or hardware resource use.

## Earlier development results

The latest user comparison on Windows 11 25H2 / RTX 3060 reports that the
observed flashes **seem to have disappeared with OpenGL**. Two OpenGL starts
were logged at 16:58:37 and 17:00:33 on 2026-10-10, both NVIDIA OpenGL 3.3.0,
driver 616.64, with successful initial placement and normal style guards.
This is a user observation on one PC; the excerpt does not establish prolonged
stability, the original flash mechanism or a separate resolution of the earlier
one-second black background. OpenGL is now the Windows default; --wgpu retains
DX12 for comparison.

The user subsequently confirmed emoji/settings shutdown fixes and reported the
existing settings/tray checklist looked correct. This is user acceptance of the
previous build; new 0.3.0 startup/quality/display changes need focused checks.
Cloud compilation
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

These are Linux GUI checks and Windows cross-compilation. Windows MSVC CI passed
in [run 38060606087](https://github.com/trybonk/time-flows-away/actions/runs/38060606087).
Windows tray/menu callbacks, native file
picker, duplicate-launch guards, OpenGL desktop layering, Explorer recovery with
a tray, and performance of this release require the user's PC checks in
SETTINGS-CHECKLIST.md. The older Windows report is useful baseline evidence;
it does not certify the settings/tray build.

## Emoji and settings shutdown fixes — 0.2.1

The user reported missing 🤫, monochrome 🎣, and a settings window remaining
after tray Quit. Wallpaper/live preview now paint native color emoji, and the
settings process follows the persistent wallpaper supervisor. Normal Quit also
requests closure of the verified settings HWND. Explorer recovery leaves the
supervisor alive and keeps settings open.

Checks before submission:

- Formatting and all-target Clippy passed for Linux and Windows GNU.
- 18 Linux tests passed, including emoji sequences/text presentation, transparent
  image cropping, invalid internal owner arguments and child argument forwarding.
- Actual OpenGL/Xvfb settings and wallpaper previews displayed colored 🤫 and 🎣
  alongside purple ordinary text. Header contained 14,576 face-colored pixels;
  footer contained 2,249 fishing-blue pixels. This used Linux's installed Noto
  Color Emoji font and the new shared layout/texture code, not Windows glyphs.

Windows CI additionally exercises native DirectWrite/Direct2D/WIC rendering of
🤫, 🎣, 👍🏽 and 👩‍💻, asserting colored pixels and transparent background, and
checks settings lifetime handles against a live, exited and missing owner.
The build run provides the native test/release result. Windows tray closure with
Settings open/minimized and preservation across Explorer restart still require
the focused checks in SETTINGS-CHECKLIST.md on the user's PC.

Windows MSVC CI passed in
[run 38065972038](https://github.com/trybonk/time-flows-away/actions/runs/38065972038).
On 2026-10-10 the user confirmed these fixes worked, then reported the earlier
manual settings/tray checklist was completed and appeared correct. The exact
checks/version hash were not supplied, so this is reported acceptance rather
than individually instrumented Windows results.

## Startup, quality and primary display recovery — 0.3.0

Adds an explicit per-user autorun switch, 2x text rasterization, optional 30/60 FPS
bar interpolation (30 default), low-power animation-off mode, foreground-cover
throttling, primary-display/DPI/power detection and bounded Shell recovery.
Multiple-monitor rendering and release publication remain deferred by request.

Linux tests cover intermediate bar frames, minute rollover, pause/resume and time
jumps; config tests preserve old partial files and reject unsupported frame rates.
Windows tests additionally cover native command-line Unicode/path quoting and
startup-value write/read/removal under an isolated HKCU test key. CI exercises the
PowerShell measurement helper using its own pwsh process; this checks the helper,
not wallpaper resource use. Windows desktop composition, sign-in autorun,
hardware GPU load and quality comparisons require the focused PC checks in
SETTINGS-CHECKLIST.md.

Current-instance checks passed with the pinned toolchain:

- Linux and Windows GNU all-target Clippy; 20 Linux tests, no failures/ignored tests.
- Linux and Windows GNU release builds; Windows executable is GUI-subsystem x64
  and imports system DLLs without an extra MinGW runtime dependency.
- Actual release OpenGL settings edits/Save, pause/resume, last-valid hot reload,
  image/font/timer rendering, colored emoji, and OpenGL/Vulkan preview shutdown.
- Actual OpenGL animation check found 17 changing bar frames while clock digits
  stayed unchanged. Pause froze bars/digits; animation off had 15 stable frames;
  quality on/off changed rasterization of the same paused text.
- Four one-minute software-OpenGL measurements covered preview off/30/60 FPS and
  Settings at 30 FPS. Post-warmup process RSS remained approximately 140–145 MiB
  without sustained growth in those intervals. The software driver consumed much
  more CPU in smooth modes; these readings cannot predict RTX 3060 CPU/GPU load.
- The PowerShell helper's parsing, CPU/RAM CSV sampling and unavailable-GPU output
  were checked using official, checksum-verified PowerShell 7.5.4 in Linux.

Windows MSVC formatting/Clippy/tests passed in run 38074114699; that run stopped
before release packaging because cold GPU-counter discovery consumed the short
helper smoke interval. The corrected CI smoke measures CPU/RAM with SkipGpu;
ordinary PC measurements retain GPU sampling. Missing counters and initial CPU
samples are omitted from averages instead of being reported as zero use.
The follow-up Windows run supplies release packaging results.


## 0.3.2 — diagnostics-driven optimization (2026-10-11)

Source implementation: `2282d53` plus performance-mode correction `32ad6a0`
on `feat/settings-and-tray`.

Implemented native shaped-glyph atlases, 3x HQ filtering, asynchronous latest-request
background loading, early JPEG DCT scaling and row-wise ordinary PNG downsampling.
Config reload now detects changed bytes with equal size/mtime and retains valid
settings on errors. Exit chord uses a monotonic one-second hold on the existing
monitor thread, independent of paint cadence. Added lifecycle/PID/status logs,
p99/max/slow-frame and cache/stage diagnostics, whole-second config poll phase,
startup-path warnings and native menu mnemonics. The normal eframe Windows context
workaround remains enabled. Accessibility inspector and an isolated context-fast-
path prototype build script provide the next Windows-specific checks.

Checks completed in this cloud instance:

- 33 Linux tests passed, none ignored. Linux and Windows GNU all-target Clippy
  passed with warnings denied; release builds for both targets completed.
- PowerShell 7.5.4 arithmetic, GPU-kind selection, invariant/Unicode/BOM CSV and
  temporary output-lock retry checks passed. Live per-PID/TOTAL CPU/RAM sampling
  produced four TOTAL samples; fractional CPU matched deltas/monotonic intervals,
  and unavailable GPU values remained empty.
- Release OpenGL GUI check: five interpolated frames while digits stayed unchanged;
  pause froze bars/digits; HQ/LQ changed the same paused text rasterization; Off
  retained twelve stable sampled frames; Escape exited with code zero. This
  development check exercises Linux fonts, not Windows DirectWrite quality.
- Release OpenGL GUI loaded synthetic 7680x4320 baseline JPEG and RGBA PNG,
  displayed the correct colors, cleared the background, detected 30->60 FPS with
  original byte length/mtime preserved, and exited cleanly. Linux software-GL RSS
  was ~119 MiB baseline / ~133 MiB after JPEG / ~163 MiB after PNG; cumulative
  VmHWM ~122/~147/~163 MiB. These are one short functional run on a different
  machine/driver, not a before/after Windows benchmark or a universal memory limit.
- An isolated eframe 0.33.3 Windows context-fast-path prototype compiled with the
  Windows GNU target. Its rendering/recovery behavior and CPU benefit remain
  unverified; no dependency workaround was removed from the normal build.
- Windows MSVC formatting, Clippy, native tests (including glyph reuse and native
  shaping/color coverage), PowerShell 5.1/7 arithmetic/CSV/lock tests and live
  CPU/RAM helper smoke passed in Actions run 38175412539. Release, packaging and artifact upload also passed. The final correction
  `32ad6a0` passed the complete Windows workflow, including release, packaging and
  upload, in run 38176079061. That artifact is the final 0.3.2 acceptance build.

- A live debug OpenGL diagnostic window with configured 60 FPS was paused for
  33 seconds. Its aggregate had target_fps=0, actual UI cadence ~1.1/s, idle
  interval max ~1968 ms, and slow20=slow25=0. Changing HQ to LQ emitted a new
  paused metrics window; Escape exited with code zero. This validates actual
  aggregation/classification, not only the statistics helper in isolation.

Live Windows acceptance remains necessary: CPU/p99 before-and-after at identical
actual FPS, startup/foreground flashes, chord behavior in idle/covered/settings,
long soak, sleep/lock/Explorer/DPI transitions, same-text quality screenshots,
startup repair and Narrator/MSAA menu behavior. See OPTIMIZATION-CHECKLIST.md.
Adam7/oriented PNG and uncommon JPEG use a full-decode fallback; progressive JPEG
coefficient buffers can remain large. Do not claim every 8K load fits 150 MiB or
that resource growth/Windows rendering issues are ruled out by these tests.

## First pre-release preparation (2026-10-12)

The user reported that the latest downloaded Windows build works correctly and
authorized the first pre-release. The native application code is unchanged from
the accepted 0.3.2 implementation. Earlier deferral of release publication is
superseded by this authorization; detailed Windows scenarios above remain the
record of what has and has not been measured.

The old web `master` commit is preserved on `legacy`. Native/settings history is
combined on the new `master` without discarding commits. Release automation now
validates the version tag and master ancestry, repeats the Windows checks, and
publishes a versioned ZIP, ZIP SHA-256 and executable build manifest as a
pre-release. The tag build supplies the Windows MSVC artifact; local cross-built
binaries are used only to exercise packaging.

Release preparation checks: formatting, all-target Linux Clippy with warnings
denied and all 33 Linux tests passed. Workflow structure and embedded Bash,
Python and PowerShell syntax passed. Packaging a real Windows GNU executable
produced the expected ZIP contents and matching hashes; the publisher rejected a
deliberately corrupted executable before publication. Windows MSVC/tag-job
results are available in the release's linked Actions run and build manifest.
