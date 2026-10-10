//! The worker owns the tray icon on the same thread as its Win32 event loop.
use eframe::egui;
use std::sync::mpsc::{self, Receiver};
use tray_icon::{
    Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
pub enum Command {
    Settings,
    TogglePause,
    Quit,
}
pub struct Tray {
    _icon: TrayIcon,
    pause: MenuItem,
    events: Receiver<Command>,
}
impl Tray {
    pub fn new(ctx: &egui::Context) -> Result<Self, String> {
        let menu = Menu::new();
        let settings = MenuItem::new("Settings…", true, None);
        let pause = MenuItem::new("Pause", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append_items(&[&settings, &pause, &PredefinedMenuItem::separator(), &quit])
            .map_err(|e| e.to_string())?;
        let mut rgba = vec![0u8; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                let circle = ((x as i32 - 16).pow(2) + (y as i32 - 16).pow(2)) as f32;
                let face = (100.0..=196.0).contains(&circle);
                let hands =
                    (x == 16 && (7..=17).contains(&y)) || (y == 16 && (16..=24).contains(&x));
                if face || hands {
                    let index = (y * 32 + x) * 4;
                    rgba[index..index + 4].copy_from_slice(&[200, 200, 200, 255]);
                }
            }
        }
        let icon = Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Time Flows Away")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;
        let (sender, events) = mpsc::channel();
        let (settings_id, pause_id, quit_id) =
            (settings.id().clone(), pause.id().clone(), quit.id().clone());
        let wake = ctx.clone();
        let click_sender = sender.clone();
        let own_id = icon.id().clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(event,TrayIconEvent::DoubleClick {ref id,button:MouseButton::Left,..} if *id==own_id)
            {
                let _ = click_sender.send(Command::Settings);
                wake.request_repaint();
            }
        }));
        let wake = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let command = if event.id == settings_id {
                Command::Settings
            } else if event.id == pause_id {
                Command::TogglePause
            } else if event.id == quit_id {
                Command::Quit
            } else {
                return;
            };
            let _ = sender.send(command);
            wake.request_repaint();
        }));
        Ok(Self {
            _icon: icon,
            pause,
            events,
        })
    }
    pub fn poll(&self) -> Vec<Command> {
        self.events.try_iter().collect()
    }
    pub fn paused(&self, paused: bool) {
        self.pause.set_text(if paused { "Resume" } else { "Pause" });
        let _ = self._icon.set_tooltip(Some(if paused {
            "Time Flows Away — Paused"
        } else {
            "Time Flows Away"
        }));
    }
}
