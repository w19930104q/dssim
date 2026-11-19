use crate::Dssim;
use rgb::{RGB8, RGBA8, RGB16, RGBA16};
use std::ptr;

pub type DssimImage = crate::DssimImage<f32>;

/// Per-scale SSIM (difference) map, accessed via `dssim_ssim_maps_get`.
///
/// Opaque to C. Borrows from the `DssimSsimMaps` set it was obtained from —
/// it is valid until that set is freed with `dssim_free_ssim_maps`.
pub type DssimSsimMap = crate::SsimMap;

/// Owned set of SSIM maps returned by `dssim_compare2`.
///
/// Opaque to C. Owned by the caller once returned, and independent of the
/// images and the `Dssim` context — it stays valid after those are freed.
/// Free it with a single `dssim_free_ssim_maps` call.
pub struct DssimSsimMaps {
    maps: Vec<crate::SsimMap>,
}

/// Status code returned by `dssim_compare2`, so failures are diagnosable
/// instead of being conflated into a NaN score.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DssimStatus {
    /// Success; all output parameters have been written.
    Ok = 0,
    /// A required pointer argument was NULL (`dssim`, `img1`, `img2`,
    /// or `out_score`).
    NullPointer = 1,
    /// The two images have different dimensions and can't be compared.
    SizeMismatch = 2,
}

/// `width * height` in pixels, or `None` for zero/overflowing dimensions.
fn checked_area(width: u32, height: u32) -> Option<usize> {
    if width == 0 || height == 0 {
        return None;
    }
    (width as usize).checked_mul(height as usize)
}

/// Create new context for comparisons
#[unsafe(no_mangle)]
pub extern "C" fn dssim_new() -> *mut Dssim {
    let d = Box::new(crate::new());
    Box::into_raw(d)
}

/// Free the context
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_free(d: *mut Dssim) {
    if d.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(d) };
}

/// Take sRGB RGBA pixels (non-premultiplied, alpha last) and preprocess them into image format that can be compared.
///
/// Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
///
/// Call `dssim_free_image` to free memory when the image is no longer needed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_create_image_rgba(dssim: &mut Dssim, pixels: *const u8, width: u32, height: u32) -> *mut DssimImage {
    if pixels.is_null() {
        return ptr::null_mut();
    }
    let Some(area) = checked_area(width, height) else { return ptr::null_mut() };
    let pixels = unsafe { std::slice::from_raw_parts(pixels.cast::<RGBA8>(), area) };
    match dssim.create_image_rgba(pixels, width as usize, height as usize) {
        Some(img) => Box::into_raw(Box::new(img)),
        None => ptr::null_mut(),
    }
}

/// Take sRGB RGB pixels and preprocess them into image format that can be compared.
///
/// Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
///
/// Call `dssim_free_image` to free memory when the image is no longer needed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_create_image_rgb(dssim: &mut Dssim, pixels: *const u8, width: u32, height: u32) -> *mut DssimImage {
    if pixels.is_null() {
        return ptr::null_mut();
    }
    let Some(area) = checked_area(width, height) else { return ptr::null_mut() };
    let pixels = unsafe { std::slice::from_raw_parts(pixels.cast::<RGB8>(), area) };
    match dssim.create_image_rgb(pixels, width as usize, height as usize) {
        Some(img) => Box::into_raw(Box::new(img)),
        None => ptr::null_mut(),
    }
}

/// Take 16-bit sRGB RGBA pixels (non-premultiplied, alpha last, native-endian,
/// full range 0..=65535) and preprocess them into image format that can be compared.
///
/// This keeps the full 16-bit precision — the gamma-to-linear conversion uses
/// a 65536-entry lookup table, so no precision is lost to 8-bit quantization.
///
/// Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
///
/// Call `dssim_free_image` to free memory when the image is no longer needed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_create_image_rgba16(dssim: &mut Dssim, pixels: *const u16, width: u32, height: u32) -> *mut DssimImage {
    if pixels.is_null() {
        return ptr::null_mut();
    }
    let Some(area) = checked_area(width, height) else { return ptr::null_mut() };
    let pixels = unsafe { std::slice::from_raw_parts(pixels.cast::<RGBA16>(), area) };
    let img = imgref::ImgVec::new(crate::ToRGBAPLU::to_rgbaplu(pixels), width as usize, height as usize);
    match dssim.create_image(&img) {
        Some(img) => Box::into_raw(Box::new(img)),
        None => ptr::null_mut(),
    }
}

/// Take 16-bit sRGB RGB pixels (native-endian, full range 0..=65535) and
/// preprocess them into image format that can be compared.
///
/// Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
///
/// Call `dssim_free_image` to free memory when the image is no longer needed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_create_image_rgb16(dssim: &mut Dssim, pixels: *const u16, width: u32, height: u32) -> *mut DssimImage {
    if pixels.is_null() {
        return ptr::null_mut();
    }
    let Some(area) = checked_area(width, height) else { return ptr::null_mut() };
    let pixels = unsafe { std::slice::from_raw_parts(pixels.cast::<RGB16>(), area) };
    let img = imgref::ImgVec::new(crate::ToRGBAPLU::to_rgblu(pixels), width as usize, height as usize);
    match dssim.create_image(&img) {
        Some(img) => Box::into_raw(Box::new(img)),
        None => ptr::null_mut(),
    }
}

/// Free image data
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_free_image(img: *mut DssimImage) {
    if img.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(img) };
}

/// Compare these two images.
///
/// Returns NaN if either image is NULL or their dimensions don't match.
/// For diagnosable errors and SSIM map output, use `dssim_compare2`.
///
/// `img1` can be reused for multiple comparisons.
///
/// Don't forget to free the images and the DSSIM context when done.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_compare(dssim: &mut Dssim, img1: *const DssimImage, img2: *const DssimImage) -> f64 {
    let Some(img1) = (unsafe { img1.as_ref() }) else { return f64::NAN };
    let Some(img2) = (unsafe { img2.as_ref() }) else { return f64::NAN };
    if img1.width() != img2.width() || img1.height() != img2.height() {
        return f64::NAN;
    }
    let (val, _) = dssim.compare(img1, img2);
    val.into()
}

/// Compare two images, with diagnosable errors and optional SSIM (difference) maps.
///
/// * `out_score` — required. Receives the DSSIM score (0 = identical).
/// * `out_maps` — optional. If non-NULL, receives a newly allocated set of
///   SSIM maps, one per scale, largest (full resolution) first. Map saving is
///   enabled automatically for this call; the context's own settings are not
///   changed. Pass NULL if you only want the score.
///
/// Each map pixel is a float SSIM value in [0, 1]: 1.0 means identical,
/// lower values mark more-different regions (i.e. `1 - value` is the
/// per-pixel dissimilarity). See `dssim_ssim_map_data`.
///
/// Ownership: the returned map set is owned by the caller, does not borrow
/// from the images or the context, and must be freed with a single
/// `dssim_free_ssim_maps` call. Maps and their data pointers obtained from
/// the set stay valid until then.
///
/// Returns `DssimStatus_Ok` on success. On failure no output is written:
/// `DssimStatus_NullPointer` if a required pointer is NULL,
/// `DssimStatus_SizeMismatch` if the images have different dimensions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_compare2(
    dssim: *mut Dssim,
    img1: *const DssimImage,
    img2: *const DssimImage,
    out_score: *mut f64,
    out_maps: *mut *mut DssimSsimMaps,
) -> DssimStatus {
    if dssim.is_null() || img1.is_null() || img2.is_null() || out_score.is_null() {
        return DssimStatus::NullPointer;
    }
    let dssim = unsafe { &mut *dssim };
    let img1 = unsafe { &*img1 };
    let img2 = unsafe { &*img2 };

    if img1.width() != img2.width() || img1.height() != img2.height() {
        return DssimStatus::SizeMismatch;
    }

    let want_maps = !out_maps.is_null();
    let (val, maps) = if want_maps {
        // Save maps for all scales, just for this call.
        let prev = dssim.save_maps_scales();
        dssim.set_save_ssim_maps(u8::MAX);
        let res = dssim.compare(img1, img2);
        dssim.set_save_ssim_maps(prev);
        res
    } else {
        dssim.compare(img1, img2)
    };

    unsafe { out_score.write(val.into()) };

    if want_maps {
        unsafe { out_maps.write(Box::into_raw(Box::new(DssimSsimMaps { maps }))) };
    }

    DssimStatus::Ok
}

/// Number of maps in a set returned by `dssim_compare2` (one per scale).
/// Returns 0 for a NULL set.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_ssim_maps_count(maps: *const DssimSsimMaps) -> usize {
    let Some(maps) = (unsafe { maps.as_ref() }) else { return 0 };
    maps.maps.len()
}

/// Get the `index`-th map of a set (largest scale first), or NULL if the set
/// is NULL or the index is out of bounds. The returned pointer borrows from
/// the set: do not free it separately, and do not use it after the set is
/// freed with `dssim_free_ssim_maps`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_ssim_maps_get(maps: *const DssimSsimMaps, index: usize) -> *const DssimSsimMap {
    let Some(maps) = (unsafe { maps.as_ref() }) else { return ptr::null() };
    maps.maps.get(index).map_or(ptr::null(), |m| m as *const DssimSsimMap)
}

/// Width of an SSIM map in pixels. Returns 0 for a NULL map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_ssim_map_width(map: *const DssimSsimMap) -> u32 {
    let Some(map) = (unsafe { map.as_ref() }) else { return 0 };
    map.map.width() as u32
}

/// Height of an SSIM map in pixels. Returns 0 for a NULL map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_ssim_map_height(map: *const DssimSsimMap) -> u32 {
    let Some(map) = (unsafe { map.as_ref() }) else { return 0 };
    map.map.height() as u32
}

/// Average SSIM of this scale (not DSSIM). Returns NaN for a NULL map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_ssim_map_ssim(map: *const DssimSsimMap) -> f64 {
    let Some(map) = (unsafe { map.as_ref() }) else { return f64::NAN };
    map.ssim
}

/// Pointer to `width * height` tightly packed row-major floats, one SSIM
/// value per pixel in [0, 1] (1.0 = identical; `1 - value` = dissimilarity).
///
/// The pointer borrows from the map: it stays valid until the map is freed
/// with `dssim_free_ssim_maps`. Returns NULL for a NULL map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_ssim_map_data(map: *const DssimSsimMap) -> *const f32 {
    let Some(map) = (unsafe { map.as_ref() }) else { return ptr::null() };
    map.map.buf().as_ptr()
}

/// Free a set of SSIM maps returned by `dssim_compare2`.
///
/// Frees the set and all maps in it, invalidating all pointers previously
/// obtained from `dssim_ssim_maps_get` and `dssim_ssim_map_data` for this
/// set. Safe to call with a NULL pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dssim_free_ssim_maps(maps: *mut DssimSsimMaps) {
    if maps.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(maps) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToRGBAPLU;

    fn load_pngs() -> (lodepng::Bitmap<rgb::RGBA<u8>>, lodepng::Bitmap<rgb::RGBA<u8>>) {
        let file1 = lodepng::decode32_file("../tests/test1-sm.png").unwrap();
        let file2 = lodepng::decode32_file("../tests/test2-sm.png").unwrap();
        (file1, file2)
    }

    /// The 8-bit C API must produce exactly the same result as the Rust API
    /// for the same input.
    #[test]
    fn c_api_8bit_matches_rust_api() {
        let (file1, file2) = load_pngs();

        // Rust API
        let d = crate::new();
        let img1 = d.create_image_rgba(&file1.buffer, file1.width, file1.height).unwrap();
        let img2 = d.create_image_rgba(&file2.buffer, file2.width, file2.height).unwrap();
        let (rust_score, _) = d.compare(&img1, &img2);
        let rust_score = f64::from(rust_score);

        // C API
        unsafe {
            let cd = dssim_new();
            let cimg1 = dssim_create_image_rgba(&mut *cd, file1.buffer.as_ptr().cast(), file1.width as u32, file1.height as u32);
            let cimg2 = dssim_create_image_rgba(&mut *cd, file2.buffer.as_ptr().cast(), file2.width as u32, file2.height as u32);
            assert!(!cimg1.is_null() && !cimg2.is_null());
            let c_score = dssim_compare(&mut *cd, cimg1, cimg2);
            assert_eq!(c_score.to_bits(), rust_score.to_bits(), "C API and Rust API disagree");

            let mut score2 = f64::NAN;
            assert_eq!(dssim_compare2(cd, cimg1, cimg2, &mut score2, ptr::null_mut()), DssimStatus::Ok);
            assert_eq!(score2.to_bits(), rust_score.to_bits(), "dssim_compare2 and Rust API disagree");

            dssim_free_image(cimg1);
            dssim_free_image(cimg2);
            dssim_free(cd);
        }
    }

    /// 16-bit C API must match the equivalent Rust path, and must preserve
    /// more precision than the 8-bit path (a smooth 16-bit gradient is not
    /// quantized to 8-bit steps).
    #[test]
    fn c_api_16bit() {
        let (file1, _) = load_pngs();
        let (w, h) = (file1.width, file1.height);
        // Expand 8-bit to 16-bit (v * 257), and a copy with the LSB nudged —
        // a difference invisible in 8-bit.
        let px16: Vec<RGBA16> = file1.buffer.iter().map(|p| RGBA16 {
            r: u16::from(p.r) * 257,
            g: u16::from(p.g) * 257,
            b: u16::from(p.b) * 257,
            a: u16::from(p.a) * 257,
        }).collect();
        let mut px16_nudged = px16.clone();
        for p in &mut px16_nudged {
            p.r = p.r.saturating_add(1);
        }

        unsafe {
            let cd = dssim_new();
            let a = dssim_create_image_rgba16(&mut *cd, px16.as_ptr().cast(), w as u32, h as u32);
            let b = dssim_create_image_rgba16(&mut *cd, px16_nudged.as_ptr().cast(), w as u32, h as u32);
            assert!(!a.is_null() && !b.is_null());

            // Same result as the equivalent Rust calls
            let d = crate::new();
            let ra = d.create_image(&imgref::ImgVec::new(px16.to_rgbaplu(), w, h)).unwrap();
            let rb = d.create_image(&imgref::ImgVec::new(px16_nudged.to_rgbaplu(), w, h)).unwrap();
            let (rust_score, _) = d.compare(&ra, &rb);

            let mut c_score = f64::NAN;
            assert_eq!(dssim_compare2(cd, a, b, &mut c_score, ptr::null_mut()), DssimStatus::Ok);
            assert_eq!(c_score.to_bits(), f64::from(rust_score).to_bits());

            // The LSB nudge is below 8-bit resolution: the 8-bit path must
            // report identity, the 16-bit path must see the difference.
            let px8: Vec<RGBA8> = px16.iter().map(|p| RGBA8 { r: (p.r / 257) as u8, g: (p.g / 257) as u8, b: (p.b / 257) as u8, a: (p.a / 257) as u8 }).collect();
            let px8_nudged: Vec<RGBA8> = px16_nudged.iter().map(|p| RGBA8 { r: (p.r / 257) as u8, g: (p.g / 257) as u8, b: (p.b / 257) as u8, a: (p.a / 257) as u8 }).collect();
            assert_eq!(px8, px8_nudged, "test premise: nudge must be invisible in 8-bit");
            assert!(c_score > 0.0, "16-bit path must detect the sub-8-bit difference, got {c_score}");

            // RGB16 variant: NULL and zero-size are rejected
            assert!(dssim_create_image_rgb16(&mut *cd, ptr::null(), 4, 4).is_null());
            assert!(dssim_create_image_rgb16(&mut *cd, px16.as_ptr().cast(), 0, 4).is_null());
            assert!(dssim_create_image_rgb16(&mut *cd, px16.as_ptr().cast(), 4, 0).is_null());
            assert!(dssim_create_image_rgba16(&mut *cd, ptr::null(), 4, 4).is_null());
            assert!(dssim_create_image_rgba(&mut *cd, ptr::null(), 4, 4).is_null());
            assert!(dssim_create_image_rgb(&mut *cd, ptr::null(), 4, 4).is_null());
            assert!(dssim_create_image_rgba(&mut *cd, px8.as_ptr().cast(), 0, 0).is_null());

            dssim_free_image(a);
            dssim_free_image(b);
            dssim_free(cd);
        }
    }

    /// Failures must be diagnosable: NULL pointers and size mismatches get
    /// distinct status codes, and outputs stay untouched on failure.
    #[test]
    fn compare2_error_diagnosis() {
        let (file1, _) = load_pngs();
        unsafe {
            let cd = dssim_new();
            let img = dssim_create_image_rgba(&mut *cd, file1.buffer.as_ptr().cast(), file1.width as u32, file1.height as u32);
            let small = dssim_create_image_rgba(&mut *cd, file1.buffer.as_ptr().cast(), 4, 4);
            assert!(!img.is_null() && !small.is_null());

            let sentinel = -1.0f64;
            let mut score = sentinel;

            // NULL pointers, each in turn
            assert_eq!(dssim_compare2(ptr::null_mut(), img, img, &mut score, ptr::null_mut()), DssimStatus::NullPointer);
            assert_eq!(dssim_compare2(cd, ptr::null(), img, &mut score, ptr::null_mut()), DssimStatus::NullPointer);
            assert_eq!(dssim_compare2(cd, img, ptr::null(), &mut score, ptr::null_mut()), DssimStatus::NullPointer);
            assert_eq!(dssim_compare2(cd, img, img, ptr::null_mut(), ptr::null_mut()), DssimStatus::NullPointer);
            assert_eq!(score, sentinel, "outputs must not be written on failure");

            // Size mismatch is a distinct, diagnosable error (not a panic)
            assert_eq!(dssim_compare2(cd, img, small, &mut score, ptr::null_mut()), DssimStatus::SizeMismatch);
            assert_eq!(score, sentinel);
            // Legacy API reports it as NaN instead of crashing
            assert!(dssim_compare(&mut *cd, img, small).is_nan());

            // Success path
            assert_eq!(dssim_compare2(cd, img, img, &mut score, ptr::null_mut()), DssimStatus::Ok);
            assert_eq!(score, 0.0, "identity must be exactly 0");

            dssim_free_image(img);
            dssim_free_image(small);
            dssim_free(cd);
        }
    }

    /// SSIM (difference) maps: correct scales, valid data, clear ownership.
    #[test]
    fn ssim_maps_output() {
        let (file1, file2) = load_pngs();
        unsafe {
            let cd = dssim_new();
            let img1 = dssim_create_image_rgba(&mut *cd, file1.buffer.as_ptr().cast(), file1.width as u32, file1.height as u32);
            let img2 = dssim_create_image_rgba(&mut *cd, file2.buffer.as_ptr().cast(), file2.width as u32, file2.height as u32);

            let mut score = f64::NAN;
            let mut maps: *mut DssimSsimMaps = ptr::null_mut();
            assert_eq!(dssim_compare2(cd, img1, img2, &mut score, &mut maps), DssimStatus::Ok);
            assert!(!maps.is_null());

            // One map per scale (5 by default), largest first, each smaller
            let count = dssim_ssim_maps_count(maps);
            assert_eq!(count, 5);
            let mut prev_area = usize::MAX;
            for i in 0..count {
                let m = dssim_ssim_maps_get(maps, i);
                assert!(!m.is_null());
                let (w, h) = (dssim_ssim_map_width(m), dssim_ssim_map_height(m));
                assert!(w > 0 && h > 0);
                let area = w as usize * h as usize;
                assert!(area < prev_area, "maps must be in descending scale order");
                prev_area = area;

                let ssim = dssim_ssim_map_ssim(m);
                assert!(ssim.is_finite() && (0.0..=1.0).contains(&ssim));

                let data = dssim_ssim_map_data(m);
                assert!(!data.is_null());
                let pixels = std::slice::from_raw_parts(data, area);
                assert!(pixels.iter().all(|&v| v.is_finite() && (0.0..=1.0).contains(&v)));
            }
            // The full-resolution map comes first
            let first = dssim_ssim_maps_get(maps, 0);
            assert_eq!(dssim_ssim_map_width(first) as usize, file1.width);
            assert_eq!(dssim_ssim_map_height(first) as usize, file1.height);
            // Out of bounds / NULL handling
            assert!(dssim_ssim_maps_get(maps, count).is_null());
            assert_eq!(dssim_ssim_maps_count(ptr::null()), 0);
            assert!(dssim_ssim_maps_get(ptr::null(), 0).is_null());
            assert_eq!(dssim_ssim_map_width(ptr::null()), 0);
            assert!(dssim_ssim_map_data(ptr::null()).is_null());

            // Maps outlive the images and the context
            dssim_free_image(img1);
            dssim_free_image(img2);
            dssim_free(cd);
            let first = dssim_ssim_maps_get(maps, 0);
            assert!(dssim_ssim_map_ssim(first).is_finite());

            dssim_free_ssim_maps(maps);
            dssim_free_ssim_maps(ptr::null_mut()); // NULL is a no-op
        }
    }
}
