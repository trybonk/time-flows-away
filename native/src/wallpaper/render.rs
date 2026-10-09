use super::clock::Clock;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
// Defaults from project.json, rather than CSS before the host applies settings.
const COLORS: [Color32; 5] = [
    Color32::from_rgb(188, 189, 190),
    Color32::from_rgb(199, 94, 83),
    Color32::from_rgb(255, 253, 93),
    Color32::from_rgb(71, 144, 229),
    Color32::from_rgb(132, 216, 75),
];
pub fn draw(ctx: &egui::Context, clock: &Clock) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(Color32::from_rgb(22, 22, 22)))
        .show(ctx, |ui| {
            let bounds = ui.max_rect();
            let scale = (bounds.width() / 850.0)
                .min(bounds.height() / 660.0)
                .min(1.0);
            let center = bounds.center();
            let painter = ui.painter();
            for (text, y, size) in [
                ("时光飞逝", -230.0, 100.0),
                (clock.date.as_str(), -100.0, 100.0),
                (clock.time.as_str(), 65.0, 100.0),
                ("Time flows away", 240.0, 90.0),
            ] {
                painter.text(
                    center + Vec2::new(0.0, y * scale),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(size * scale),
                    Color32::from_rgb(200, 200, 200),
                );
            }
            for (i, (value, color)) in clock.progress.iter().zip(COLORS).enumerate() {
                let y = [-26.0, -11.0, 137.0, 152.0, 167.0][i];
                let rect = Rect::from_min_size(
                    Pos2::new(center.x - 265.0 * scale, center.y + y * scale),
                    Vec2::new(530.0 * scale, 10.0 * scale),
                );
                painter.rect_filled(
                    Rect::from_min_size(rect.min, Vec2::new(rect.width() * value, rect.height())),
                    0.0,
                    color,
                );
                painter.rect_stroke(rect, 0.0, Stroke::new(scale, color), StrokeKind::Inside);
            }
        });
}
