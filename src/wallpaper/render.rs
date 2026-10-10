use crate::config::{Alignment, Config};
use chrono::NaiveDateTime;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
pub fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
pub fn draw(
    ctx: &egui::Context,
    now: NaiveDateTime,
    config: &Config,
    image: Option<&egui::TextureHandle>,
    text_renderer: &mut super::text::TextRenderer,
    progress: [f32; 5],
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            draw_in(
                ui,
                ui.max_rect(),
                now,
                config,
                image,
                text_renderer,
                progress,
            );
        });
}
pub fn draw_in(
    ui: &egui::Ui,
    bounds: Rect,
    now: NaiveDateTime,
    c: &Config,
    image: Option<&egui::TextureHandle>,
    text_renderer: &mut super::text::TextRenderer,
    progress: [f32; 5],
) {
    let painter = ui.painter().with_clip_rect(bounds);
    painter.rect_filled(bounds, 0.0, color(c.background));
    if let Some(image) = image {
        let ratio = bounds.aspect_ratio() / image.aspect_ratio();
        let uv = if ratio > 1.0 {
            Rect::from_min_max(
                Pos2::new(0.0, (1.0 - 1.0 / ratio) * 0.5),
                Pos2::new(1.0, (1.0 + 1.0 / ratio) * 0.5),
            )
        } else {
            Rect::from_min_max(
                Pos2::new((1.0 - ratio) * 0.5, 0.0),
                Pos2::new((1.0 + ratio) * 0.5, 1.0),
            )
        };
        painter.image(image.id(), bounds, uv, Color32::WHITE);
    }
    let mut date = now.format(&c.date_format).to_string();
    if c.show_weekday {
        date.push_str(&now.format(" %A").to_string());
    }
    let time = now.format(&c.time_format).to_string();
    enum Row {
        Text(String, f32),
        Bars(usize, usize),
        Gap(f32),
    }
    let mut rows = Vec::new();
    if !c.header.is_empty() {
        rows.extend([Row::Text(c.header.clone(), c.header_size), Row::Gap(30.0)]);
    }
    rows.extend([
        Row::Text(date, c.date_size),
        Row::Gap(24.0),
        Row::Bars(0, 2),
        Row::Gap(16.0),
        Row::Text(time, c.time_size),
        Row::Gap(22.0),
        Row::Bars(2, 3),
    ]);
    if !c.footer.is_empty() {
        rows.extend([Row::Gap(18.0), Row::Text(c.footer.clone(), c.footer_size)]);
    }
    if let Some(text) = c.timer_text(now) {
        rows.extend([Row::Gap(18.0), Row::Text(text, c.countdown_size)]);
    }
    text_renderer.retain_used(rows.iter().filter_map(|row| match row {
        Row::Text(text, _) => Some(text.as_str()),
        _ => None,
    }));
    let font = |size| FontId::new(size, FontFamily::Name("wallpaper".into()));
    let height = |row: &Row| match row {
        Row::Text(_, size) => *size,
        Row::Bars(_, n) => c.bar_height * (*n as f32) + c.bar_gap * ((*n - 1) as f32),
        Row::Gap(gap) => *gap,
    };
    let total: f32 = rows.iter().map(height).sum();
    let width = rows
        .iter()
        .map(|row| match row {
            Row::Text(text, size) => {
                text_renderer.width(&painter, text, font(*size), color(c.text_color))
            }
            Row::Bars(..) => c.bar_width,
            Row::Gap(_) => 0.0,
        })
        .fold(0.0_f32, f32::max);
    let scale = (bounds.width() / (width + 60.0))
        .min(bounds.height() / (total + 90.0))
        .min(1.0);
    let size = Vec2::new(width, total) * scale;
    let align = |start: f32, end: f32, size: f32, anchor: Alignment| match anchor {
        Alignment::Start => start + 30.0 * scale,
        Alignment::Center => (start + end - size) * 0.5,
        Alignment::End => end - size - 30.0 * scale,
    };
    let origin = Pos2::new(
        align(bounds.left(), bounds.right(), size.x, c.horizontal),
        align(bounds.top(), bounds.bottom(), size.y, c.vertical),
    ) + Vec2::new(c.offset_x, c.offset_y) * scale;
    let origin = origin + Vec2::new(0.0, 2.5 * scale);
    let panel = Rect::from_min_size(origin, size).expand2(Vec2::new(30.0, 10.0) * scale);
    painter.rect_filled(
        panel,
        30.0 * scale,
        Color32::from_rgba_unmultiplied(
            c.panel_color[0],
            c.panel_color[1],
            c.panel_color[2],
            (c.panel_opacity * 255.0).round() as u8,
        ),
    );
    let center_x = origin.x + size.x * 0.5;
    let mut y = origin.y;
    for row in &rows {
        match row {
            Row::Text(text, size) => {
                text_renderer.draw_centered(
                    &painter,
                    Pos2::new(center_x, y + size * scale * 0.5),
                    text,
                    font(size * scale),
                    color(c.text_color),
                );
            }
            Row::Bars(start, n) => {
                for i in 0..*n {
                    let index = start + i;
                    let rect = Rect::from_min_size(
                        Pos2::new(
                            center_x - c.bar_width * scale * 0.5,
                            y + i as f32 * (c.bar_height + c.bar_gap) * scale,
                        ),
                        Vec2::new(c.bar_width, c.bar_height) * scale,
                    );
                    let color = color(c.bar_colors[index]);
                    painter.rect_filled(
                        Rect::from_min_size(
                            rect.min,
                            Vec2::new(rect.width() * progress[index], rect.height()),
                        ),
                        0.0,
                        color,
                    );
                    painter.rect_stroke(rect, 0.0, Stroke::new(scale, color), StrokeKind::Inside);
                }
            }
            Row::Gap(_) => {}
        }
        y += height(row) * scale;
    }
}
