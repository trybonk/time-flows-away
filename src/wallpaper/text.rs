//! Cached native text and color emoji, with egui typography for development previews.
use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, TextureHandle, Vec2};
#[cfg(windows)]
use std::cell::{Cell, RefCell};
use std::{rc::Rc, sync::Arc};
use unicode_properties::{EmojiStatus, UnicodeEmoji};
use unicode_segmentation::UnicodeSegmentation;

#[cfg(windows)]
mod windows;

const CACHE_LIMIT: usize = 512;
const TEXTURE_BUDGET: usize = 32 * 1024 * 1024;

#[derive(Default)]
pub struct TextRenderer {
    pub high_quality: bool,
    // Cache failures too, so unavailable glyphs never trigger work every frame.
    emoji: Vec<EmojiEntry>,
    layouts: Vec<LayoutEntry>,
    epoch: u64,
    pub metrics: Metrics,
    #[cfg(windows)]
    glyphs: Vec<GlyphEntry>,
    #[cfg(windows)]
    atlas: Vec<AtlasPage>,
    texture_bytes: usize,
    atlas_fill: f32,
    render_key: Option<(f32, bool)>,
    pub(super) scene: Option<super::render::Scene>,
    #[cfg(windows)]
    native_font: Option<windows::Font>,
    #[cfg(not(windows))]
    emoji_font: Option<Vec<u8>>,
    #[cfg(not(windows))]
    font_loaded: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Metrics {
    pub layout_misses: u64,
    pub raster_misses: u64,
    pub layout_ms: f64,
    pub raster_ms: f64,
    pub filter_ms: f64,
    pub upload_ms: f64,
}
#[cfg(windows)]
struct GlyphEntry {
    glyph: windows::glyphs::Glyph,
    ppp: f32,
    quality: bool,
    image: Option<(usize, Rect, Rect)>,
    seen: u64,
}

#[cfg(windows)]
struct AtlasPage {
    texture: TextureHandle,
    side: usize,
    x: usize,
    y: usize,
    row: usize,
}
#[cfg(windows)]
impl AtlasPage {
    fn place(&mut self, image: egui::ColorImage) -> Result<Rect, egui::ColorImage> {
        let [w, h] = image.size;
        let (mut x, mut y, mut row) = (self.x, self.y, self.row);
        if x + w > self.side {
            x = 0;
            y += row;
            row = 0;
        }
        if w > self.side || y + h > self.side {
            return Err(image);
        }
        self.texture
            .set_partial([x, y], image, egui::TextureOptions::LINEAR);
        self.x = x + w;
        self.y = y;
        self.row = row.max(h);
        Ok(Rect::from_min_max(
            Pos2::new(x as f32 / self.side as f32, y as f32 / self.side as f32),
            Pos2::new(
                (x + w) as f32 / self.side as f32,
                (y + h) as f32 / self.side as f32,
            ),
        ))
    }
}

struct EmojiEntry {
    text: String,
    pixels: u32,
    quality: bool,
    texture: Option<TextureHandle>,
    bytes: usize,
    seen: u64,
}
struct LayoutEntry {
    text: String,
    font: FontId,
    ppp: f32,
    quality: bool,
    parts: Rc<Vec<Part>>,
    seen: u64,
}

enum Part {
    Text(Arc<egui::Galley>),
    Emoji(String),
    #[cfg(windows)]
    Native {
        text: String,
        size: Vec2,
        image: RefCell<Option<(TextureHandle, f32, Vec2)>>,
        attempted: Cell<bool>,
        shaped: Option<windows::glyphs::Shaped>,
    },
}
impl Part {
    fn width(&self, size: f32) -> f32 {
        match self {
            Self::Text(galley) => galley.size().x,
            Self::Emoji(_) => size,
            #[cfg(windows)]
            Self::Native { size, .. } => size.x,
        }
    }
}

impl TextRenderer {
    pub fn glyph_count(&self) -> usize {
        #[cfg(windows)]
        {
            self.glyphs.len()
        }
        #[cfg(not(windows))]
        {
            0
        }
    }
    pub fn cache_stats(&self) -> (usize, usize, usize) {
        (self.layouts.len(), self.emoji.len(), self.texture_bytes)
    }
    pub fn reset(&mut self) {
        self.layouts.clear();
        self.emoji.clear();
        self.texture_bytes = 0;
        self.scene = None;
        #[cfg(windows)]
        {
            self.glyphs.clear();
            self.atlas.clear();
        }
    }
    #[cfg(windows)]
    pub fn set_font(&mut self, path: &str) -> Result<(), String> {
        self.reset();
        match windows::Font::new(path) {
            Ok(font) => {
                self.native_font = Some(font);
                Ok(())
            }
            Err(error) => {
                self.native_font = windows::Font::new("").ok();
                Err(error.to_string())
            }
        }
    }
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        let key = (ctx.pixels_per_point(), self.high_quality);
        if self.render_key.is_some_and(|previous| previous != key) {
            self.reset();
        }
        self.render_key = Some(key);
        self.epoch = self.epoch.wrapping_add(1);
        let fill = ctx.fonts(|f| f.font_atlas_fill_ratio());
        // Atlas resets can invalidate cached egui UVs. Growing it is safe.
        if fill < self.atlas_fill {
            self.layouts.clear();
        }
    }
    pub fn finish_frame(&mut self, ctx: &egui::Context) {
        self.layouts.retain(|e| e.seen == self.epoch);
        self.emoji.retain(|e| e.seen == self.epoch);
        self.texture_bytes = self.emoji.iter().map(|e| e.bytes).sum();
        #[cfg(windows)]
        {
            // Pages persist across clock ticks: a digit used once per ten seconds
            // must not be evicted on a frame-count boundary. Font/DPI/quality and
            // size changes clear them; page bytes and entry count stay bounded.
            self.texture_bytes += self
                .atlas
                .iter()
                .map(|p| p.side * p.side * 4)
                .sum::<usize>();
            for layout in &self.layouts {
                for part in layout.parts.iter() {
                    if let Part::Native { image, .. } = part
                        && let Some((texture, ..)) = image.borrow().as_ref()
                    {
                        self.texture_bytes += texture.size()[0] * texture.size()[1] * 4;
                    }
                }
            }
        }
        self.atlas_fill = ctx.fonts(|f| f.font_atlas_fill_ratio());
    }
    fn plain(&self, painter: &Painter, text: String, font: &FontId) -> Part {
        #[cfg(windows)]
        if let Some(native) = &self.native_font {
            let shaped = native.shape(&text, font.size).ok();
            let size = shaped
                .as_ref()
                .map(|s| Ok(s.size))
                .unwrap_or_else(|| native.measure(&text, font.size));
            if let Ok(size) = size {
                return Part::Native {
                    shaped,
                    text,
                    size,
                    image: RefCell::new(None),
                    attempted: Cell::new(false),
                };
            }
        }
        let factor = if self.high_quality { 2.0 } else { 1.0 };
        let mut large_font = font.clone();
        large_font.size *= factor;
        let galley = painter.layout_no_wrap(text, large_font, Color32::WHITE);
        if factor == 1.0 {
            return Part::Text(galley);
        }
        // Transform geometry while keeping the higher-resolution atlas glyphs.
        // egui caches the source layout; no font/image decode happens per frame.
        let mut shape = egui::epaint::TextShape::new(Pos2::ZERO, galley, Color32::WHITE);
        shape.transform(egui::emath::TSTransform::from_scaling(1.0 / factor));
        Part::Text(shape.galley)
    }
    fn texture(&mut self, ctx: &egui::Context, emoji: &str, pixels: u32) -> Option<TextureHandle> {
        if let Some(entry) = self
            .emoji
            .iter_mut()
            .find(|e| e.text == emoji && e.pixels == pixels && e.quality == self.high_quality)
        {
            entry.seen = self.epoch;
            return entry.texture.clone();
        }
        // Bounded allocation, including a heading with many different emoji.
        // Additional glyphs use the normal system font fallback.
        if self.emoji.len() >= CACHE_LIMIT {
            return None;
        }
        let started = std::time::Instant::now();
        self.metrics.raster_misses += 1;
        #[cfg(windows)]
        let image = windows::rasterize(emoji, pixels).ok().and_then(crop);
        #[cfg(not(windows))]
        let image = self.rasterize(emoji).and_then(crop);
        let image = image.map(|image| {
            let aspect = image.size[0] as f32 / image.size[1] as f32;
            let size = if aspect > 1.0 {
                [pixels, (pixels as f32 / aspect).ceil() as u32]
            } else {
                [(pixels as f32 * aspect).ceil() as u32, pixels]
            };
            if self.high_quality {
                resample(image, size[0].max(1), size[1].max(1))
            } else {
                image
            }
        });
        self.metrics.raster_ms += started.elapsed().as_secs_f64() * 1000.0;
        let upload = std::time::Instant::now();
        let bytes = image.as_ref().map_or(0, |i| i.pixels.len() * 4);
        let texture = image
            .filter(|_| self.texture_bytes + bytes <= TEXTURE_BUDGET)
            .map(|image| {
                ctx.load_texture(
                    format!("emoji-{emoji}"),
                    image,
                    egui::TextureOptions::LINEAR,
                )
            });
        let bytes = if texture.is_some() { bytes } else { 0 };
        self.metrics.upload_ms += upload.elapsed().as_secs_f64() * 1000.0;
        self.texture_bytes += bytes;
        self.emoji.push(EmojiEntry {
            text: emoji.to_owned(),
            pixels,
            quality: self.high_quality,
            texture: texture.clone(),
            bytes,
            seen: self.epoch,
        });
        texture
    }

    fn layout(&mut self, painter: &Painter, text: &str, font: &FontId) -> Rc<Vec<Part>> {
        let ppp = painter.ctx().pixels_per_point();
        if let Some(entry) = self.layouts.iter_mut().find(|e| {
            e.text == text && e.font == *font && e.ppp == ppp && e.quality == self.high_quality
        }) {
            entry.seen = self.epoch;
            return entry.parts.clone();
        }
        let started = std::time::Instant::now();
        self.metrics.layout_misses += 1;
        let mut parts = Vec::new();
        let mut plain = String::new();
        for grapheme in text.graphemes(true) {
            if is_color_emoji(grapheme) {
                if !plain.is_empty() {
                    parts.push(self.plain(painter, std::mem::take(&mut plain), font));
                }
                parts.push(Part::Emoji(grapheme.to_owned()));
            } else {
                plain.push_str(grapheme);
            }
        }
        if !plain.is_empty() {
            parts.push(self.plain(painter, plain, font));
        }
        self.metrics.layout_ms += started.elapsed().as_secs_f64() * 1000.0;
        let parts = Rc::new(parts);
        self.layouts.push(LayoutEntry {
            text: text.to_owned(),
            font: font.clone(),
            ppp,
            quality: self.high_quality,
            parts: parts.clone(),
            seen: self.epoch,
        });
        parts
    }

    pub fn width(&mut self, painter: &Painter, text: &str, font: FontId, _: Color32) -> f32 {
        self.layout(painter, text, &font)
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
        let parts = self.layout(painter, text, &font);
        let width: f32 = parts.iter().map(|p| p.width(font.size)).sum();
        let mut x = center.x - width * 0.5;
        for part in parts.iter() {
            let width = part.width(font.size);
            match part {
                Part::Text(galley) => {
                    let y = center.y - galley.size().y * 0.5;
                    painter.galley_with_override_text_color(Pos2::new(x, y), galley.clone(), color);
                }
                Part::Emoji(emoji) => {
                    let pixels = (font.size * painter.ctx().pixels_per_point())
                        .ceil()
                        .clamp(1.0, 1024.0) as u32;
                    let Some(texture) = self.texture(painter.ctx(), emoji, pixels) else {
                        let galley = painter.layout_no_wrap(emoji.clone(), font.clone(), color);
                        painter.galley(
                            Pos2::new(x, center.y - galley.size().y * 0.5),
                            galley,
                            color,
                        );
                        x += width;
                        continue;
                    };
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
                #[cfg(windows)]
                Part::Native {
                    text,
                    size,
                    image,
                    attempted,
                    shaped,
                } => {
                    if let Some(shaped) = shaped
                        && self.draw_glyphs(
                            painter,
                            Pos2::new(x, center.y - size.y * 0.5),
                            shaped,
                            color,
                        )
                    {
                        x += width;
                        continue;
                    }
                    if !attempted.replace(true)
                        && let Some(native) = &self.native_font
                    {
                        let ctx = painter.ctx();
                        let started = std::time::Instant::now();
                        self.metrics.raster_misses += 1;
                        if let Ok((pixels, pad, logical)) = native.image(
                            text,
                            font.size,
                            ctx.pixels_per_point(),
                            self.high_quality,
                            ctx.input(|i| i.max_texture_side),
                        ) {
                            self.metrics.raster_ms += started.elapsed().as_secs_f64() * 1000.0;
                            let upload = std::time::Instant::now();
                            let bytes = pixels.pixels.len() * 4;
                            if self.texture_bytes + bytes <= TEXTURE_BUDGET {
                                let texture = ctx.load_texture(
                                    "wallpaper-text",
                                    pixels,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.texture_bytes += bytes;
                                *image.borrow_mut() = Some((texture, pad, logical));
                                self.metrics.upload_ms += upload.elapsed().as_secs_f64() * 1000.0;
                            }
                        }
                    }
                    if let Some((texture, pad, logical)) = image.borrow().as_ref() {
                        let rect = Rect::from_min_size(
                            Pos2::new(x - pad, center.y - size.y * 0.5 - pad),
                            *logical,
                        );
                        painter.image(
                            texture.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            color,
                        );
                    } else {
                        let galley = painter.layout_no_wrap(text.clone(), font.clone(), color);
                        painter.galley(
                            Pos2::new(x, center.y - galley.size().y * 0.5),
                            galley,
                            color,
                        );
                    }
                }
            }
            x += width;
        }
    }

    #[cfg(windows)]
    fn atlas_image(
        &mut self,
        ctx: &egui::Context,
        mut image: egui::ColorImage,
    ) -> Option<(usize, Rect)> {
        for (index, page) in self.atlas.iter_mut().enumerate() {
            match page.place(image) {
                Ok(uv) => return Some((index, uv)),
                Err(remaining) => image = remaining,
            }
        }
        let max = ctx.input(|i| i.max_texture_side).min(2048);
        let side = 1024_usize
            .max(image.size[0].max(image.size[1]).next_power_of_two())
            .min(max);
        if side < image.size[0].max(image.size[1])
            || self.texture_bytes + side * side * 4 > TEXTURE_BUDGET
        {
            return None;
        }
        let mut page = AtlasPage {
            texture: ctx.load_texture(
                "native-glyph-atlas",
                egui::ColorImage::filled([side, side], Color32::TRANSPARENT),
                egui::TextureOptions::LINEAR,
            ),
            side,
            x: 0,
            y: 0,
            row: 0,
        };
        let uv = page.place(image).ok()?;
        self.texture_bytes += side * side * 4;
        self.atlas.push(page);
        Some((self.atlas.len() - 1, uv))
    }

    #[cfg(windows)]
    fn draw_glyphs(
        &mut self,
        painter: &Painter,
        origin: Pos2,
        shaped: &windows::glyphs::Shaped,
        color: Color32,
    ) -> bool {
        let ctx = painter.ctx();
        let ppp = ctx.pixels_per_point();
        let mut images = Vec::with_capacity(shaped.glyphs.len());
        for glyph in &shaped.glyphs {
            let entry = self.glyphs.iter().position(|e| {
                e.glyph.id == glyph.id
                    && e.glyph.em == glyph.em
                    && e.ppp == ppp
                    && e.quality == self.high_quality
                    && unsafe { e.glyph.face.Equals(&glyph.face).as_bool() }
            });
            let index = if let Some(index) = entry {
                index
            } else {
                if self.glyphs.len() >= CACHE_LIMIT {
                    return false;
                }
                let started = std::time::Instant::now();
                self.metrics.raster_misses += 1;
                let pixels = windows::glyphs::image(
                    glyph,
                    ppp,
                    self.high_quality,
                    ctx.input(|i| i.max_texture_side),
                )
                .ok();
                self.metrics.raster_ms += started.elapsed().as_secs_f64() * 1000.0;
                let image = pixels.and_then(|(pixels, rect, filter_ms)| {
                    self.metrics.filter_ms += filter_ms;
                    self.metrics.raster_ms -= filter_ms;
                    let started = std::time::Instant::now();
                    let result = self
                        .atlas_image(ctx, pixels)
                        .map(|(page, uv)| (page, uv, rect));
                    self.metrics.upload_ms += started.elapsed().as_secs_f64() * 1000.0;
                    result
                });
                self.glyphs.push(GlyphEntry {
                    glyph: glyph.clone(),
                    ppp,
                    quality: self.high_quality,
                    image,
                    seen: self.epoch,
                });
                self.glyphs.len() - 1
            };
            let entry = &mut self.glyphs[index];
            entry.seen = self.epoch;
            let Some((page, uv, rect)) = &entry.image else {
                return false;
            };
            images.push((
                self.atlas[*page].texture.clone(),
                rect.translate(origin.to_vec2() + glyph.origin),
                *uv,
            ));
        }
        for (texture, rect, uv) in images {
            painter.image(texture.id(), rect, uv, color);
        }
        true
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

// Filter premultiplied channels together to avoid dark/colored fringes at
// transparent edges. This runs on cache misses, never on animation frames.
fn resample(image: egui::ColorImage, width: u32, height: u32) -> egui::ColorImage {
    if image.size == [width as usize, height as usize] {
        return image;
    }
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    let source =
        image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, bytes).unwrap();
    let mut output = image::imageops::resize(
        &source,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    for p in output.pixels_mut() {
        for c in 0..3 {
            p[c] = p[c].min(p[3]);
        }
    }
    egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], output.as_raw())
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
    #[cfg(windows)]
    #[test]
    fn changing_clock_reuses_native_glyphs_and_invalidates_for_dpi_quality_and_font() {
        let ctx = egui::Context::default();
        let mut renderer = TextRenderer {
            high_quality: true,
            ..Default::default()
        };
        renderer.set_font("").unwrap();
        let font = FontId::proportional(64.0);
        let draw = |ctx: &egui::Context, renderer: &mut TextRenderer, text: &str| {
            let _ = ctx.run(Default::default(), |ctx| {
                renderer.begin_frame(ctx);
                let painter = ctx.layer_painter(egui::LayerId::background());
                renderer.draw_centered(
                    &painter,
                    Pos2::new(500.0, 200.0),
                    text,
                    font.clone(),
                    Color32::WHITE,
                );
                renderer.finish_frame(ctx);
            });
        };
        draw(&ctx, &mut renderer, "0123456789:.");
        assert!(renderer.glyph_count() >= 12);
        let warm = renderer.metrics.raster_misses;
        for second in 0..60 {
            draw(&ctx, &mut renderer, &format!("23:59:{second:02}"));
        }
        assert_eq!(
            renderer.metrics.raster_misses, warm,
            "clock created new bitmaps after digit warmup"
        );
        ctx.set_pixels_per_point(1.5);
        draw(&ctx, &mut renderer, "23:59:00");
        assert!(renderer.metrics.raster_misses > warm);
        let warm = renderer.metrics.raster_misses;
        renderer.high_quality = false;
        draw(&ctx, &mut renderer, "23:59:00");
        assert!(renderer.metrics.raster_misses > warm);
        let warm = renderer.metrics.raster_misses;
        let path =
            std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts/arial.ttf");
        renderer.set_font(path.to_str().unwrap()).unwrap();
        draw(&ctx, &mut renderer, "23:59:00");
        assert!(renderer.metrics.raster_misses > warm);
        assert!(renderer.texture_bytes <= TEXTURE_BUDGET);
    }
    #[test]
    fn downsampling_retains_antialiasing_and_premultiplied_color() {
        let mut image = egui::ColorImage::filled([12, 12], Color32::TRANSPARENT);
        for y in 0..12 {
            for x in 0..12 {
                if x < y {
                    image.pixels[y * 12 + x] = Color32::RED;
                }
            }
        }
        let image = resample(image, 5, 5);
        assert!(image.pixels.iter().any(|p| p.a() > 0 && p.a() < 255));
        assert!(
            image
                .pixels
                .iter()
                .all(|p| p.r() <= p.a() && p.g() == 0 && p.b() == 0)
        );
    }
    #[test]
    fn layouts_reuse_scaled_geometry_and_rebuild_for_dpi() {
        let ctx = egui::Context::default();
        let mut renderer = TextRenderer {
            high_quality: true,
            ..Default::default()
        };
        let font = FontId::proportional(64.0);
        let mut previous = None;
        for _ in 0..120 {
            let _ = ctx.run(Default::default(), |ctx| {
                renderer.begin_frame(ctx);
                let painter = ctx.layer_painter(egui::LayerId::background());
                let parts = renderer.layout(&painter, "Clock 12:34", &font);
                if let Some(previous) = &previous {
                    assert!(Rc::ptr_eq(previous, &parts));
                }
                previous = Some(parts);
                renderer.finish_frame(ctx);
            });
        }
        ctx.set_pixels_per_point(1.5);
        let _ = ctx.run(Default::default(), |ctx| {
            renderer.begin_frame(ctx);
            let parts = renderer.layout(
                &ctx.layer_painter(egui::LayerId::background()),
                "Clock 12:34",
                &font,
            );
            assert!(!Rc::ptr_eq(previous.as_ref().unwrap(), &parts));
            renderer.finish_frame(ctx);
        });
        assert_eq!(renderer.layouts.len(), 1);
    }
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
