//! Cached fonts and backgrounds sized to the physical viewport.
use eframe::egui;
use std::path::PathBuf;
use time_flows_away_native::config::Config;
pub struct Assets {
    image_path: Option<String>,
    font_path: Option<String>,
    font_flags: (bool, bool),
    settings: bool,
    image_size: [u32; 2],
    resize_pending: Option<([u32; 2], std::time::Instant)>,
    pub texture: Option<egui::TextureHandle>,
    pub text: time_flows_away_native::wallpaper::text::TextRenderer,
    pub animation: time_flows_away_native::wallpaper::animation::ProgressAnimation,
    image_error: Option<String>,
    font_error: Option<String>,
}
impl Assets {
    pub fn new() -> Self {
        Self {
            image_path: None,
            font_path: None,
            font_flags: (false, false),
            settings: false,
            image_size: [0; 2],
            resize_pending: None,
            texture: None,
            text: Default::default(),
            animation: Default::default(),
            image_error: None,
            font_error: None,
        }
    }
    pub fn for_settings() -> Self {
        Self {
            settings: true,
            ..Self::new()
        }
    }
    pub fn sync(&mut self, ctx: &egui::Context, c: &Config) {
        self.sync_inner(ctx, c, true);
    }
    pub fn prepare(&mut self, ctx: &egui::Context, c: &Config) {
        // CreationContext has not received the real viewport yet. Load the
        // background in the first update, before eframe shows its first frame.
        self.sync_inner(ctx, c, false);
    }
    fn sync_inner(&mut self, ctx: &egui::Context, c: &Config, images: bool) {
        self.text.high_quality = c.high_quality_text;
        let size = ctx.input(|i| i.viewport_rect().size()) * ctx.pixels_per_point();
        let image_size = [size.x.ceil().max(1.0) as u32, size.y.ceil().max(1.0) as u32];
        let path_changed = self.image_path.as_ref() != Some(&c.background_image);
        let resized = !c.background_image.is_empty() && self.image_size != image_size;
        let mut resize_ready = false;
        if images && resized && !path_changed {
            let (pending, since) = *self
                .resize_pending
                .get_or_insert((image_size, std::time::Instant::now()));
            if pending != image_size {
                self.resize_pending = Some((image_size, std::time::Instant::now()));
            } else {
                resize_ready = since.elapsed() >= std::time::Duration::from_millis(200);
            }
            ctx.request_repaint_after(
                std::time::Duration::from_millis(200)
                    + std::time::Duration::from_secs_f32(ctx.input(|i| i.predicted_dt)),
            );
        }
        if !resized {
            self.resize_pending = None;
        }
        if images && (path_changed || resize_ready) {
            self.resize_pending = None;
            self.image_size = image_size;
            self.image_path = Some(c.background_image.clone());
            self.texture = None;
            self.image_error = None;
            if !c.background_image.is_empty() {
                match load_image(ctx, &c.background_image, image_size) {
                    Ok(texture) => self.texture = Some(texture),
                    Err(e) => self.image_error = Some(e),
                }
            }
        }
        let flags = if self.settings {
            let values = [
                &c.header,
                &c.footer,
                &c.font_path,
                &c.background_image,
                &c.countdown_prefix,
                &c.countdown_suffix,
            ];
            (values.iter().any(|s|s.chars().any(|ch|matches!(ch as u32,0x2e80..=0x9fff|0xac00..=0xd7ff|0xf900..=0xfaff|0x20000..=0x323af))),
                values.iter().any(|s|s.chars().any(|ch|matches!(ch as u32,0x2600..=0x27bf|0x1f000..=0x1ffff))))
        } else {
            (false, false)
        };
        if self.font_path.as_ref() != Some(&c.font_path) || self.font_flags != flags {
            self.font_flags = flags;
            self.font_path = Some(c.font_path.clone());
            self.font_error = None;
            self.text.reset();
            let mut definitions = egui::FontDefinitions::default();
            let mut family = definitions.families[&egui::FontFamily::Proportional].clone();
            #[cfg(windows)]
            {
                let fonts = PathBuf::from(
                    std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()),
                )
                .join("Fonts");
                self.font_error = if !c.font_path.is_empty()
                    && std::fs::metadata(&c.font_path).is_ok_and(|m| m.len() > 50 * 1024 * 1024)
                {
                    let _ = self.text.set_font("");
                    Some("Font exceeds 50 MiB".into())
                } else {
                    self.text.set_font(&c.font_path).err()
                };
                if flags.1
                    && let Ok(bytes) = font_bytes(&fonts.join("seguiemj.ttf"))
                {
                    definitions.font_data.insert(
                        "system-emoji".into(),
                        egui::FontData::from_owned(bytes).into(),
                    );
                    // Input fields use outline glyphs, while wallpaper/preview
                    // use Windows' native color renderer. Prefer the current OS
                    // font to egui's bundled older emoji fallbacks.
                    family.insert(1, "system-emoji".into());
                    definitions
                        .families
                        .get_mut(&egui::FontFamily::Proportional)
                        .unwrap()
                        .insert(1, "system-emoji".into());
                }
            }
            let load_cjk = cfg!(not(windows)) || flags.0;
            if load_cjk
                && let Some(bytes) = system_fonts()
                    .into_iter()
                    .find_map(|path| font_bytes(&path).ok())
            {
                definitions.font_data.insert(
                    "system-cjk".into(),
                    egui::FontData::from_owned(bytes).into(),
                );
                // Preserve the normal settings UI font, with CJK glyph fallback
                // for editable headings and paths. Wallpaper keeps the bold face.
                definitions
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("system-cjk".into());
                family.insert(0, "system-cjk".into());
            }
            if cfg!(not(windows)) && !c.font_path.is_empty() {
                match font_bytes(&PathBuf::from(&c.font_path)) {
                    Ok(bytes) => {
                        definitions
                            .font_data
                            .insert("user-font".into(), egui::FontData::from_owned(bytes).into());
                        family.insert(0, "user-font".into());
                    }
                    Err(e) => self.font_error = Some(e),
                }
            }
            definitions
                .families
                .insert(egui::FontFamily::Name("wallpaper".into()), family);
            ctx.set_fonts(definitions);
        }
    }
    pub fn error(&self) -> Option<String> {
        self.image_error.clone().or_else(|| self.font_error.clone())
    }
}
fn font_bytes(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 50 * 1024 * 1024 {
        return Err("Font exceeds 50 MiB".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    ttf_parser::Face::parse(&bytes, 0).map_err(|e| format!("Invalid font: {e:?}"))?;
    Ok(bytes)
}
fn load_image(
    ctx: &egui::Context,
    path: &str,
    viewport: [u32; 2],
) -> Result<egui::TextureHandle, String> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut image = reader.decode().map_err(|e| e.to_string())?;
    let ratio = viewport[0] as f32 / viewport[1] as f32;
    let source = image.width() as f32 / image.height() as f32;
    // Crop to the visible cover area before filtering. An ultra-wide source
    // should not stay huge merely to cover a normal desktop.
    if source > ratio {
        let width = (image.height() as f32 * ratio).round().max(1.0) as u32;
        image = image.crop_imm((image.width() - width) / 2, 0, width, image.height());
    } else {
        let height = (image.width() as f32 / ratio).round().max(1.0) as u32;
        image = image.crop_imm(0, (image.height() - height) / 2, image.width(), height);
    }
    let max = ctx.input(|i| i.max_texture_side).min(4096) as u32;
    let scale = (viewport[0] as f32 / image.width() as f32)
        .min(viewport[1] as f32 / image.height() as f32)
        .min(max as f32 / image.width().max(image.height()) as f32)
        .min(1.0);
    if scale < 1.0 {
        image = image.resize_exact(
            (image.width() as f32 * scale).round().max(1.0) as u32,
            (image.height() as f32 * scale).round().max(1.0) as u32,
            image::imageops::FilterType::Lanczos3,
        );
    }
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ctx.load_texture(
        "wallpaper-background",
        egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
        egui::TextureOptions::LINEAR,
    ))
}
fn system_fonts() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let fonts =
            PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()))
                .join("Fonts");
        ["msjhbd.ttc", "msjh.ttc", "msyh.ttc"]
            .iter()
            .map(|name| fonts.join(name))
            .collect()
    }
    #[cfg(not(windows))]
    {
        [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn wallpaper_keeps_system_font_files_out_of_egui() {
        let ctx = egui::Context::default();
        let mut assets = Assets::new();
        let _ = ctx.run(Default::default(), |ctx| {
            assets.sync(ctx, &Config::default())
        });
        let _ = ctx.run(Default::default(), |ctx| {
            ctx.fonts(|fonts| {
                assert!(!fonts.definitions().font_data.contains_key("system-cjk"));
                assert!(!fonts.definitions().font_data.contains_key("system-emoji"));
            });
        });
        assert!(assets.error().is_none());
    }
}
