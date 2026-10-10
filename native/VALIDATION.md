# MVP validation

Validated in the Codex Linux cloud environment using Rust 1.99.0 and the committed
Cargo.lock. The first Windows feedback identified corner/size changes at startup,
a persistent console, and a brief appearance over other windows. The subsequent
manual report confirms the startup and console fixes and most desktop checks.
It also identifies a remaining post-startup flash and a one-second black
background. Results and log evidence are recorded below; full Windows desktop
acceptance remains **pending resolution of those visual issues**.

| Check | Result |
|---|---|
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed on Linux |
| `cargo test --locked` | 6 clock tests + 4 relative-order regression tests passed on Linux; 0 failed/ignored |
| `cargo build --locked --release` | Passed on Linux |
| `cargo clippy --locked --target x86_64-pc-windows-gnu --all-targets -- -D warnings` | Passed, including Win32 module and supervisor |
| Windows GNU x64 release build | Cross-compiled; this does not prove Windows runtime behavior |
| Release Linux preview in Xvfb + Mesa Vulkan | Rendered background and all 5 bar colors; clock pixels changed; Esc exited with code 0 |
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
are in README.md. Settings, tray, autorun and multi-monitor features are not part
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
