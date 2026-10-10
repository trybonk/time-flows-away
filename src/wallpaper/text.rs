//! Keep egui typography for text; paint native color emoji as cached textures.
use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, TextureHandle, Vec2};
use std::collections::{HashMap, HashSet};
use unicode_properties::{EmojiStatus, UnicodeEmoji};
use unicode_segmentation::UnicodeSegmentation;

#[cfg(windows)]
mod windows;

const CACHE_LIMIT: usize = 512;

#[derive(Default)]
pub struct TextRenderer {
    // Cache failures too, so unavailable glyphs never trigger work every frame.
    emoji: HashMap<String, Option<TextureHandle>>,
    #[cfg(not(windows))]
    emoji_font: Option<Vec<u8>>,
    #[cfg(not(windows))]
    font_loaded: bool,
}

enum Part {
    Text(std::sync::Arc<egui::Galley>),
    Emoji(TextureHandle),
}
impl Part {
    fn width(&self, size: f32) -> f32 {
        match self {
            Self::Text(galley) => galley.size().x,
            Self::Emoji(_) => size,
        }
    }
}

impl TextRenderer {
    pub fn retain_used<'a>(&mut self, text: impl IntoIterator<Item = &'a str>) {
        let used: HashSet<&str> = text.into_iter().flat_map(|s| s.graphemes(true)).collect();
        self.emoji.retain(|emoji, _| used.contains(emoji.as_str()));
    }
    fn texture(&mut self, ctx: &egui::Context, emoji: &str) -> Option<TextureHandle> {
        if let Some(texture) = self.emoji.get(emoji) {
            return texture.clone();
        }
        // Bounded allocation, including a heading with many different emoji.
        // Additional glyphs use the normal system font fallback.
        if self.emoji.len() >= CACHE_LIMIT {
            return None;
        }
        #[cfg(windows)]
        let image = windows::rasterize(emoji).ok().and_then(crop);
        #[cfg(not(windows))]
        let image = self.rasterize(emoji).and_then(crop);
        let texture = image.map(|image| {
            ctx.load_texture(
                format!("emoji-{emoji}"),
                image,
                egui::TextureOptions::LINEAR,
            )
        });
        self.emoji.insert(emoji.to_owned(), texture.clone());
        texture
    }

    fn layout(
        &mut self,
        painter: &Painter,
        text: &str,
        font: &FontId,
        color: Color32,
    ) -> Vec<Part> {
        let mut parts = Vec::new();
        let mut plain = String::new();
        for grapheme in text.graphemes(true) {
            let texture = is_color_emoji(grapheme)
                .then(|| self.texture(painter.ctx(), grapheme))
                .flatten();
            if let Some(texture) = texture {
                if !plain.is_empty() {
                    parts.push(Part::Text(painter.layout_no_wrap(
                        std::mem::take(&mut plain),
                        font.clone(),
                        color,
                    )));
                }
                parts.push(Part::Emoji(texture));
            } else {
                plain.push_str(grapheme);
            }
        }
        if !plain.is_empty() {
            parts.push(Part::Text(painter.layout_no_wrap(
                plain,
                font.clone(),
                color,
            )));
        }
        parts
    }

    pub fn width(&mut self, painter: &Painter, text: &str, font: FontId, color: Color32) -> f32 {
        self.layout(painter, text, &font, color)
            .iter()
            .map(|p| p.width(font.size))
            .sum()
    }

    pub fn draw_centered(
        &mut self,
        painter: &Painter,
        center: Pos2,
        text: &str,
        font: FontId,
        color: Color32,
    ) {
        let parts = self.layout(painter, text, &font, color);
        let width: f32 = parts.iter().map(|p| p.width(font.size)).sum();
        let mut x = center.x - width * 0.5;
        for part in parts {
            let width = part.width(font.size);
            match part {
                Part::Text(galley) => {
                    let y = center.y - galley.size().y * 0.5;
                    painter.galley(Pos2::new(x, y), galley, color);
                }
                Part::Emoji(texture) => {
                    let aspect = texture.aspect_ratio();
                    let size = if aspect > 1.0 {
                        Vec2::new(font.size, font.size / aspect)
                    } else {
                        Vec2::new(font.size * aspect, font.size)
                    };
                    painter.image(
                        texture.id(),
                        Rect::from_center_size(Pos2::new(x + width * 0.5, center.y), size),
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            }
            x += width;
        }
    }

    #[cfg(not(windows))]
    fn rasterize(&mut self, emoji: &str) -> Option<egui::ColorImage> {
        // Linux is a development preview. Use the installed Noto bitmap font
        // for single-codepoint emoji; Windows shapes full sequences natively.
        if !self.font_loaded {
            self.font_loaded = true;
            self.emoji_font =
                std::fs::read("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf").ok();
        }
        let mut chars = emoji.chars().filter(|c| *c != '\u{fe0f}');
        let ch = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        let face = ttf_parser::Face::parse(self.emoji_font.as_ref()?, 0).ok()?;
        let glyph = face.glyph_raster_image(face.glyph_index(ch)?, 256)?;
        if glyph.format != ttf_parser::RasterImageFormat::PNG {
            return None;
        }
        let rgba = image::load_from_memory_with_format(glyph.data, image::ImageFormat::Png)
            .ok()?
            .to_rgba8();
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            &rgba,
        ))
    }
}

fn is_color_emoji(grapheme: &str) -> bool {
    if grapheme.contains('\u{fe0e}') {
        return false;
    }
    let requested = grapheme.contains('\u{fe0f}') || grapheme.contains('\u{20e3}');
    grapheme.chars().any(|c| {
        (requested && c.is_emoji_char())
            || matches!(
                c.emoji_status(),
                EmojiStatus::EmojiPresentation
                    | EmojiStatus::EmojiPresentationAndModifierBase
                    | EmojiStatus::EmojiPresentationAndEmojiComponent
                    | EmojiStatus::EmojiPresentationAndModifierAndEmojiComponent
            )
    })
}

fn crop(image: egui::ColorImage) -> Option<egui::ColorImage> {
    let [width, height] = image.size;
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, pixel) in image.pixels.iter().enumerate() {
        if pixel.a() > 0 {
            let x = index % width;
            let y = index / width;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return None;
    }
    let mut result = egui::ColorImage::new(
        [right - left, bottom - top],
        vec![Color32::TRANSPARENT; (right - left) * (bottom - top)],
    );
    for y in top..bottom {
        result.pixels[(y - top) * (right - left)..(y - top + 1) * (right - left)]
            .copy_from_slice(&image.pixels[y * width + left..y * width + right]);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emoji_sequences_keep_their_components_and_text_presentation() {
        let input = "Time 12:34 🤫🎣👍🏽👩‍💻1️⃣☀️☀︎";
        let emoji: Vec<_> = input
            .graphemes(true)
            .filter(|g| is_color_emoji(g))
            .collect();
        assert_eq!(emoji, ["🤫", "🎣", "👍🏽", "👩‍💻", "1️⃣", "☀️"]);
        assert!(!is_color_emoji("©"));
        assert!(!is_color_emoji("时"));
    }
    #[test]
    fn transparent_padding_is_removed_without_losing_edge_pixels() {
        let mut image = egui::ColorImage::new([4, 3], vec![Color32::TRANSPARENT; 12]);
        image.pixels[5] = Color32::RED;
        image.pixels[10] = Color32::BLUE;
        let result = crop(image).unwrap();
        assert_eq!(result.size, [2, 2]);
        assert_eq!(
            result.pixels,
            [
                Color32::RED,
                Color32::TRANSPARENT,
                Color32::TRANSPARENT,
                Color32::BLUE
            ]
        );
    }
}
