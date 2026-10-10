//! Render through Windows' color-font support, without a visible helper window
//! or an additional GPU surface. Factories and brushes live only during decoding.
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
    core::w,
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

pub(super) fn rasterize(emoji: &str) -> windows::core::Result<egui::ColorImage> {
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
        const SIDE: u32 = 384;
        let bitmap = wic.CreateBitmap(
            SIDE,
            SIDE,
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
        let format = dwrite.CreateTextFormat(
            w!("Segoe UI Emoji"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            256.0,
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
                right: SIDE as f32,
                bottom: SIDE as f32,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None)?;
        let mut pixels = vec![0; (SIDE * SIDE * 4) as usize];
        bitmap.CopyPixels(std::ptr::null(), SIDE * 4, &mut pixels)?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        Ok(egui::ColorImage::from_rgba_premultiplied(
            [SIDE as usize; 2],
            &pixels,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_renders_reported_emoji_in_color_on_transparency() {
        for emoji in ["🤫", "🎣", "👍🏽", "👩‍💻"] {
            let image = rasterize(emoji).unwrap();
            let colored = image
                .pixels
                .iter()
                .filter(|p| p.a() > 128 && p.r().abs_diff(p.b()) > 30)
                .count();
            assert!(colored > 100, "{emoji} has no native color-font pixels");
            assert_eq!(image.pixels[0], egui::Color32::TRANSPARENT);
        }
    }
}
