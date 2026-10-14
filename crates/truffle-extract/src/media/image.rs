//! Pictures to RGB for the model. macOS decodes with ImageIO, as DigUp does (straight to the thumbnail size, EXIF
//! orientation applied, HEIC/RAW/AVIF included); elsewhere the image crate decodes and fast_image_resize scales.

use std::path::Path;

use truffle_semantic::image::RgbImage;

/// A decoded picture: scaled so its long side is at most `max_pixel`, and its size as stored.
pub struct Decoded {
    pub image: RgbImage,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    TooSmall(usize, usize),
    Failed(String),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::Failed(message) => f.write_str(message),
            ImageError::TooSmall(width, height) => write!(f, "small image {width}×{height}"),
        }
    }
}

/// The picture's size as stored, without decoding it.
pub fn size(path: &Path) -> Option<(usize, usize)> {
    #[cfg(target_os = "macos")]
    if let Some(s) = apple::size(path) {
        return Some(s);
    }
    image::image_dimensions(path).ok().map(|(w, h)| (w as usize, h as usize))
}

/// Decodes the picture (the first frame), scaled so its long side is at most `max_pixel`, composited on white.
/// `min_side`: a picture whose long side is smaller is too small to show anything (`TooSmall`).
pub fn decode(path: &Path, max_pixel: usize, min_side: usize) -> Result<Decoded, ImageError> {
    #[cfg(target_os = "macos")]
    {
        apple::decode(path, max_pixel, min_side)
    }
    #[cfg(not(target_os = "macos"))]
    {
        portable::decode(path, max_pixel, min_side)
    }
}

/// Writes `image` as a JPEG (quality 0–1): previews and thumbnails.
pub fn save_jpeg(image: &RgbImage, path: &Path, quality: f32) -> Result<(), ImageError> {
    let file = std::fs::File::create(path).map_err(|e| ImageError::Failed(format!("can't write {}: {e}", path.display())))?;
    let mut out = std::io::BufWriter::new(file);
    let q = (quality.clamp(0.0, 1.0) * 100.0).round() as u8;
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q.max(1))
        .encode(&image.pixels, image.width as u32, image.height as u32, image::ExtendedColorType::Rgb8)
        .map_err(|e| ImageError::Failed(format!("can't write JPEG: {e}")))
}

/// Scales RGB down so the long side is at most `max_side` (never up): a SIMD box/bilinear filter.
pub fn shrink(image: &RgbImage, max_side: usize) -> RgbImage {
    let (w, h) = truffle_semantic::image::fit(image.width, image.height, max_side);
    if (w, h) == (image.width, image.height) {
        return image.clone();
    }
    resize(image, w, h)
}

fn resize(image: &RgbImage, w: usize, h: usize) -> RgbImage {
    use fast_image_resize::images::{Image, ImageRef};
    use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
    let (Ok(src), true) = (ImageRef::new(image.width as u32, image.height as u32, &image.pixels, PixelType::U8x3), w > 0 && h > 0) else {
        return image.clone();
    };
    let mut dst = Image::new(w as u32, h as u32, PixelType::U8x3);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));
    if Resizer::new().resize(&src, &mut dst, &options).is_err() {
        return image.clone();
    }
    RgbImage::new(w, h, dst.into_vec())
}

#[cfg(not(target_os = "macos"))]
mod portable {
    use super::*;

    pub fn decode(path: &Path, max_pixel: usize, min_side: usize) -> Result<Decoded, ImageError> {
        let failed = |e: image::ImageError| ImageError::Failed(format!("can't decode image: {e}"));
        let reader = image::ImageReader::open(path)
            .map_err(|e| ImageError::Failed(format!("can't open image: {e}")))?
            .with_guessed_format()
            .map_err(|e| ImageError::Failed(format!("can't read image: {e}")))?;
        let decoded = reader.decode().map_err(failed)?;
        let (width, height) = (decoded.width() as usize, decoded.height() as usize);
        if width.max(height) < min_side {
            return Err(ImageError::TooSmall(width, height));
        }
        let rgba = decoded.to_rgba8();
        let rgb = RgbImage::from_rgba_on_white(width, height, rgba.as_raw());
        let rgb = rgb.oriented(crate::exif::read(path).and_then(|e| e.orientation).unwrap_or(1));
        Ok(Decoded { image: shrink(&rgb, max_pixel), width, height })
    }
}

/// ImageIO and Core Graphics are plain C: the dozen calls needed are declared here.
#[cfg(target_os = "macos")]
pub(crate) mod apple {
    use super::*;
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;

    pub(crate) type CFTypeRef = *const c_void;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: CFTypeRef);
        fn CFURLCreateFromFileSystemRepresentation(alloc: CFTypeRef, buf: *const u8, len: isize, is_dir: u8) -> CFTypeRef;
        fn CFDictionaryCreate(
            alloc: CFTypeRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            n: isize,
            key_cb: *const c_void,
            value_cb: *const c_void,
        ) -> CFTypeRef;
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFNumberCreate(alloc: CFTypeRef, kind: isize, value: *const c_void) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, kind: isize, value: *mut c_void) -> u8;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: CFTypeRef;
        static kCFBooleanFalse: CFTypeRef;
    }

    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        fn CGImageSourceCreateWithURL(url: CFTypeRef, options: CFTypeRef) -> CFTypeRef;
        fn CGImageSourceGetType(source: CFTypeRef) -> CFTypeRef;
        fn CGImageSourceCopyPropertiesAtIndex(source: CFTypeRef, index: usize, options: CFTypeRef) -> CFTypeRef;
        fn CGImageSourceCreateThumbnailAtIndex(source: CFTypeRef, index: usize, options: CFTypeRef) -> CFTypeRef;
        static kCGImageSourceShouldCache: CFTypeRef;
        static kCGImageSourceShouldCacheImmediately: CFTypeRef;
        static kCGImageSourceCreateThumbnailFromImageAlways: CFTypeRef;
        static kCGImageSourceThumbnailMaxPixelSize: CFTypeRef;
        static kCGImageSourceCreateThumbnailWithTransform: CFTypeRef;
        static kCGImagePropertyPixelWidth: CFTypeRef;
        static kCGImagePropertyPixelHeight: CFTypeRef;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGImageGetWidth(image: CFTypeRef) -> usize;
        fn CGImageGetHeight(image: CFTypeRef) -> usize;
        fn CGColorSpaceCreateDeviceRGB() -> CFTypeRef;
        fn CGColorSpaceRelease(space: CFTypeRef);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits: usize,
            row_bytes: usize,
            space: CFTypeRef,
            info: u32,
        ) -> CFTypeRef;
        fn CGContextRelease(context: CFTypeRef);
        fn CGContextSetRGBFillColor(context: CFTypeRef, r: f64, g: f64, b: f64, a: f64);
        fn CGContextFillRect(context: CFTypeRef, rect: CGRect);
        fn CGContextDrawImage(context: CFTypeRef, rect: CGRect, image: CFTypeRef);
    }

    const NUMBER_SINT64: isize = 4;
    /// kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big: R G B A in memory.
    const RGBA_PREMULTIPLIED: u32 = 1 | (4 << 12);

    /// A CF object released on drop.
    struct Owned(CFTypeRef);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn dictionary(pairs: &[(CFTypeRef, CFTypeRef)]) -> Owned {
        let keys: Vec<CFTypeRef> = pairs.iter().map(|p| p.0).collect();
        let values: Vec<CFTypeRef> = pairs.iter().map(|p| p.1).collect();
        Owned(unsafe {
            CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                pairs.len() as isize,
                &raw const kCFTypeDictionaryKeyCallBacks,
                &raw const kCFTypeDictionaryValueCallBacks,
            )
        })
    }

    fn number(value: i64) -> Owned {
        Owned(unsafe { CFNumberCreate(std::ptr::null(), NUMBER_SINT64, (&raw const value).cast()) })
    }

    fn get_int(dict: CFTypeRef, key: CFTypeRef) -> usize {
        let v = unsafe { CFDictionaryGetValue(dict, key) };
        let mut out = 0i64;
        if !v.is_null() {
            unsafe { CFNumberGetValue(v, NUMBER_SINT64, (&raw mut out).cast()) };
        }
        out.max(0) as usize
    }

    fn source(path: &Path) -> Option<Owned> {
        let bytes = path.as_os_str().as_bytes();
        let url = Owned(unsafe { CFURLCreateFromFileSystemRepresentation(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize, 0) });
        if url.0.is_null() {
            return None;
        }
        let options = dictionary(&[unsafe { (kCGImageSourceShouldCache, kCFBooleanFalse) }]);
        let source = Owned(unsafe { CGImageSourceCreateWithURL(url.0, options.0) });
        (!source.0.is_null() && !unsafe { CGImageSourceGetType(source.0) }.is_null()).then_some(source)
    }

    fn stored_size(source: &Owned) -> (usize, usize) {
        let props = Owned(unsafe { CGImageSourceCopyPropertiesAtIndex(source.0, 0, std::ptr::null()) });
        if props.0.is_null() {
            return (0, 0);
        }
        unsafe { (get_int(props.0, kCGImagePropertyPixelWidth), get_int(props.0, kCGImagePropertyPixelHeight)) }
    }

    pub fn size(path: &Path) -> Option<(usize, usize)> {
        let source = source(path)?;
        let s = stored_size(&source);
        (s.0 > 0 && s.1 > 0).then_some(s)
    }

    pub fn decode(path: &Path, max_pixel: usize, min_side: usize) -> Result<Decoded, ImageError> {
        // ImageIO can't open some pictures (SVG goes through resvg elsewhere); the image crate tries them.
        let Some(source) = source(path) else {
            return fallback(path, max_pixel, min_side);
        };
        let (width, height) = stored_size(&source);
        if width.max(height) < min_side {
            return Err(ImageError::TooSmall(width, height));
        }
        let max = number(max_pixel as i64);
        let options = unsafe {
            dictionary(&[
                (kCGImageSourceCreateThumbnailFromImageAlways, kCFBooleanTrue),
                (kCGImageSourceThumbnailMaxPixelSize, max.0),
                (kCGImageSourceCreateThumbnailWithTransform, kCFBooleanTrue),
                (kCGImageSourceShouldCacheImmediately, kCFBooleanTrue),
            ])
        };
        let thumb = Owned(unsafe { CGImageSourceCreateThumbnailAtIndex(source.0, 0, options.0) });
        if thumb.0.is_null() {
            return Err(ImageError::Failed("can't decode image".into()));
        }
        let image = draw_on_white(thumb.0).ok_or_else(|| ImageError::Failed("can't draw image".into()))?;
        Ok(Decoded { image, width, height })
    }

    /// The CGImage drawn into an RGBA bitmap filled with white first (transparent parts come out white).
    pub(crate) fn draw_on_white(image: CFTypeRef) -> Option<RgbImage> {
        let (w, h) = unsafe { (CGImageGetWidth(image), CGImageGetHeight(image)) };
        if w == 0 || h == 0 {
            return None;
        }
        let mut rgba = vec![0u8; w * h * 4];
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            let context = CGBitmapContextCreate(rgba.as_mut_ptr().cast(), w, h, 8, w * 4, space, RGBA_PREMULTIPLIED);
            CGColorSpaceRelease(space);
            if context.is_null() {
                return None;
            }
            let rect = CGRect { x: 0.0, y: 0.0, w: w as f64, h: h as f64 };
            CGContextSetRGBFillColor(context, 1.0, 1.0, 1.0, 1.0);
            CGContextFillRect(context, rect);
            CGContextDrawImage(context, rect, image);
            CGContextRelease(context);
        }
        let mut pixels = Vec::with_capacity(w * h * 3);
        for p in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&p[..3]);
        }
        Some(RgbImage::new(w, h, pixels))
    }

    fn fallback(path: &Path, max_pixel: usize, min_side: usize) -> Result<Decoded, ImageError> {
        let decoded = image::open(path).map_err(|e| ImageError::Failed(format!("can't decode image: {e}")))?;
        let (width, height) = (decoded.width() as usize, decoded.height() as usize);
        if width.max(height) < min_side {
            return Err(ImageError::TooSmall(width, height));
        }
        let rgb = RgbImage::from_rgba_on_white(width, height, decoded.to_rgba8().as_raw());
        Ok(Decoded { image: shrink(&rgb, max_pixel), width, height })
    }
}
