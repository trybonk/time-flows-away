//! Assets are decoded only when their configured paths change.
use eframe::egui;
use std::path::PathBuf;
use time_flows_away_native::config::Config;
pub struct Assets {
    image_path: Option<String>,
    font_path: Option<String>,
    pub texture: Option<egui::TextureHandle>,
    pub text: time_flows_away_native::wallpaper::text::TextRenderer,
    image_error: Option<String>,
    font_error: Option<String>,
}
impl Assets {
    pub fn new() -> Self {
        Self {
            image_path: None,
            font_path: None,
            texture: None,
            text: Default::default(),
            image_error: None,
            font_error: None,
        }
    }
    pub fn sync(&mut self, ctx: &egui::Context, c: &Config) {
        if self.image_path.as_ref() != Some(&c.background_image) {
            self.image_path = Some(c.background_image.clone());
            self.texture = None;
            self.image_error = None;
            if !c.background_image.is_empty() {
                match load_image(ctx, &c.background_image) {
                    Ok(texture) => self.texture = Some(texture),
                    Err(e) => self.image_error = Some(e),
                }
            }
        }
        if self.font_path.as_ref() != Some(&c.font_path) {
            self.font_path = Some(c.font_path.clone());
            self.font_error = None;
            let mut definitions = egui::FontDefinitions::default();
            let mut family = definitions.families[&egui::FontFamily::Proportional].clone();
            #[cfg(windows)]
            {
                let fonts = PathBuf::from(
                    std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()),
                )
                .join("Fonts");
                if let Ok(bytes) = font_bytes(&fonts.join("seguiemj.ttf")) {
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
            if let Some(bytes) = system_fonts()
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
            if !c.font_path.is_empty() {
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
fn load_image(ctx: &egui::Context, path: &str) -> Result<egui::TextureHandle, String> {
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
    let max = ctx.input(|i| i.max_texture_side) as u32;
    if image.width() > max || image.height() > max {
        image = image.thumbnail(max, max);
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
