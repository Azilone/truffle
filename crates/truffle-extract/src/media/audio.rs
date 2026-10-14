//! Soundtracks through symphonia (pure Rust): MP3, AAC (M4A, MP4/MOV soundtracks), ALAC, FLAC, Vorbis, WAV/AIFF,
//! CAF, MKV/WebM. Decoded a packet at a time, downmixed, resampled to 16 kHz and handed over in blocks, so a three-hour
//! podcast never sits in memory whole.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use super::av::{Info, Resampler, SAMPLE_RATE, downmix};

fn open(path: &Path) -> Result<Box<dyn FormatReader>, String> {
    let file = File::open(path).map_err(|e| format!("can't open: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| format!("can't read audio ({e})"))
}

/// The soundtrack's presence and length.
pub fn probe(path: &Path) -> Result<Info, String> {
    let format = open(path)?;
    let Some(track) = format.default_track(TrackType::Audio) else {
        return Ok(Info::default());
    };
    let duration = match (track.time_base, track.duration) {
        (Some(tb), Some(d)) => tb.calc_duration(d).map_or(0.0, |t| t.as_secs_f64()),
        _ => 0.0,
    };
    Ok(Info { duration, audio: true, ..Info::default() })
}

/// Blocks of about a second go to `each`.
const BLOCK: usize = SAMPLE_RATE as usize;

/// Symphonia first; when it can't decode the soundtrack at all (HE-AAC, Opus, AC-3…), the platform's decoder:
/// AVFoundation on macOS, ffmpeg on Linux.
pub fn decode(path: &Path, mut each: impl FnMut(&[f32]) -> Result<(), String>) -> Result<bool, String> {
    let mut delivered = 0usize;
    let first = decode_symphonia(path, &mut |block: &[f32]| {
        delivered += block.len();
        each(block)
    });
    match first {
        Ok(true) if delivered > 0 => Ok(true),
        Ok(false) => fallback::decode(path, &mut each).or(Ok(false)),
        Ok(true) | Err(_) if delivered == 0 => match fallback::decode(path, &mut each) {
            Ok(has) => Ok(has),
            Err(e) => first.and(Err(e)),
        },
        other => other,
    }
}

fn decode_symphonia(path: &Path, each: &mut dyn FnMut(&[f32]) -> Result<(), String>) -> Result<bool, String> {
    let mut format = open(path)?;
    let Some(track) = format.default_track(TrackType::Audio) else {
        return Ok(false);
    };
    let id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or("no audio codec parameters")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| format!("no decoder for this audio ({e})"))?;
    let mut resampler: Option<(u32, Resampler)> = None;
    let (mut interleaved, mut pending) = (Vec::<f32>::new(), Vec::<f32>::with_capacity(BLOCK * 2));
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // A truncated file ends where it's cut.
            Err(Error::IoError(_)) => break,
            Err(e) => return Err(format!("can't read audio ({e})")),
        };
        if packet.track_id != id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("can't decode audio ({e})")),
        };
        let (rate, channels) = (buffer.spec().rate(), buffer.spec().channels().count().max(1));
        interleaved.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut interleaved);
        let mono = downmix(&interleaved, channels);
        let out = if rate == SAMPLE_RATE {
            mono
        } else {
            let r = match &mut resampler {
                Some((at, r)) if *at == rate => r,
                slot => {
                    *slot = Some((rate, Resampler::new(rate, SAMPLE_RATE)));
                    &mut slot.as_mut().unwrap().1
                }
            };
            r.process(&mono)
        };
        pending.extend_from_slice(&out);
        while pending.len() >= BLOCK {
            each(&pending[..BLOCK])?;
            pending.drain(..BLOCK);
        }
    }
    if let Some((_, r)) = &mut resampler {
        pending.extend(r.finish());
    }
    if !pending.is_empty() {
        each(&pending)?;
    }
    Ok(true)
}

#[cfg(target_os = "macos")]
mod fallback {
    use super::*;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_av_foundation::{AVAssetReader, AVAssetReaderTrackOutput, AVMediaTypeAudio, AVURLAsset};
    use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

    /// kAudioFormatLinearPCM ('lpcm').
    const LINEAR_PCM: u32 = 0x6C70_636D;

    pub fn decode(path: &Path, each: &mut dyn FnMut(&[f32]) -> Result<(), String>) -> Result<bool, String> {
        objc2::rc::autoreleasepool(|_| unsafe {
            let text = path.to_str().ok_or("path isn't UTF-8")?;
            let asset = AVURLAsset::URLAssetWithURL_options(&NSURL::fileURLWithPath(&NSString::from_str(text)), None);
            #[allow(deprecated)]
            let tracks = asset.tracksWithMediaType(AVMediaTypeAudio.ok_or("no AVMediaTypeAudio")?);
            let Some(track) = tracks.firstObject() else { return Ok(false) };
            let reader = AVAssetReader::assetReaderWithAsset_error(&asset).map_err(|e| format!("can't read audio ({})", e.localizedDescription()))?;
            // The AVAudioSettings keys are strings equal to their names.
            let key = |k: &str| NSString::from_str(k);
            let keys = [
                key("AVFormatIDKey"),
                key("AVSampleRateKey"),
                key("AVNumberOfChannelsKey"),
                key("AVLinearPCMBitDepthKey"),
                key("AVLinearPCMIsFloatKey"),
                key("AVLinearPCMIsNonInterleaved"),
                key("AVLinearPCMIsBigEndianKey"),
            ];
            let values: [Retained<NSNumber>; 7] = [
                NSNumber::new_u32(LINEAR_PCM),
                NSNumber::new_f64(SAMPLE_RATE as f64),
                NSNumber::new_u32(1),
                NSNumber::new_u32(32),
                NSNumber::new_bool(true),
                NSNumber::new_bool(false),
                NSNumber::new_bool(false),
            ];
            let key_refs: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
            let value_refs: Vec<&AnyObject> = values.iter().map(|v| v.as_ref()).collect();
            let settings = NSDictionary::from_slices(&key_refs, &value_refs);
            let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(&track, Some(&settings));
            if !reader.canAddOutput(&output) {
                return Err("can't decode this audio".into());
            }
            reader.addOutput(&output);
            if !reader.startReading() {
                return Err("can't decode this audio".into());
            }
            let mut any = false;
            let mut floats: Vec<f32> = Vec::new();
            while let Some(sample) = output.copyNextSampleBuffer() {
                let Some(block) = sample.data_buffer() else { continue };
                let bytes = block.data_length();
                floats.resize(bytes / 4, 0.0);
                if bytes >= 4 && block.copy_data_bytes(0, floats.len() * 4, std::ptr::NonNull::new(floats.as_mut_ptr().cast()).unwrap()) == 0 {
                    any = true;
                    each(&floats)?;
                }
            }
            Ok(any)
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod fallback {
    use super::*;
    use std::io::Read;
    use std::process::{Command, Stdio};

    pub fn decode(path: &Path, each: &mut dyn FnMut(&[f32]) -> Result<(), String>) -> Result<bool, String> {
        let ffmpeg = std::env::var_os("PATH")
            .and_then(|p| p.to_str().map(|p| p.split(':').map(|d| Path::new(d).join("ffmpeg")).find(|f| f.is_file())))
            .flatten()
            .ok_or("no decoder for this audio (install ffmpeg)")?;
        let mut child = Command::new(ffmpeg)
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(path)
            .args(["-vn", "-ac", "1", "-ar", &SAMPLE_RATE.to_string(), "-f", "f32le", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("ffmpeg: {e}"))?;
        let mut out = child.stdout.take().ok_or("ffmpeg: no output")?;
        let (mut buf, mut any) = (vec![0u8; BLOCK * 4], false);
        loop {
            let mut filled = 0;
            while filled < buf.len() {
                match out.read(&mut buf[filled..]) {
                    Ok(0) => break,
                    Ok(n) => filled += n,
                    Err(e) => return Err(format!("ffmpeg: {e}")),
                }
            }
            let samples: Vec<f32> = buf[..filled - filled % 4].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            if !samples.is_empty() {
                any = true;
                each(&samples)?;
            }
            if filled < buf.len() {
                break;
            }
        }
        let _ = child.wait();
        Ok(any)
    }
}
