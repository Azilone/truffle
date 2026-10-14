//! Text in pictures, for keyword search (an invoice number in a screenshot, a scanned page's words). macOS: Vision, as
//! DigUp reads them (accurate, language correction, languages detected). Linux has no OCR yet: `is_available` says so
//! and pictures are still found by what they show.

use truffle_semantic::image::RgbImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    /// The languages Vision detects in the picture.
    UserProfile,
    Arabic,
}

pub fn is_available() -> bool {
    cfg!(target_os = "macos")
}

/// Accurate recognition reads Arabic on macOS 15 and later.
pub fn reads_arabic() -> bool {
    cfg!(target_os = "macos")
}

/// The text in `image`, a line per line read ("" when nothing is read, or OCR isn't available).
pub fn text(image: &RgbImage, script: Script) -> String {
    #[cfg(target_os = "macos")]
    {
        apple::text(image, script)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (image, script);
        String::new()
    }
}

#[cfg(target_os = "macos")]
mod apple {
    use super::*;
    use objc2::AnyThread;
    use objc2_core_graphics::CGImage;
    use objc2_foundation::{NSArray, NSDictionary, NSString};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDataProviderCreateWithData(info: *mut c_void, data: *const c_void, size: usize, release: *const c_void) -> *const c_void;
        fn CGDataProviderRelease(provider: *const c_void);
        fn CGColorSpaceCreateDeviceRGB() -> *const c_void;
        fn CGColorSpaceRelease(space: *const c_void);
        fn CGImageCreate(
            width: usize,
            height: usize,
            bits_per_component: usize,
            bits_per_pixel: usize,
            bytes_per_row: usize,
            space: *const c_void,
            bitmap_info: u32,
            provider: *const c_void,
            decode: *const f64,
            interpolate: bool,
            intent: i32,
        ) -> *const c_void;
        fn CGImageRelease(image: *const c_void);
    }

    pub fn text(image: &RgbImage, script: Script) -> String {
        if image.width == 0 || image.height == 0 {
            return String::new();
        }
        objc2::rc::autoreleasepool(|_| unsafe {
            // Packed RGB, no alpha (kCGImageAlphaNone): the pixels as they are, borrowed while Vision reads them.
            let provider = CGDataProviderCreateWithData(std::ptr::null_mut(), image.pixels.as_ptr().cast(), image.pixels.len(), std::ptr::null());
            let space = CGColorSpaceCreateDeviceRGB();
            let cg = CGImageCreate(image.width, image.height, 8, 24, image.width * 3, space, 0, provider, std::ptr::null(), false, 0);
            CGColorSpaceRelease(space);
            CGDataProviderRelease(provider);
            if cg.is_null() {
                return String::new();
            }
            let request = VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc());
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            request.setUsesLanguageCorrection(true);
            match script {
                Script::UserProfile => request.setAutomaticallyDetectsLanguage(true),
                Script::Arabic => request.setRecognitionLanguages(&NSArray::from_retained_slice(&[NSString::from_str("ar")])),
            }
            let handler = VNImageRequestHandler::initWithCGImage_options(VNImageRequestHandler::alloc(), &*(cg as *const CGImage), &NSDictionary::new());
            let requests: objc2::rc::Retained<NSArray<VNRequest>> = NSArray::from_slice(&[&**request]);
            let ok = handler.performRequests_error(&requests).is_ok();
            let mut lines = Vec::new();
            if ok && let Some(results) = request.results() {
                for observation in results.iter() {
                    if let Some(best) = observation.topCandidates(1).firstObject() {
                        let line = best.string().to_string();
                        if !line.trim().is_empty() {
                            lines.push(line);
                        }
                    }
                }
            }
            CGImageRelease(cg);
            lines.join("\n")
        })
    }
}
