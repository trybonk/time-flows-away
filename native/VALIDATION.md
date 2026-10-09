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

Not executed: Windows 11 24H2+ composition/layering, icons visible/hidden,
Explorer restart, Win+D, DPI/resolution changes, wallpaper-mode shutdown, or
CPU/RAM/GPU measurements. The exact checklist and known DX12/layered-window risk
are in README.md. Settings, tray, autorun and multi-monitor features are not part
of this MVP.
