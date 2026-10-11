//! One sleeping worker, one pending request and one result; newest request wins.
use eframe::egui;
use image::{DynamicImage, ImageDecoder, metadata::Orientation};
use std::{
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};
#[derive(Clone)]
pub(super) struct Job {
    pub path: String,
    pub viewport: [u32; 2],
    pub max: u32,
}
struct Queue {
    job: Option<(u64, Job)>,
    result: Option<Result<egui::ColorImage, String>>,
    latest: u64,
    stopped: bool,
}
pub(super) struct Worker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(ctx: egui::Context) -> Result<Self, String> {
        let queue = Arc::new((
            Mutex::new(Queue {
                job: None,
                result: None,
                latest: 0,
                stopped: false,
            }),
            Condvar::new(),
        ));
        let shared = queue.clone();
        let thread = std::thread::Builder::new()
            .name("background-loader".into())
            .spawn(move || loop {
                let (id,job) = {
                    let (mutex,wake) = &*shared;
                    let mut state = mutex.lock().unwrap();
                    while state.job.is_none() && !state.stopped {
                        state = wake.wait(state).unwrap();
                    }
                    if state.stopped {return;}
                    state.job.take().unwrap()
                };
                let started = Instant::now();
                let image = decode(Path::new(&job.path),job.viewport,job.max);
                let success = image.is_ok();
                let accepted = {
                    let mut state = shared.0.lock().unwrap();
                    if state.stopped {return;}
                    if state.latest == id {state.result = Some(image);true} else {false}
                };
                // Never hold the request mutex while logging or notifying egui.
                if accepted {
                    crate::diagnostics::event(&format!(
                        "Background loaded: generation={id} elapsed_ms={:.2} target={}x{} success={success}",
                        started.elapsed().as_secs_f64()*1000.0,job.viewport[0],job.viewport[1]));
                    ctx.request_repaint();
                }
            }).map_err(|e|e.to_string())?;
        Ok(Self {
            queue,
            thread: Some(thread),
        })
    }
    #[cfg(test)]
    fn stop_and_join(&mut self) {
        {
            let (mutex, wake) = &*self.queue;
            let mut state = mutex.lock().unwrap();
            state.stopped = true;
            state.job = None;
            state.result = None;
            wake.notify_one();
        }
        self.thread.take().unwrap().join().unwrap();
    }
    pub fn submit(&self, job: Option<Job>) {
        let (mutex, wake) = &*self.queue;
        let mut state = mutex.lock().unwrap();
        state.latest = state.latest.wrapping_add(1);
        state.job = job.map(|job| (state.latest, job));
        state.result = None;
        wake.notify_one();
    }
    pub fn take(&self) -> Option<Result<egui::ColorImage, String>> {
        self.queue.0.lock().unwrap().result.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Don't block closing a window on an in-flight decode. It discards its result.
        let (mutex, wake) = &*self.queue;
        let mut state = mutex.lock().unwrap();
        state.stopped = true;
        state.job = None;
        state.result = None;
        wake.notify_one();
        self.thread.take(); // dropping JoinHandle deliberately detaches
    }
}
fn dimensions(w: u32, h: u32) -> Result<(), String> {
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        Err("Background dimensions must be between 1 and 8192 pixels".into())
    } else {
        Ok(())
    }
}
fn cover(w: u32, h: u32, viewport: [u32; 2], max: u32) -> (u32, u32, u32, u32, [u32; 2]) {
    let ratio = f64::from(viewport[0].max(1)) / f64::from(viewport[1].max(1));
    let (cw, ch) = if f64::from(w) / f64::from(h) > ratio {
        (
            (f64::from(h) * ratio).round().clamp(1.0, f64::from(w)) as u32,
            h,
        )
    } else {
        (
            w,
            (f64::from(w) / ratio).round().clamp(1.0, f64::from(h)) as u32,
        )
    };
    let scale = (f64::from(viewport[0].max(1)) / f64::from(cw))
        .min(f64::from(viewport[1].max(1)) / f64::from(ch))
        .min(f64::from(max.max(1)) / f64::from(cw.max(ch)))
        .min(1.0);
    (
        (w - cw) / 2,
        (h - ch) / 2,
        cw,
        ch,
        [
            (f64::from(cw) * scale).round().max(1.0) as u32,
            (f64::from(ch) * scale).round().max(1.0) as u32,
        ],
    )
}
fn finish(
    mut image: DynamicImage,
    orientation: Orientation,
    viewport: [u32; 2],
    max: u32,
) -> egui::ColorImage {
    image.apply_orientation(orientation);
    let (x, y, w, h, target) = cover(image.width(), image.height(), viewport, max);
    if w != image.width() || h != image.height() {
        image = image.crop_imm(x, y, w, h);
    }
    if target != [w, h] {
        image = image.resize_exact(target[0], target[1], image::imageops::FilterType::Lanczos3);
    }
    let rgba = image.into_rgba8();
    egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba)
}
pub(super) fn decode(
    path: &Path,
    viewport: [u32; 2],
    max: u32,
) -> Result<egui::ColorImage, String> {
    let format = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .format();
    match format {
        Some(image::ImageFormat::Jpeg) => jpeg(path, viewport, max),
        Some(image::ImageFormat::Png) => png_rows(path, viewport, max),
        _ => Err("Background must be PNG or JPEG".into()),
    }
}
fn jpeg(path: &Path, viewport: [u32; 2], max: u32) -> Result<egui::ColorImage, String> {
    let mut decoder =
        jpeg_decoder::Decoder::new(BufReader::new(File::open(path).map_err(|e| e.to_string())?));
    decoder.set_max_decoding_buffer_size(256 * 1024 * 1024);
    decoder.read_info().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("Missing JPEG header")?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    dimensions(w, h)?;
    let orientation = decoder
        .exif_data()
        .and_then(|bytes| {
            Orientation::from_exif_chunk(bytes.strip_prefix(b"Exif\0\0").unwrap_or(bytes))
        })
        .unwrap_or(Orientation::NoTransforms);
    let rotated = matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    let (ow, oh) = if rotated { (h, w) } else { (w, h) };
    let (_, _, cw, ch, target) = cover(ow, oh, viewport, max);
    let scale = (f64::from(target[0]) / f64::from(cw)).max(f64::from(target[1]) / f64::from(ch));
    decoder
        .scale(
            (f64::from(w) * scale).ceil().max(1.0) as u16,
            (f64::from(h) * scale).ceil().max(1.0) as u16,
        )
        .map_err(|e| e.to_string())?;
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("Missing JPEG pixels")?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let image = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(w, h, pixels).ok_or("Invalid JPEG buffer")?,
        ),
        jpeg_decoder::PixelFormat::L8 => DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(w, h, pixels).ok_or("Invalid JPEG buffer")?,
        ),
        _ => {
            drop(pixels);
            drop(decoder);
            return generic(path, viewport, max);
        }
    };
    drop(decoder);
    Ok(finish(image, orientation, viewport, max))
}
fn generic(path: &Path, viewport: [u32; 2], max: u32) -> Result<egui::ColorImage, String> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    Ok(finish(
        DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?,
        orientation,
        viewport,
        max,
    ))
}
fn png_rows(path: &Path, viewport: [u32; 2], max: u32) -> Result<egui::ColorImage, String> {
    let mut decoder = png::Decoder::new_with_limits(
        BufReader::new(File::open(path).map_err(|e| e.to_string())?),
        png::Limits {
            bytes: 128 * 1024 * 1024,
        },
    );
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    dimensions(info.width, info.height)?;
    let orientation = info
        .exif_metadata
        .as_ref()
        .and_then(|bytes| Orientation::from_exif_chunk(bytes))
        .unwrap_or(Orientation::NoTransforms);
    if info.interlaced || orientation != Orientation::NoTransforms {
        drop(reader);
        return generic(path, viewport, max);
    }
    let (sw, sh) = (info.width, info.height);
    let (x, y, w, h, target) = cover(sw, sh, viewport, max);
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return Err("Unsupported PNG depth".into());
    }
    let channels = color.samples();
    let mut intermediate = image::RgbaImage::new(target[0], h);
    let mut index = 0;
    while let Some(row) = reader.next_row().map_err(|e| e.to_string())? {
        if index >= y && index < y + h {
            let mut rgba = Vec::with_capacity(w as usize * 4);
            for pixel in row.data()[x as usize * channels..(x + w) as usize * channels]
                .chunks_exact(channels)
            {
                let (r, g, b, a) = match color {
                    png::ColorType::Grayscale => (pixel[0], pixel[0], pixel[0], 255),
                    png::ColorType::GrayscaleAlpha => (pixel[0], pixel[0], pixel[0], pixel[1]),
                    png::ColorType::Rgb => (pixel[0], pixel[1], pixel[2], 255),
                    png::ColorType::Rgba => (pixel[0], pixel[1], pixel[2], pixel[3]),
                    _ => return Err("Unexpanded PNG palette".into()),
                };
                rgba.extend(egui::Color32::from_rgba_unmultiplied(r, g, b, a).to_array());
            }
            let row = image::RgbaImage::from_raw(w, 1, rgba).ok_or("Invalid PNG row")?;
            let scaled = if w == target[0] {
                row
            } else {
                image::imageops::resize(&row, target[0], 1, image::imageops::FilterType::Lanczos3)
            };
            let offset = (index - y) as usize * target[0] as usize * 4;
            intermediate.as_mut()[offset..offset + scaled.len()].copy_from_slice(scaled.as_raw());
        }
        index += 1;
    }
    reader.finish().map_err(|e| e.to_string())?;
    if index != sh {
        return Err("Truncated PNG rows".into());
    }
    let mut output = if h == target[1] {
        intermediate
    } else {
        image::imageops::resize(
            &intermediate,
            target[0],
            target[1],
            image::imageops::FilterType::Lanczos3,
        )
    };
    for p in output.pixels_mut() {
        for c in 0..3 {
            p[c] = p[c].min(p[3]);
        }
    }
    Ok(egui::ColorImage::from_rgba_premultiplied(
        [target[0] as usize, target[1] as usize],
        output.as_raw(),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cover_caps_without_upscaling() {
        assert_eq!(
            cover(8000, 4000, [1920, 1080], 4096),
            (444, 0, 7111, 4000, [1920, 1080])
        );
        assert_eq!(
            cover(100, 100, [1920, 1080], 4096),
            (0, 22, 100, 56, [100, 56])
        );
        assert_eq!(cover(8000, 4000, [8000, 4000], 4096).4, [4096, 2048]);
    }
    #[test]
    fn adam7_fallback_keeps_alpha_and_jpeg_exif_rotates_pixels() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/background-adam7.png");
        let image = decode(&path, [5, 5], 4096).unwrap();
        assert_eq!(image.size, [5, 5]);
        assert_eq!(image.pixels[0], egui::Color32::RED);
        assert_eq!(image.pixels[4].a(), 128);
        assert!(image.pixels[4].b() > 100);
        let path = std::env::temp_dir().join(format!("tfa-orient-{}.jpg", std::process::id()));
        let source = image::RgbImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                image::Rgb([250, 0, 0])
            } else {
                image::Rgb([0, 0, 250])
            }
        });
        source.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut tagged = vec![0xff, 0xd8, 0xff, 0xe1];
        tagged.extend(((exif.len() + 2) as u16).to_be_bytes());
        tagged.extend(exif);
        tagged.extend(&bytes[2..]);
        std::fs::write(&path, tagged).unwrap();
        let image = decode(&path, [32, 64], 4096).unwrap();
        assert_eq!(image.size, [32, 64]);
        assert!(image.pixels[16].r() > 200);
        assert!(image.pixels[63 * 32 + 16].b() > 200);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn worker_latest_request_and_clear_discard_previous_results() {
        let dir = std::env::temp_dir().join(format!("tfa-worker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let red = dir.join("red.png");
        let blue = dir.join("blue.png");
        image::RgbImage::from_pixel(64, 64, image::Rgb([255, 0, 0]))
            .save(&red)
            .unwrap();
        image::RgbImage::from_pixel(64, 64, image::Rgb([0, 0, 255]))
            .save(&blue)
            .unwrap();
        let mut worker = Worker::new(egui::Context::default()).unwrap();
        for _ in 0..10 {
            worker.submit(Some(Job {
                path: red.to_str().unwrap().into(),
                viewport: [32, 32],
                max: 4096,
            }));
        }
        worker.submit(Some(Job {
            path: blue.to_str().unwrap().into(),
            viewport: [32, 32],
            max: 4096,
        }));
        let started = Instant::now();
        let result = loop {
            if let Some(result) = worker.take() {
                break result;
            }
            assert!(started.elapsed().as_secs() < 5);
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(
            result
                .unwrap()
                .pixels
                .iter()
                .all(|p| p.b() == 255 && p.r() == 0)
        );
        worker.submit(Some(Job {
            path: red.to_str().unwrap().into(),
            viewport: [32, 32],
            max: 4096,
        }));
        worker.submit(None);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(worker.take().is_none());
        worker.stop_and_join();
        drop(worker);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn jpeg_png_downscale_keep_color_and_alpha() {
        let dir = std::env::temp_dir().join(format!(
            "tfa-image-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image.jpg");
        image::RgbImage::from_pixel(1024, 512, image::Rgb([250, 30, 10]))
            .save(&path)
            .unwrap();
        let image = decode(&path, [128, 64], 4096).unwrap();
        assert_eq!(image.size, [128, 64]);
        assert!(image.pixels.iter().all(|p| p.r() > 240 && p.g() < 40));
        let path = dir.join("image.png");
        image::RgbaImage::from_fn(128, 64, |x, _| {
            if x < 64 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 255, 0, 0])
            }
        })
        .save(&path)
        .unwrap();
        let image = decode(&path, [32, 16], 4096).unwrap();
        assert_eq!(image.size, [32, 16]);
        assert!(image.pixels.iter().any(|p| p.a() > 0 && p.a() < 255));
        assert!(image.pixels.iter().all(|p| p.g() == 0 && p.r() <= p.a()));
        assert_eq!(decode(&path, [128, 64], 4096).unwrap().size, [128, 64]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
