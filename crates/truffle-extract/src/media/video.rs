//! Pictures from videos. macOS: AVFoundation (hardware decoders, every codec the system plays). Linux: the ffmpeg and
//! ffprobe on PATH; without them a video's pictures are skipped and its soundtrack is still read (symphonia).

use std::path::Path;

use super::av::{Frame, Info};

pub fn info(path: &Path) -> Result<Info, String> {
    imp::info(path)
}

pub fn frames(path: &Path, times: &[f64], max_pixel: usize, tolerance: f64) -> Result<Vec<Frame>, String> {
    imp::frames(path, times, max_pixel, tolerance)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::AnyThread;
    use objc2_av_foundation::{AVAssetImageGenerator, AVMediaTypeVideo, AVURLAsset};
    use objc2_core_foundation::CGSize;
    use objc2_core_media::CMTime;
    use objc2_foundation::{NSString, NSURL};

    fn asset(path: &Path) -> Result<objc2::rc::Retained<AVURLAsset>, String> {
        let text = path.to_str().ok_or("path isn't UTF-8")?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        Ok(unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) })
    }

    pub fn info(path: &Path) -> Result<Info, String> {
        objc2::rc::autoreleasepool(|_| {
            let asset = asset(path)?;
            let duration = unsafe { asset.duration().seconds() };
            #[allow(deprecated)]
            let tracks = unsafe { asset.tracksWithMediaType(AVMediaTypeVideo.ok_or("no AVMediaTypeVideo")?) };
            let mut info = Info { duration: if duration.is_finite() { duration } else { 0.0 }, ..Info::default() };
            if let Some(track) = tracks.firstObject() {
                info.video = true;
                let size = unsafe { track.naturalSize() };
                let t = unsafe { track.preferredTransform() };
                // A quarter turn (phones filming upright) swaps the sides.
                let turned = t.a.abs() < 0.5 && t.b.abs() > 0.5;
                let (w, h) = (size.width.abs() as usize, size.height.abs() as usize);
                (info.width, info.height) = if turned { (h, w) } else { (w, h) };
            }
            Ok(info)
        })
    }

    pub fn frames(path: &Path, times: &[f64], max_pixel: usize, tolerance: f64) -> Result<Vec<Frame>, String> {
        objc2::rc::autoreleasepool(|_| {
            let asset = asset(path)?;
            let generator = unsafe { AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset) };
            let tol = unsafe { CMTime::with_seconds(tolerance.max(0.0), 600) };
            unsafe {
                generator.setAppliesPreferredTrackTransform(true);
                generator.setMaximumSize(CGSize { width: max_pixel as f64, height: max_pixel as f64 });
                generator.setRequestedTimeToleranceBefore(tol);
                generator.setRequestedTimeToleranceAfter(CMTime::with_seconds(0.0, 600));
            }
            let mut out: Vec<Frame> = Vec::new();
            for &time in times {
                let mut actual = unsafe { CMTime::with_seconds(time, 600) };
                #[allow(deprecated)]
                let image = unsafe { generator.copyCGImageAtTime_actualTime_error(CMTime::with_seconds(time, 600), &mut actual) };
                let Ok(image) = image else { continue };
                let at = unsafe { actual.seconds() };
                let at = if at.is_finite() { at } else { time };
                if out.last().is_some_and(|f| (f.time - at).abs() < 1e-3) {
                    continue;
                }
                let ptr = objc2::rc::Retained::as_ptr(&image) as *const std::ffi::c_void;
                if let Some(rgb) = super::super::image::apple::draw_on_white(ptr) {
                    out.push(Frame { time: at, image: rgb });
                }
            }
            Ok(out)
        })
    }

    #[allow(dead_code)]
    fn _assert_any_thread(_: &dyn AnyThread) {}
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    use std::process::{Command, Stdio};
    use truffle_semantic::image::RgbImage;

    fn tool(name: &str) -> Option<std::path::PathBuf> {
        std::env::var_os("PATH")?.to_str()?.split(':').map(|d| std::path::Path::new(d).join(name)).find(|p| p.is_file())
    }

    pub fn info(path: &Path) -> Result<Info, String> {
        let probe = tool("ffprobe").ok_or("no ffprobe: install ffmpeg to index videos")?;
        let out = Command::new(probe)
            .args(["-v", "error", "-print_format", "json", "-show_entries", "format=duration:stream=codec_type,width,height:stream_side_data=rotation"])
            .arg(path)
            .stderr(Stdio::null())
            .output()
            .map_err(|e| format!("ffprobe: {e}"))?;
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("ffprobe: {e}"))?;
        let mut info = Info {
            duration: v["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0),
            ..Info::default()
        };
        for s in v["streams"].as_array().into_iter().flatten() {
            match s["codec_type"].as_str() {
                Some("audio") => info.audio = true,
                Some("video") if !info.video => {
                    info.video = true;
                    let (w, h) = (s["width"].as_u64().unwrap_or(0) as usize, s["height"].as_u64().unwrap_or(0) as usize);
                    let rotation = s["side_data_list"].as_array().into_iter().flatten().find_map(|d| d["rotation"].as_i64()).unwrap_or(0);
                    (info.width, info.height) = if rotation.rem_euclid(180) == 90 { (h, w) } else { (w, h) };
                }
                _ => {}
            }
        }
        Ok(info)
    }

    pub fn frames(path: &Path, times: &[f64], max_pixel: usize, _tolerance: f64) -> Result<Vec<Frame>, String> {
        let ffmpeg = tool("ffmpeg").ok_or("no ffmpeg: install it to index videos")?;
        let info = info(path)?;
        if !info.video || info.width == 0 || info.height == 0 {
            return Ok(Vec::new());
        }
        // Even sides: some pixel formats need them.
        let (w, h) = truffle_semantic::image::fit(info.width, info.height, max_pixel);
        let (w, h) = ((w.max(2) / 2) * 2, (h.max(2) / 2) * 2);
        let mut out = Vec::new();
        for &time in times {
            // Seeking before -i jumps to the keyframe, then decodes forward to the exact time; ffmpeg applies the
            // rotation itself.
            let result = Command::new(&ffmpeg)
                .args(["-v", "error", "-nostdin", "-ss", &format!("{time:.3}"), "-i"])
                .arg(path)
                .args(["-frames:v", "1", "-vf", &format!("scale={w}:{h}"), "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
                .stderr(Stdio::null())
                .output()
                .map_err(|e| format!("ffmpeg: {e}"))?;
            if result.stdout.len() == w * h * 3 {
                out.push(Frame { time, image: RgbImage::new(w, h, result.stdout) });
            }
        }
        Ok(out)
    }
}
