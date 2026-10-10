# Settings and tray — Windows manual checks

Record the commit/build, Windows version, GPU/driver and display scale. Close the
previous version before replacing its EXE. Use default OpenGL first; keep app.log
if anything flashes, disappears or steals focus.

- [ ] Start normally five times. No terminal, corner frame or foreground flash;
      clocks appear behind usable desktop icons. Observe for 15–30 minutes with
      other applications in front, including Task Manager.
- [ ] Find the tray clock (including hidden icons). Right-click Settings, or
      double-click the icon. The settings window is English and appears normally.
- [ ] Open Settings repeatedly: one settings window. Close/reopen it; wallpaper
      continues. Run the EXE again: no extra supervisor/renderer or tray icon.
- [ ] Edit heading/footer including Cyrillic/CJK, colors, font sizes and all five
      bars. Live preview changes. Before Save, desktop wallpaper stays unchanged.
- [ ] Enter 🤫 and 🎣, mixed with ordinary text, plus 👍🏽 and 👩‍💻. They are
      colored in the preview and, after Save, on the wallpaper. Changing text
      color/font preserves emoji colors; input fields show monochrome glyphs.
- [ ] Save / Ctrl+S: wallpaper updates within two seconds. Close/reopen settings
      and restart the application: saved values persist.
- [ ] Change left/center/right, top/center/bottom, X/Y offsets, width/height/gap.
      Check fit in resized preview and at 100%, 125%, 150% display scaling.
- [ ] Select 12/24-hour presets, weekday and a custom date format. A bad format
      is rejected without crashing; correcting it allows Save.
- [ ] Browse for a PNG and a JPEG, including a path with spaces/non-ASCII letters.
      Image fills the screen with centered cropping; mask opacity/color work.
      Clear the path: solid color returns. Missing/corrupt image gives an error
      and a solid background; an invalid font falls back without a crash.
- [ ] Browse for a TTF/OTF/TTC font. Text uses it; clearing restores the system
      font. Check the chosen font contains your label characters.
- [ ] Enable a future timer in each display mode, then a past target. Prefix,
      suffix and timer size work. Test crossing a target one minute ahead.
      Invalid dates such as 2026-02-30 are rejected.
- [ ] Reset to defaults: preview resets; desktop changes only after Save.
      Close with unsaved edits: previous saved settings remain on reopening.
- [ ] While running, edit config.toml to invalid TOML. Wallpaper keeps its last
      valid state and logs the error. Correct and save it: changes reload.
- [ ] Pause from tray: seconds and bars stop. Settings and desktop icons still
      work. Resume: current time appears, without a catch-up animation.
- [ ] While paused, restart Explorer or change resolution. Recovery works and
      only one tray icon/renderer remains. Explorer destruction creates a fresh
      renderer in the running state; the pause state is intentionally temporary.
- [ ] Leave Settings open while restarting Explorer. Settings remain open;
      opening them again from the recovered tray focuses the same window.
- [ ] Lock/unlock, sleep/wake, Win+D, hide/show icons and fullscreen return. Check
      wallpaper never covers applications and tray still responds.
- [ ] Quit from tray, then repeat using Ctrl+Alt+Shift+Q (hold at least one second).
      Both wallpaper processes and the open settings window end, tray disappears
      and desktop stays usable. Repeat with Settings minimized and immediately
      after opening Settings. Unsaved edits are discarded as when closing Settings.
- [ ] Compare total CPU/RAM/GPU at idle and paused for at least a minute. Report
      both wallpaper processes and the settings process separately.

Autorun and additional-monitor rendering are outside this release. DX12 can be
compared with --wgpu; the previous flashes occurred with this path on an RTX 3060.
