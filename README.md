# Time Flows Away

[![Windows build](https://github.com/trybonk/time-flows-away/actions/workflows/native-windows.yml/badge.svg)](https://github.com/trybonk/time-flows-away/actions/workflows/native-windows.yml)

A native Windows desktop wallpaper with a clock, date, and five progress bars
for the year, month, day, hour, and minute. Written in Rust with egui. Runs behind
desktop icons without a browser or Wallpaper Engine.

Targets **Windows 11 x64, 24H2 or newer**. This is an early development release.
Desktop checks passed on a user's Windows 11 25H2 PC; they report that the
flashes observed with DX12 seem to have disappeared with OpenGL. Longer observation and checks on other hardware
remain necessary. See [validation results](docs/VALIDATION.md).

## Download and run

1. Open [Windows build](https://github.com/trybonk/time-flows-away/actions/workflows/native-windows.yml)
   and choose a successful run for the version you want. The settings release
   is developed on `feat/settings-and-tray`.
2. Download the `time-flows-away-windows-x64` artifact and extract the ZIP.
3. Double-click `time-flows-away-native.exe`. Administrator rights are not required.

OpenGL is the default renderer on Windows. Right-click the tray clock for
**Settings**, **Pause / Resume**, and **Quit**. Double-click it to open settings.
The tray icon may be in Windows' hidden-icons menu. Repeated wallpaper launches
are ignored. Wallpaper mode uses a supervisor and a renderer process; Explorer
recovery restarts the renderer. Settings run in a separate ordinary window.
Quitting the wallpaper closes its settings window too. An Explorer restart
keeps settings open.

| Mode | Start | Exit |
|---|---|---|
| Desktop wallpaper | Double-click the EXE | Tray **Quit**, or hold **Ctrl+Alt+Shift+Q** for at least one second |
| Window preview | `time-flows-away-native.exe --preview` | **Esc**; **Space** pauses/resumes |
| Settings | `time-flows-away-native.exe --settings`, or tray **Settings** | Close the window |
| DX12 diagnostic comparison | `time-flows-away-native.exe --wgpu` | Tray **Quit**, or the exit chord |

`--opengl` explicitly selects OpenGL; `--wgpu` selects DX12 on Windows and Vulkan
on Linux. Both can be combined with `--preview` or `--settings`.
The application opens without a console. Diagnostics are written to
`%LOCALAPPDATA%\TimeFlowsAway\app.log`.

## Customize

The English settings window provides a live preview. **Save** / **Ctrl+S** writes
`%LOCALAPPDATA%\TimeFlowsAway\config.toml`; running wallpaper loads changes within
two seconds. Closing settings discards unsaved edits. **Reset to defaults** changes
the preview until you save. Settings need not be open for wallpaper to run.

- Heading/footer, font file (TTF/OTF/TTC), four text sizes and text color.
- Color emoji from Windows' Segoe UI Emoji in wallpaper and live preview.
  Emoji keep their native colors when the text color or font changes. Input
  fields use monochrome system glyphs.
- Five progress colors, bar width/height/gap, horizontal/vertical alignment and offsets.
- High quality text and smooth progress bars at 30 FPS (default), optional 60 FPS,
  or animation off for lower resource use. Clock digits update once per second.
- Date/time formats, 12/24-hour presets and weekday.
- Solid or PNG/JPEG background, panel color and opacity.
- Countdown to a future local date/time or elapsed time since a past one;
  days, days + hours/minutes/seconds, or total seconds, with prefix/suffix and size.

Images are centered and cropped to fill the display. Layout sizes and offsets
use logical pixels; the layout shrinks to fit small windows. Weekday and automatic
timer labels are English. Timer durations use local civil time, including its
clock/DST changes. Use an absolute asset path; files remain on your PC.
Images are limited to 8192 pixels per side and a 256 MiB decode allocation;
fonts are limited to 50 MiB. An invalid asset falls back to a solid background
or the system font, with an error in settings. To retry a replaced asset at the
same path, clear and reselect its path.

The settings file is created when you save; a missing file uses original defaults.
Invalid settings edits keep the last valid configuration and are logged; the file
is preserved for correction. `--config "C:\path\config.toml"` selects another file
and is forwarded to settings and restarted workers. Settings and wallpaper each
allow one instance per Windows session; preview windows are independent.

In **System**, enable **Start with Windows** to start at sign-in. This switch
applies immediately, without Save or administrator rights. Keep the EXE at the
registered location; after moving it, use **Use current application location**.
The startup entry preserves the selected renderer and settings-file path.
Disabling it removes the application's startup entry without closing the wallpaper.

## Build from source

Install Rust and Visual Studio Build Tools with **Desktop development with C++**
and a Windows SDK. Run from the repository root:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
.\target\release\time-flows-away-native.exe --preview
.\target\release\time-flows-away-native.exe --settings
```

The toolchain is pinned in `rust-toolchain.toml`; dependencies are committed in
`Cargo.lock`. GitHub Actions checks formatting, linting and tests, builds Windows
MSVC and packages the executable and documentation. Pushes to `feat/**`, master,
pull requests and manual runs are supported. Compilation does not establish
that desktop embedding works on a particular PC.

## Project layout

```text
Cargo.toml / Cargo.lock     Rust package and dependency lockfile
rust-toolchain.toml         Pinned development toolchain
src/                       Native application, settings, desktop integration and rendering
docs/                      Development, manual checks and validation results
legacy/wallpaper-engine/   Preserved original web wallpaper and README
.github/workflows/         Windows CI and downloadable build artifacts
LICENSE / NOTICE           Apache-2.0 license and original-project credits
```

Primary-monitor rendering, Explorer recovery, display/DPI change detection and
opt-in autorun are implemented. Additional-monitor rendering and installation/
release packaging remain planned. Startup registration is disabled until enabled
in settings. No administrator privileges are required.
Read [development details](docs/DEVELOPMENT.md) and the
[settings and tray checklist](docs/SETTINGS-CHECKLIST.md).

## License and attribution

Licensed under [Apache-2.0](LICENSE). The native design is based on
[twenty-zhang/time-flows-away](https://github.com/twenty-zhang/time-flows-away).
See [NOTICE](NOTICE) for original wallpaper credits and research attribution.
The original web files are preserved as a reference; they are not used by the EXE.
