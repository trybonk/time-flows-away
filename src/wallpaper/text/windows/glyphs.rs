//! Capture actual DirectWrite shaping, including fallback faces and advances.
//! Glyph bitmaps are independent of the string that placed them.
use super::*;
use std::{cell::RefCell, ffi::c_void, rc::Rc};
use windows::Win32::Foundation::{BOOL, E_NOTIMPL};
use windows::core::{ComObject, IUnknown, implement};

#[derive(Clone)]
pub(crate) struct Glyph {
    pub face: IDWriteFontFace5,
    pub id: u16,
    pub em: f32,
    pub origin: egui::Vec2,
}
pub(crate) struct Shaped {
    pub size: egui::Vec2,
    pub glyphs: Vec<Glyph>,
}
#[implement(IDWriteTextRenderer)]
struct Capture {
    glyphs: Rc<RefCell<Vec<Glyph>>>,
}
#[allow(non_snake_case)]
impl IDWritePixelSnapping_Impl for Capture_Impl {
    fn IsPixelSnappingDisabled(&self, _: *const c_void) -> windows::core::Result<BOOL> {
        Ok(BOOL(1))
    }
    fn GetCurrentTransform(
        &self,
        _: *const c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> windows::core::Result<()> {
        // SAFETY: DirectWrite supplies this writable out parameter during Draw.
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m22: 1.0,
                ..Default::default()
            };
        }
        Ok(())
    }
    fn GetPixelsPerDip(&self, _: *const c_void) -> windows::core::Result<f32> {
        Ok(1.0)
    }
}
#[allow(non_snake_case)]
impl IDWriteTextRenderer_Impl for Capture_Impl {
    fn DrawGlyphRun(
        &self,
        _: *const c_void,
        x: f32,
        y: f32,
        _: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        _: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _: Option<&IUnknown>,
    ) -> windows::core::Result<()> {
        // SAFETY: arrays and face are borrowed only for this synchronous callback.
        unsafe {
            let run = &*run;
            if run.isSideways.as_bool() {
                return Err(E_NOTIMPL.into());
            }
            let face: IDWriteFontFace5 = run
                .fontFace
                .as_ref()
                .ok_or_else(|| windows::core::Error::from(E_NOTIMPL))?
                .cast()?;
            let rtl = !run.bidiLevel.is_multiple_of(2);
            let mut pen = x;
            let mut glyphs = self.glyphs.borrow_mut();
            for i in 0..run.glyphCount as usize {
                let advance = *run.glyphAdvances.add(i);
                let offset = if run.glyphOffsets.is_null() {
                    DWRITE_GLYPH_OFFSET::default()
                } else {
                    *run.glyphOffsets.add(i)
                };
                let gx = if rtl {
                    pen - advance - offset.advanceOffset
                } else {
                    pen + offset.advanceOffset
                };
                glyphs.push(Glyph {
                    face: face.clone(),
                    id: *run.glyphIndices.add(i),
                    em: run.fontEmSize,
                    origin: egui::vec2(gx, y - offset.ascenderOffset),
                });
                pen += if rtl { -advance } else { advance };
            }
        }
        Ok(())
    }
    fn DrawUnderline(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_UNDERLINE,
        _: Option<&IUnknown>,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn DrawStrikethrough(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_STRIKETHROUGH,
        _: Option<&IUnknown>,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn DrawInlineObject(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: Option<&IDWriteInlineObject>,
        _: BOOL,
        _: BOOL,
        _: Option<&IUnknown>,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
}
impl Font {
    pub fn shape(&self, text: &str, size: f32) -> windows::core::Result<Shaped> {
        unsafe {
            let layout = self.layout(text, size)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            let glyphs = Rc::new(RefCell::new(Vec::new()));
            let capture: IDWriteTextRenderer = ComObject::new(Capture {
                glyphs: glyphs.clone(),
            })
            .into_interface();
            layout.Draw(None, &capture, 0.0, 0.0)?;
            let glyphs = glyphs.borrow().clone();
            Ok(Shaped {
                size: egui::vec2(metrics.widthIncludingTrailingWhitespace, metrics.height),
                glyphs,
            })
        }
    }
}

// Rasterize one shaped glyph using the same grayscale AA and HQ filter as a
// whole line. Return its rectangle relative to an LTR glyph baseline.
pub(crate) fn image(
    glyph: &Glyph,
    ppp: f32,
    quality: bool,
    max_side: usize,
) -> windows::core::Result<(egui::ColorImage, egui::Rect, f64)> {
    unsafe {
        let face: IDWriteFontFace = glyph.face.cast()?;
        let mut fm = DWRITE_FONT_METRICS::default();
        face.GetMetrics(&mut fm);
        let mut gm = DWRITE_GLYPH_METRICS::default();
        face.GetDesignGlyphMetrics(&glyph.id, 1, &mut gm, false)?;
        let scale = glyph.em / f32::from(fm.designUnitsPerEm);
        let pad = 2.0 / ppp + glyph.em * 0.05;
        let left = gm.leftSideBearing as f32 * scale - pad;
        let top = (gm.topSideBearing - gm.verticalOriginY) as f32 * scale - pad;
        let width =
            (gm.advanceWidth as f32 - gm.leftSideBearing as f32 - gm.rightSideBearing as f32)
                .max(0.0)
                * scale
                + pad * 2.0;
        let height =
            (gm.advanceHeight as f32 - gm.topSideBearing as f32 - gm.bottomSideBearing as f32)
                .max(0.0)
                * scale
                + pad * 2.0;
        let output = [
            (width * ppp).ceil().max(1.0) as u32,
            (height * ppp).ceil().max(1.0) as u32,
        ];
        if output.iter().any(|side| *side as usize > max_side) {
            return Err(E_NOTIMPL.into());
        }
        let samples = if quality { 3.0 } else { 1.0 };
        let (w, h) = (
            (output[0] as f32 * samples) as u32,
            (output[1] as f32 * samples) as u32,
        );
        if u64::from(w) * u64::from(h) * 4 > 32 * 1024 * 1024 {
            return Err(E_NOTIMPL.into());
        }
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if initialized != RPC_E_CHANGED_MODE {
            initialized.ok()?;
        }
        let _guard = ComGuard(initialized.is_ok());
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let bitmap =
            wic.CreateBitmap(w, h, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)?;
        let target = d2d.CreateWicBitmapRenderTarget(
            &bitmap,
            &D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0 * ppp * samples,
                dpiY: 96.0 * ppp * samples,
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
        let advance = 0.0;
        let offset = DWRITE_GLYPH_OFFSET::default();
        let run = DWRITE_GLYPH_RUN {
            fontFace: std::mem::ManuallyDrop::new(Some(face)),
            fontEmSize: glyph.em,
            glyphCount: 1,
            glyphIndices: &glyph.id,
            glyphAdvances: &advance,
            glyphOffsets: &offset,
            ..Default::default()
        };
        target.BeginDraw();
        target.Clear(None);
        target.DrawGlyphRun(
            D2D_POINT_2F { x: -left, y: -top },
            &run,
            &brush,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        let drawn = target.EndDraw(None, None);
        // DWRITE_GLYPH_RUN is a borrowed ABI struct; release our owned clone.
        let mut run = run;
        std::mem::ManuallyDrop::drop(&mut run.fontFace);
        drawn?;
        let mut rgba = vec![0; (w * h * 4) as usize];
        bitmap.CopyPixels(std::ptr::null(), w * 4, &mut rgba)?;
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
        let filter_started = std::time::Instant::now();
        let image = super::super::resample(image, output[0], output[1]);
        let filter_ms = filter_started.elapsed().as_secs_f64() * 1000.0;
        Ok((
            image,
            egui::Rect::from_min_size(
                egui::pos2(left, top),
                egui::vec2(output[0] as f32 / ppp, output[1] as f32 / ppp),
            ),
            filter_ms,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shaping_keeps_metrics_fallback_kerning_ligatures_and_bidi() {
        let font = Font::new("").unwrap();
        for text in ["AV fi 12:34", "Привет 时光", "العربية עברית", "jÁg"] {
            let shaped = font.shape(text, 64.0).unwrap();
            let measured = font.measure(text, 64.0).unwrap();
            assert!((shaped.size.x - measured.x).abs() < 0.01);
            assert!((shaped.size.y - measured.y).abs() < 0.01);
            assert!(!shaped.glyphs.is_empty());
            for glyph in shaped.glyphs {
                let (image, rect, _) = image(&glyph, 1.5, true, 4096).unwrap();
                assert!(rect.is_finite() && rect.width() > 0.0 && rect.height() > 0.0);
                assert!(
                    image
                        .pixels
                        .iter()
                        .all(|p| p.r() <= p.a() && p.g() <= p.a() && p.b() <= p.a())
                );
                assert_eq!(image.pixels[0], egui::Color32::TRANSPARENT);
            }
        }
    }
}
