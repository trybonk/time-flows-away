# MVP validation

Validated in the Codex Linux cloud environment using Rust 1.99.0 and the committed
Cargo.lock. The user confirmed that the first Windows build renders the wallpaper and supplied
a comparison screenshot. They reported a small initial clock in the corner before
it grew/centered, one brief appearance over other windows, and a persistent console.
This follow-up removes the console, initializes display-sized rendering before
first visibility, preserves child styles across winit updates, avoids unnecessary
Shell reordering, and adjusts vertical spacing. Full Windows desktop acceptance
and manual verification of these fixes are **pending**.

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
| GitHub Actions workflow | Added; actual Windows MSVC runner result must be checked in GitHub Actions |
| Window style regression tests | Two additional Windows-only tests compile with the Win32 module; execution belongs to Windows CI |
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
binary hash, display resolution, GPU driver version, and event log were not
included in the report.

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
