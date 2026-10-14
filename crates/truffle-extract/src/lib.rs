//! Truffle's readers: PDF (pdf_oxide, pure Rust), documents (docx, odt, rtf, legacy doc, html, plain text in any
//! common encoding), EXIF and SVG, and the media layer: pictures, audio, video and OCR (`media`).

pub mod docs;
pub mod exif;
pub mod pdf;
pub mod svg;
pub mod text;

/// Why a file gave nothing to index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtractError {
    Unreadable(String),
    TooSmall(usize, usize),
    Locked,
    Empty(String),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::Unreadable(why) => write!(f, "unreadable: {why}"),
            ExtractError::TooSmall(width, height) => write!(f, "small image {width}×{height}"),
            ExtractError::Locked => f.write_str("password-protected"),
            ExtractError::Empty(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ExtractError {}

impl ExtractError {
    /// The state a file gets: a picture too small to show anything is skipped; everything else failed.
    pub fn state(&self) -> &'static str {
        match self {
            ExtractError::TooSmall(..) | ExtractError::Empty(_) => "skipped",
            _ => "failed",
        }
    }
}

pub type Result<T> = std::result::Result<T, ExtractError>;
