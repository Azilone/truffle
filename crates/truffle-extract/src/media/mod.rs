//! Pictures, sound, video and OCR: the system's own decoders on macOS (ImageIO, AVFoundation, Vision), pure Rust
//! elsewhere (image, symphonia), ffmpeg for Linux video.

pub mod audio;
pub mod av;
pub mod image;
pub mod ocr;
pub mod video;
