//! PDFs in pure Rust, through pdf_oxide: each page's text, or a rendering of the page (tiny-skia) when it has too
//! little text (scans, slides) or its text came out in glyph order. The logic above the reader is truffle's (which
//! used PDFium); a bad file fails on its own, never the process.

use std::path::Path;

use pdf_oxide::PdfDocument;
use pdf_oxide::rendering::{ImageFormat, RenderOptions, render_page_fit};
use truffle_semantic::image::RgbImage;
use truffle_semantic::search_text::PageTextProblem;

use crate::{ExtractError, Result};

/// An open document.
pub struct Document {
    inner: PdfDocument,
    pages: usize,
}

/// Runs `body` with the document open. `Locked` when it needs a password, `Unreadable` when it won't open. A panic
/// inside the parser (a malformed file) becomes `Unreadable` too.
pub fn with_document<T>(path: &Path, body: impl FnOnce(&Document) -> Result<T>) -> Result<T> {
    let opened = std::panic::catch_unwind(|| PdfDocument::open(path));
    let inner = match opened {
        Ok(Ok(doc)) => doc,
        Ok(Err(pdf_oxide::Error::EncryptedPdf)) => return Err(ExtractError::Locked),
        Ok(Err(e)) => return Err(ExtractError::Unreadable(format!("can't open PDF: {e}"))),
        Err(_) => return Err(ExtractError::Unreadable("the PDF reader failed on this file".into())),
    };
    if inner.is_encrypted() && inner.authenticate(b"").map_or(true, |ok| !ok) {
        return Err(ExtractError::Locked);
    }
    let pages = inner.page_count().map_err(|e| ExtractError::Unreadable(format!("can't read PDF pages: {e}")))?;
    let document = Document { inner, pages };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&document)))
        .unwrap_or_else(|_| Err(ExtractError::Unreadable("the PDF reader failed on this file".into())))
}

impl Document {
    pub fn page_count(&self) -> usize {
        self.pages
    }

    /// A metadata string ("Title"…): pdf_oxide exposes only Producer and Creator, so the title comes from the file name.
    pub fn meta(&self, _tag: &str) -> Option<String> {
        None
    }

    pub fn page(&self, index: usize) -> Option<Page<'_>> {
        (index < self.pages).then_some(Page { document: self, index })
    }
}

pub struct Page<'a> {
    document: &'a Document,
    index: usize,
}

impl Page<'_> {
    /// The page's text in reading order.
    pub fn text(&self) -> String {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.document.inner.extract_text(self.index)))
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default()
    }

    /// The page as a picture fitted into a square of `pixels`, on white, annotations drawn.
    pub fn render(&self, pixels: usize) -> Option<RgbImage> {
        let mut options = RenderOptions::default();
        options.format = ImageFormat::RawRgba8;
        let px = pixels.max(1) as u32;
        let image = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_page_fit(&self.document.inner, self.index, px, px, &options)))
            .ok()?
            .ok()?;
        let (w, h) = (image.width as usize, image.height as usize);
        if w == 0 || h == 0 || image.data.len() < w * h * 4 {
            return None;
        }
        // Premultiplied RGBA on a white background: opaque, so the colour channels are the picture.
        Some(RgbImage::from_rgba_on_white(w, h, &image.data[..w * h * 4]))
    }
}

/// One page as the indexer takes it.
#[derive(Clone, Debug)]
pub struct PageContent {
    /// From 1.
    pub number: usize,
    /// May be empty (scans).
    pub text: String,
    /// Set when the page had too little text (or garbled text) and was rendered for the vision encoder.
    pub image: Option<RgbImage>,
    /// Text read in glyph order: too garbled to show or match words against, but enough of its words survive for a
    /// meaning vector beside the page's picture.
    pub garbled: Option<String>,
    /// The text came from OCR (a scan, or a text layer that lost letters).
    pub ocr: bool,
}

/// What was read of a PDF.
#[derive(Debug)]
pub struct Pages {
    pub pages: Vec<PageContent>,
    pub total: usize,
    pub title: Option<String>,
}

/// Text recognition, handed in by the caller (Vision on macOS): the PDF reader doesn't depend on it.
pub trait Ocr {
    /// The words in `image`; `arabic` asks for the Arabic engine.
    fn text(&self, image: &RgbImage, arabic: bool) -> String;
    fn reads_arabic(&self) -> bool;
}

/// Lost Arabic letters are read again by OCR at this size (small diacritics need it).
const ARABIC_OCR_PIXELS: usize = 2000;

/// Reads the pages in `range(total)` (0-based). Pages with real text become text; scans, slides and pages whose text
/// layer came out in glyph order are rendered at `render_pixels`. With `ocr`: a rendered page's words come from OCR
/// (for keyword search: an invoice number on a scan), and an Arabic text layer that lost letters is replaced by OCR's
/// reading when that's real Arabic. Bengali in glyph order isn't OCR'd: Windows' OCR doesn't read Bengali either.
pub fn pages(
    path: &Path,
    range: impl FnOnce(usize) -> std::ops::Range<usize>,
    min_text: usize,
    render_pixels: usize,
    ocr: Option<&dyn Ocr>,
) -> Result<Pages> {
    with_document(path, |document| {
        let total = document.page_count();
        let wanted = range(total);
        let mut pages = Vec::new();
        for index in wanted.start.min(total)..wanted.end.min(total) {
            let Some(page) = document.page(index) else {
                continue;
            };
            let number = index + 1;
            let mut text = page.text().trim().to_string();
            let mut garbled = None;
            match PageTextProblem::check(&text) {
                Some(PageTextProblem::GlyphOrder) => garbled = Some(std::mem::take(&mut text)),
                problem => {
                    if problem == Some(PageTextProblem::LostLetters) {
                        if let Some(ocr) = ocr.filter(|ocr| ocr.reads_arabic()) {
                            if let Some(image) = page.render(ARABIC_OCR_PIXELS) {
                                let read = ocr.text(&image, true).trim().to_string();
                                if read.chars().count() >= min_text
                                    && PageTextProblem::check(&read).is_none()
                                    && read.chars().any(|c| (0x0600..0x0700).contains(&(c as u32)))
                                {
                                    pages.push(PageContent {
                                        number,
                                        text: read,
                                        image: None,
                                        garbled: None,
                                        ocr: true,
                                    });
                                    continue;
                                }
                            }
                        }
                    }
                    if text.chars().count() >= min_text {
                        pages.push(PageContent {
                            number,
                            text,
                            image: None,
                            garbled: None,
                            ocr: false,
                        });
                        continue;
                    }
                }
            }
            let Some(image) = page.render(render_pixels) else {
                continue;
            };
            // A scan, or a page that's mostly a picture: its words come from OCR, for keyword search only, and its
            // meaning stays the picture's vector.
            let mut read_by_ocr = false;
            if let (Some(ocr), None) = (ocr, &garbled) {
                let read = ocr.text(&image, false).trim().to_string();
                if read.chars().count() > text.chars().count() {
                    text = read;
                    read_by_ocr = true;
                }
            }
            pages.push(PageContent {
                number,
                text,
                image: Some(image),
                garbled,
                ocr: read_by_ocr,
            });
        }
        Ok(Pages {
            pages,
            total,
            title: document.meta("Title"),
        })
    })
}

/// How many pages a PDF has (1 when it won't open: estimates only).
pub fn page_count(path: &Path) -> usize {
    with_document(path, |document| Ok(document.page_count())).unwrap_or(1)
}

/// Page `number` (from 1), fitted into a square of `pixels`: for previews.
pub fn render_page(path: &Path, number: usize, pixels: usize) -> Result<RgbImage> {
    with_document(path, |document| {
        let page = document
            .page(number.saturating_sub(1))
            .ok_or_else(|| ExtractError::Unreadable(format!("no page {number}")))?;
        page.render(pixels)
            .ok_or_else(|| ExtractError::Unreadable(format!("can't render page {number}")))
    })
}
