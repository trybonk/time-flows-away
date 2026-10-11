# 0.3.2 optimization acceptance on Windows

Record Windows/GPU/driver, screen resolution and DPI. Keep the prior build for
A/B comparison. Stop Wallpaper Engine for the main baseline; test coexistence
separately. This is a manual acceptance checklist, not a claim of measured gains.

1. Verify `build-manifest.json` version/commit and compare its SHA-256 with
   `Get-FileHash .\time-flows-away-native.exe`. Start with `--performance`.
   Check app.log Build and Settings applied entries against the actual configuration.
2. Run default and mixed emoji content at HQ/LQ, Off/30/60 FPS for two minutes each,
   settings closed. Discard the first 30 seconds. Collect CSV:
   `./docs/Measure-Performance.ps1 -DurationSeconds 120 -Label visible-hq-60 -OutputPath hq60.csv`.
   Compare TOTAL rows at the same actual FPS. Record p99/max/slow20/slow25 and cache
   misses. After digit warmup, ordinary seconds should reuse glyphs. Quantiles are
   UI-update cadence; collect a new Nsight preview trace to assess SwapBuffers.
   The target is removal of periodic one-second stalls, not zero system jitter.
3. Compare identical paused text at 100/125/150/200% DPI: numerals/colon, jÁg,
   Cyrillic/CJK, Arabic/Hebrew, custom proportional/italic font and color emoji
   🤫 🎣 👍🏽 👩‍💻. Check glyph positioning, spaces/kerning, clipping, halos and
   partial edge coverage. Exercise 59->00, 12/24h, fractional formats and timers.
4. Toggle Off/Pause/covered/suspended, restore and resume. Check mode logs and
   responsiveness. Off/covered should approach one paint per second in steady
   state; Pause may retain a two-second heartbeat. UI events can add frames.
5. Hold Ctrl+Alt+Shift+Q for 0.5 s: app stays. Hold for 1.2 s: all app processes exit.
   Repeat in Pause/covered, with Settings active/minimized/closed. Repeat tray Quit
   and window-close paths. Check Exit reason, Renderer exited PID/status/decision.
6. Save 30->60->30 and confirm applied FPS within ~2 s. Repeat overwriting the file
   while preserving its byte length and LastWriteTimeUtc, then using atomic rename.
   Invalid TOML/oversized file must keep the last valid settings; restoring valid
   content recovers. An unchanged invalid file must not flood logs.
7. Load and clear 8K JPEG/baseline+progressive and PNG/RGBA+palette+16-bit+Adam7 ten
   times. Verify center cover crop and EXIF orientation, responsiveness while loading,
   retention of prior image, last selection winning after rapid edits, and clearing
   during a load. Record per-PID Private/WS peak; compare each image class separately.
   Adam7/oriented PNG and progressive JPEG may still peak higher than the common path.
8. Run visible HQ60 for 30–60 minutes; sample at one second, watch Private/WS/handles/
   threads and periodic stalls. Repeat 30–50 starts after normal Quit/chord, with
   Settings and without. Record a video over a white foreground window. A vanished
   process needs its exit status from an external observer if no Exit log exists.
9. Move the EXE folder after exiting. Settings/System should warn about the old
   startup location. Use `Use current application location`, then verify enable/
   disable and a Windows logon, including paths with spaces/non-ASCII characters.
10. Open the tray menu and run `./docs/Inspect-TrayAccessibility.ps1` from a second
    terminal without dismissing the menu. Check UIA Raw/Control, MSAA names/roles
    and Narrator. Arrow keys/Enter and S/P/R/Q must activate the correct actions.
    UIA showing only Pane is insufficient to establish a failure of the native menu.
11. Repeat desktop click-through, icon visibility, Win+D, lock/unlock, sleep/wake,
    display power, resolution/DPI and Explorer restart. Settings survives renderer
    recovery and closes on whole-app exit. No focus steal/foreground flash.

Optional context experiment (source checkout with Rust installed):
`./docs/Build-GlContextPrototype.ps1`. It creates a separate experimental build,
leaving the normal checkout/dependency unchanged. Repeat step 2 and all of step 11,
plus preview/settings/file-picker/resize/minimize/restore. Keep the current eframe
workaround unless the experiment provides a measurable gain without regressions.
