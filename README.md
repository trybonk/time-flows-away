# Time Flows Away

[![Windows build](https://github.com/trybonk/time-flows-away/actions/workflows/native-windows.yml/badge.svg?branch=feat%2Fnative-win11-wallpaper)](https://github.com/trybonk/time-flows-away/actions/workflows/native-windows.yml)

A native Windows desktop wallpaper with a clock, date, and five progress bars
for the year, month, day, hour, and minute. Written in Rust with egui and wgpu.
The application runs behind desktop icons without a browser or Wallpaper Engine.

Targets **Windows 11 x64, 24H2 or newer**. This is an early MVP: most desktop
checks passed on a user's Windows 11 25H2 PC, while two transient visual issues
remain under investigation. See [validation results](docs/VALIDATION.md).

## Download and run

1. Open [Windows build](https://github.com/trybonk/time-flows-away/actions/workflows/native-windows.yml)
   and choose a successful run for `feat/native-win11-wallpaper`.
2. Download the `time-flows-away-windows-x64` artifact and extract the ZIP.
3. Double-click `time-flows-away-native.exe`. Administrator rights are not required.

Close an existing copy before launching another. Single-instance protection is
not implemented yet. Wallpaper mode normally has two processes: a supervisor
and the renderer, which can be restarted after Explorer recreates the desktop.

| Mode | Start | Exit |
|---|---|---|
| Desktop wallpaper | Double-click the EXE | Hold **Ctrl+Alt+Shift+Q** for at least one second |
| Window preview | `time-flows-away-native.exe --preview` | **Esc** |

The application opens without a console. Diagnostics are written to
`%LOCALAPPDATA%\TimeFlowsAway\app.log`. For a visual fault, include this log,
the event time, Windows build, display settings, and GPU/driver information.

## Build from source

Install Rust and Visual Studio Build Tools with **Desktop development with C++**
and a Windows SDK. Run these commands from the repository root:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
.\target\release\time-flows-away-native.exe --preview
```

The Rust toolchain is pinned in `rust-toolchain.toml`; dependency versions are
committed in `Cargo.lock`. GitHub Actions performs formatting, linting, tests,
and a Windows MSVC release build, then packages the executable and documentation.
Compilation does not establish that desktop embedding works on a particular PC.

## Project layout

```text
Cargo.toml / Cargo.lock     Rust package and dependency lockfile
rust-toolchain.toml         Pinned development toolchain
src/                       Native application, desktop integration, clock and rendering
docs/                      Architecture, development, manual checks and validation results
legacy/wallpaper-engine/   Preserved original web wallpaper and README
.github/workflows/         Windows CI and downloadable build artifacts
LICENSE / NOTICE           Apache-2.0 license and original-project credits
```

The original web files are retained as a reference. They are not used by the
native executable. Their contents and the root license are preserved.

## Current scope

- Date and local 24-hour clock with the original Chinese heading and English footer.
- Year/month/day/hour/minute progress, using actual calendar month/year lengths.
- Primary-monitor wallpaper behind desktop icons, with a preview fallback.
- Explorer recovery, display-size maintenance, and explicit shutdown controls.

Progress bars update once per second. Settings, tray controls, autorun,
single-instance protection, and multi-monitor rendering are planned after the
Windows MVP is stable. A brief post-startup flash and a one-second black
background reported on Windows remain open; the current build adds diagnostics.

Read [development and manual testing](docs/DEVELOPMENT.md) for implementation
details, platform limitations, and the Windows acceptance checklist.

## License and attribution

Licensed under [Apache-2.0](LICENSE). The native design is based on
[twenty-zhang/time-flows-away](https://github.com/twenty-zhang/time-flows-away).
See [NOTICE](NOTICE) for original wallpaper credits and research attribution.
