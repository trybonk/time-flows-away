//! Render through Windows' color-font support, without a visible helper window
//! or an additional GPU surface. Only the font factory is retained between decodes.
use eframe::egui;
use windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Imaging::*,
        },
        System::Com::*,
    },
    core::{Interface, PCWSTR, w},
};

struct ComGuard(bool);
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

pub(super) fn rasterize(emoji: &str, pixels: u32) -> windows::core::Result<egui::ColorImage> {
    // All COM interfaces are local to this thread and dropped before the guard.
    // Respect an apartment already initialized by eframe or the file picker.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if initialized != RPC_E_CHANGED_MODE {
            initialized.ok()?;
        }
        let _com = ComGuard(initialized.is_ok());
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let font_size = (pixels as f32 * 2.0).clamp(128.0, 1536.0);
        let side = (font_size * 1.5).ceil() as u32;
        let bitmap = wic.CreateBitmap(
            side,
            side,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )?;
        let target = d2d.CreateWicBitmapRenderTarget(
            &bitmap,
            &D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            },
        )?;
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        let format = dwrite.CreateTextFormat(
            w!("Segoe UI Emoji"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_size,
            w!("en-us"),
        )?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let brush = target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            None,
        )?;
        let text: Vec<u16> = emoji.encode_utf16().collect();
        target.BeginDraw();
        target.Clear(None);
        target.DrawText(
            &text,
            &format,
            &D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: side as f32,
                bottom: side as f32,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None)?;
        let mut pixels = vec![0; (side * side * 4) as usize];
        bitmap.CopyPixels(std::ptr::null(), side * 4, &mut pixels)?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        Ok(egui::ColorImage::from_rgba_premultiplied(
            [side as usize; 2],
            &pixels,
        ))
    }
}

// DirectWrite keeps system font files mapped instead of copying complete TTC
// and emoji files into both FontDefinitions and ab_glyph in the wallpaper.
pub(super) struct Font {
    factory: IDWriteFactory,
    collection: Option<IDWriteFontCollection>,
    family: Vec<u16>,
    weight: DWRITE_FONT_WEIGHT,
    style: DWRITE_FONT_STYLE,
    stretch: DWRITE_FONT_STRETCH,
}
impl Font {
    pub fn new(path: &str) -> windows::core::Result<Self> {
        unsafe {
            let factory: IDWriteFactory3 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut result = Self {
                factory: factory.cast()?,
                collection: None,
                family: "Microsoft JhengHei".encode_utf16().chain(Some(0)).collect(),
                weight: DWRITE_FONT_WEIGHT_BOLD,
                style: DWRITE_FONT_STYLE_NORMAL,
                stretch: DWRITE_FONT_STRETCH_NORMAL,
            };
            if !path.is_empty() {
                let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
                let face = factory.CreateFontFaceReference2(
                    PCWSTR(path.as_ptr()),
                    None,
                    0,
                    DWRITE_FONT_SIMULATIONS_NONE,
                )?;
                let builder = factory.CreateFontSetBuilder()?;
                builder.AddFontFaceReference2(&face)?;
                let collection: IDWriteFontCollection = factory
                    .CreateFontCollectionFromFontSet(&builder.CreateFontSet()?)?
                    .cast()?;
                let family = collection.GetFontFamily(0)?;
                let names = family.GetFamilyNames()?;
                result.family = vec![0; names.GetStringLength(0)? as usize + 1];
                names.GetString(0, &mut result.family)?;
                let font = family.GetFont(0)?;
                result.weight = font.GetWeight();
                result.style = font.GetStyle();
                result.stretch = font.GetStretch();
                result.collection = Some(collection);
            }
            Ok(result)
        }
    }
    unsafe fn layout(&self, text: &str, size: f32) -> windows::core::Result<IDWriteTextLayout> {
        unsafe {
            let format = self.factory.CreateTextFormat(
                PCWSTR(self.family.as_ptr()),
                self.collection.as_ref(),
                self.weight,
                self.style,
                self.stretch,
                size,
                w!("en-us"),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            self.factory.CreateTextLayout(
                &text.encode_utf16().collect::<Vec<_>>(),
                &format,
                65536.0,
                4096.0,
            )
        }
    }
    pub fn measure(&self, text: &str, size: f32) -> windows::core::Result<egui::Vec2> {
        unsafe {
            let layout = self.layout(text, size)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            Ok(egui::vec2(
                metrics.widthIncludingTrailingWhitespace,
                metrics.height,
            ))
        }
    }
    pub fn image(
        &self,
        text: &str,
        size: f32,
        ppp: f32,
        quality: bool,
        max_side: usize,
    ) -> windows::core::Result<(egui::ColorImage, f32, egui::Vec2)> {
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if initialized != RPC_E_CHANGED_MODE {
                initialized.ok()?;
            }
            let _guard = ComGuard(initialized.is_ok());
            let layout = self.layout(text, size)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            let pad = size * 0.25 + 2.0 / ppp;
            let logical = egui::vec2(
                metrics.widthIncludingTrailingWhitespace + pad * 2.0,
                metrics.height + pad * 2.0,
            );
            let output_scale = ppp
                .min(max_side as f32 / logical.x.max(logical.y))
                .max(f32::EPSILON);
            let output = [
                ((logical.x * output_scale).ceil().max(1.0) as u32).min(max_side as u32),
                ((logical.y * output_scale).ceil().max(1.0) as u32).min(max_side as u32),
            ];
            let samples = if quality {
                3.0_f32
                    .min(8192.0 / output[0].max(output[1]) as f32)
                    .min(
                        (32.0 * 1024.0 * 1024.0 / (output[0] as f32 * output[1] as f32 * 4.0))
                            .sqrt(),
                    )
                    .max(1.0)
            } else {
                1.0
            };
            let width = (output[0] as f32 * samples).ceil() as u32;
            let height = (output[1] as f32 * samples).ceil() as u32;
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let bitmap = wic.CreateBitmap(
                width,
                height,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )?;
            let target = d2d.CreateWicBitmapRenderTarget(
                &bitmap,
                &D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0 * output_scale * samples,
                    dpiY: 96.0 * output_scale * samples,
                    ..Default::default()
                },
            )?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = target.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                None,
            )?;
            target.BeginDraw();
            target.Clear(None);
            target.DrawTextLayout(
                D2D_POINT_2F { x: pad, y: pad },
                &layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
            target.EndDraw(None, None)?;
            let mut rgba = vec![0; (width * height * 4) as usize];
            bitmap.CopyPixels(std::ptr::null(), width * 4, &mut rgba)?;
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let image =
                egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], &rgba);
            Ok((super::resample(image, output[0], output[1]), pad, logical))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_renders_reported_emoji_in_color_on_transparency() {
        for emoji in ["🤫", "🎣", "👍🏽", "👩‍💻"] {
            let image = rasterize(emoji, 128).unwrap();
            let colored = image
                .pixels
                .iter()
                .filter(|p| p.a() > 128 && p.r().abs_diff(p.b()) > 30)
                .count();
            assert!(colored > 100, "{emoji} has no native color-font pixels");
            assert_eq!(image.pixels[0], egui::Color32::TRANSPARENT);
        }
    }
    #[test]
    fn native_text_has_partial_coverage_and_keeps_unicode_and_custom_fonts() {
        let font = Font::new("").unwrap();
        for text in ["12:34:56", "Привет 时光", "jÁg"] {
            let (image, _, _) = font.image(text, 64.0, 1.5, true, 4096).unwrap();
            assert!(
                image
                    .pixels
                    .iter()
                    .filter(|p| p.a() > 0 && p.a() < 255)
                    .count()
                    > 100
            );
            assert!(image.pixels.iter().any(|p| p.a() == 255));
            assert_eq!(image.pixels[0], egui::Color32::TRANSPARENT);
        }
        let path =
            std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts/arial.ttf");
        let custom = Font::new(path.to_str().unwrap()).unwrap();
        assert!(custom.measure("Custom 123", 64.0).unwrap().x > 100.0);
        let (image, _, _) = custom
            .image("Wide ".repeat(40).as_str(), 300.0, 2.0, true, 512)
            .unwrap();
        assert!(image.size.iter().all(|side| *side <= 512));
        assert!(image.pixels.iter().any(|p| p.a() > 0));
        assert!(Font::new("Z:\\missing-font.ttf").is_err());
    }
}
