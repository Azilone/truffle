//! Audio and video: the soundtrack as 16 kHz mono float (symphonia, pure Rust, on every OS), and pictures at chosen
//! times (AVFoundation on macOS, ffmpeg on Linux when it's installed). The resampler, the downmix and the frame hash
//! come from trovara.

use std::path::Path;

use truffle_semantic::image::RgbImage;

pub const SAMPLE_RATE: u32 = 16_000;

/// What a media file holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub duration: f64,
    pub audio: bool,
    pub video: bool,
    /// The video's size as stored.
    pub width: usize,
    pub height: usize,
}

/// One picture from a video.
pub struct Frame {
    /// Seconds into the video where this picture is shown.
    pub time: f64,
    pub image: RgbImage,
}

/// What `path` holds: its soundtrack from symphonia, its pictures from the platform's video reader.
pub fn info(path: &Path) -> Result<Info, String> {
    let sound = super::audio::probe(path);
    let video = super::video::info(path);
    match (sound, video) {
        (Ok(a), Ok(v)) => Ok(Info { duration: v.duration.max(a.duration), audio: a.audio || v.audio, ..v }),
        (Ok(a), Err(_)) => Ok(a),
        (Err(_), Ok(v)) => Ok(v),
        (Err(e), Err(_)) => Err(e),
    }
}

/// Decodes the soundtrack to 16 kHz mono float and hands it to `each` in blocks, in order. Returns false when the
/// file has no audio.
pub fn audio(path: &Path, each: impl FnMut(&[f32]) -> Result<(), String>) -> Result<bool, String> {
    super::audio::decode(path, each)
}

/// Pictures at (about) `times` seconds, their long side at most `max_pixel`, in order. Times that land on a frame
/// already taken are skipped.
pub fn frames(path: &Path, times: &[f64], max_pixel: usize, tolerance: f64) -> Result<Vec<Frame>, String> {
    super::video::frames(path, times, max_pixel, tolerance)
}

pub(crate) fn downmix(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// A windowed-sinc resampler (Blackman window, 64 taps a side at the lower rate's cutoff), streaming: for the rare
/// whatever rate the file was recorded at.
pub struct Resampler {
    ratio: f64,
    /// The windowed sinc sampled every 1/KERNEL_STEPS of an input sample over (-TAPS, TAPS), computed once: the
    /// trigonometry per tap per output sample cost a second per half-minute of audio.
    kernel: Vec<f32>,
    history: Vec<f32>,
    /// The next output's position in input samples, counted from the start of `history`.
    position: f64,
}

const TAPS: usize = 64;
const KERNEL_STEPS: usize = 256;

impl Resampler {
    pub fn new(from: u32, to: u32) -> Resampler {
        let ratio = from as f64 / to as f64;
        let cutoff = (1.0 / ratio).min(1.0) * 0.95;
        let kernel = (0..=2 * TAPS * KERNEL_STEPS)
            .map(|i| {
                let x = i as f64 / KERNEL_STEPS as f64 - TAPS as f64;
                let sinc = if x == 0.0 { 1.0 } else { (std::f64::consts::PI * x * cutoff).sin() / (std::f64::consts::PI * x * cutoff) };
                let t = (x / TAPS as f64 + 1.0) / 2.0; // 0…1 across the window
                let window = 0.42 - 0.5 * (2.0 * std::f64::consts::PI * t).cos() + 0.08 * (4.0 * std::f64::consts::PI * t).cos();
                (sinc * window) as f32
            })
            .collect();
        Resampler {
            ratio,
            kernel,
            history: vec![0.0; TAPS],
            position: TAPS as f64,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        self.history.extend_from_slice(input);
        let mut out = Vec::with_capacity((input.len() as f64 / self.ratio) as usize + 1);
        while self.position + (TAPS as f64) < self.history.len() as f64 {
            out.push(self.at(self.position));
            self.position += self.ratio;
        }
        // Keep what the next outputs still need.
        let keep_from = (self.position as usize).saturating_sub(TAPS);
        self.history.drain(..keep_from);
        self.position -= keep_from as f64;
        out
    }

    pub fn finish(&mut self) -> Vec<f32> {
        self.process(&vec![0.0; TAPS])
    }

    fn at(&self, position: f64) -> f32 {
        let center = position.floor() as isize;
        let frac = position - center as f64;
        let (mut sum, mut weights) = (0.0f32, 0.0f32);
        for i in -(TAPS as isize) + 1..TAPS as isize {
            let index = center + i;
            if index < 0 || index as usize >= self.history.len() {
                continue;
            }
            // x = i - frac, looked up between two kernel samples.
            let k = (i as f64 - frac + TAPS as f64) * KERNEL_STEPS as f64;
            let (k0, f) = (k.floor() as usize, (k - k.floor()) as f32);
            let weight = self.kernel[k0] + (self.kernel[(k0 + 1).min(self.kernel.len() - 1)] - self.kernel[k0]) * f;
            sum += self.history[index as usize] * weight;
            weights += weight;
        }
        if weights == 0.0 { 0.0 } else { sum / weights }
    }
}

/// One picture from a video.

/// 8×8 average hash, as the reference's: gray cells brighter than the mean are 1. Two frames within 4 bits of each
/// other show the same shot.
pub fn average_hash(image: &RgbImage) -> u64 {
    let side = 8;
    let mut cells = [0u32; 64];
    for cy in 0..side {
        let y0 = cy * image.height / side;
        let y1 = image
            .height
            .min((y0 + 1).max((cy + 1) * image.height / side));
        for cx in 0..side {
            let x0 = cx * image.width / side;
            let x1 = image.width.min((x0 + 1).max((cx + 1) * image.width / side));
            let (mut sum, mut n) = (0u32, 0u32);
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = (y * image.width + x) * 3;
                    sum += (image.pixels[p] as u32 * 299
                        + image.pixels[p + 1] as u32 * 587
                        + image.pixels[p + 2] as u32 * 114)
                        / 1000;
                    n += 1;
                }
            }
            cells[cy * side + cx] = sum.checked_div(n).unwrap_or(0);
        }
    }
    let mean = cells.iter().sum::<u32>() / 64;
    cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| **cell > mean)
        .fold(0u64, |hash, (i, _)| hash | 1 << i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_a_tone() {
        // A 440 Hz tone at 44.1 kHz → 16 kHz: same frequency (zero crossings), same level.
        let input: Vec<f32> = (0..44_100)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44_100.0).sin() * 0.5)
            .collect();
        let mut resampler = Resampler::new(44_100, 16_000);
        let mut out = resampler.process(&input[..20_000]);
        out.extend(resampler.process(&input[20_000..]));
        out.extend(resampler.finish());
        assert!((out.len() as i64 - 16_000).abs() < 80, "{}", out.len());
        let middle = &out[2000..14_000];
        let crossings = middle
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        assert!(
            (crossings as f64 - 440.0 * 12_000.0 / 16_000.0).abs() < 3.0,
            "{crossings}"
        );
        let peak = middle.iter().fold(0f32, |m, &x| m.max(x.abs()));
        assert!((peak - 0.5).abs() < 0.03, "{peak}");
    }

    #[test]
    fn hashes_tell_shots_apart() {
        let half = |left: u8, right: u8| {
            let mut pixels = Vec::new();
            for _ in 0..64 {
                for x in 0..64 {
                    let v = if x < 32 { left } else { right };
                    pixels.extend_from_slice(&[v, v, v]);
                }
            }
            RgbImage::new(64, 64, pixels)
        };
        let a = average_hash(&half(20, 230));
        let b = average_hash(&half(30, 220)); // the same shot, a little darker
        let c = average_hash(&half(230, 20)); // another shot
        assert!((a ^ b).count_ones() <= 4);
        assert!((a ^ c).count_ones() > 4);
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), [0.5, 0.5]);
    }
}
