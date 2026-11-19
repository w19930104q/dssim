/**
 * Use cargo build --release to create target/release/libdssim_core.a
 *
 * or
 *
 * rustup update
 * cargo install cargo-c
 * cargo cbuild --release # makes target/<platform>/release/libdssim.{a,so,pc}
 * cargo cinstall --release --verbose # installs libdssim.so globally
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Configuration for the comparison
 */
typedef struct Dssim Dssim;

/**
 * Abstract wrapper for images. See [`Dssim::create_image()`]
 */
typedef struct DssimImage_f32 DssimImage;

/**
 * Per-scale SSIM (difference) map, accessed via `dssim_ssim_maps_get`.
 *
 * Opaque. Borrows from the `DssimSsimMaps` set it was obtained from —
 * it is valid until that set is freed with `dssim_free_ssim_maps`.
 */
typedef struct DssimSsimMap DssimSsimMap;

/**
 * Owned set of SSIM maps returned by `dssim_compare2`.
 *
 * Opaque. Owned by the caller once returned, and independent of the
 * images and the `Dssim` context — it stays valid after those are freed.
 * Free it with a single `dssim_free_ssim_maps` call.
 */
typedef struct DssimSsimMaps DssimSsimMaps;

/**
 * Status code returned by `dssim_compare2`, so failures are diagnosable
 * instead of being conflated into a NaN score.
 */
typedef enum DssimStatus {
    /**
     * Success; all output parameters have been written.
     */
    DSSIM_STATUS_OK = 0,
    /**
     * A required pointer argument was NULL (`dssim`, `img1`, `img2`,
     * or `out_score`).
     */
    DSSIM_STATUS_NULL_POINTER = 1,
    /**
     * The two images have different dimensions and can't be compared.
     */
    DSSIM_STATUS_SIZE_MISMATCH = 2,
} DssimStatus;

/**
 * Create new context for comparisons
 */
struct Dssim *dssim_new(void);

/**
 * Free the context
 */
void dssim_free(struct Dssim *d);

/**
 * Take sRGB RGBA pixels (non-premultiplied, alpha last) and preprocess them into image format that can be compared.
 *
 * Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
 *
 * Call `dssim_free_image` to free memory when the image is no longer needed.
 */
DssimImage *dssim_create_image_rgba(struct Dssim *dssim,
                                    const uint8_t *pixels,
                                    uint32_t width,
                                    uint32_t height);

/**
 * Take sRGB RGB pixels and preprocess them into image format that can be compared.
 *
 * Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
 *
 * Call `dssim_free_image` to free memory when the image is no longer needed.
 */
DssimImage *dssim_create_image_rgb(struct Dssim *dssim,
                                   const uint8_t *pixels,
                                   uint32_t width,
                                   uint32_t height);

/**
 * Take 16-bit sRGB RGBA pixels (non-premultiplied, alpha last, native-endian,
 * full range 0..65535) and preprocess them into image format that can be compared.
 *
 * This keeps the full 16-bit precision — the gamma-to-linear conversion uses
 * a 65536-entry lookup table, so no precision is lost to 8-bit quantization.
 *
 * Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
 *
 * Call `dssim_free_image` to free memory when the image is no longer needed.
 */
DssimImage *dssim_create_image_rgba16(struct Dssim *dssim,
                                      const uint16_t *pixels,
                                      uint32_t width,
                                      uint32_t height);

/**
 * Take 16-bit sRGB RGB pixels (native-endian, full range 0..65535) and
 * preprocess them into image format that can be compared.
 *
 * Pixels are copied. Returns NULL on error (NULL `pixels`, or zero/overflowing dimensions).
 *
 * Call `dssim_free_image` to free memory when the image is no longer needed.
 */
DssimImage *dssim_create_image_rgb16(struct Dssim *dssim,
                                     const uint16_t *pixels,
                                     uint32_t width,
                                     uint32_t height);

/**
 * Free image data
 */
void dssim_free_image(DssimImage *img);

/**
 * Compare these two images.
 *
 * Returns NaN if either image is NULL or their dimensions don't match.
 * For diagnosable errors and SSIM map output, use `dssim_compare2`.
 *
 * `img1` can be reused for multiple comparisons.
 *
 * Don't forget to free the images and the DSSIM context when done.
 */
double dssim_compare(struct Dssim *dssim, const DssimImage *img1, const DssimImage *img2);

/**
 * Compare two images, with diagnosable errors and optional SSIM (difference) maps.
 *
 * `out_score` — required. Receives the DSSIM score (0 = identical).
 * `out_maps` — optional. If non-NULL, receives a newly allocated set of
 * SSIM maps, one per scale, largest (full resolution) first. Map saving is
 * enabled automatically for this call; the context's own settings are not
 * changed. Pass NULL if you only want the score.
 *
 * Each map pixel is a float SSIM value in [0, 1]: 1.0 means identical,
 * lower values mark more-different regions (i.e. `1 - value` is the
 * per-pixel dissimilarity). See `dssim_ssim_map_data`.
 *
 * Ownership: the returned map set is owned by the caller, does not borrow
 * from the images or the context, and must be freed with a single
 * `dssim_free_ssim_maps` call. Maps and their data pointers obtained from
 * the set stay valid until then.
 *
 * Returns `DSSIM_STATUS_OK` on success. On failure no output is written:
 * `DSSIM_STATUS_NULL_POINTER` if a required pointer is NULL,
 * `DSSIM_STATUS_SIZE_MISMATCH` if the images have different dimensions.
 */
DssimStatus dssim_compare2(struct Dssim *dssim,
                           const DssimImage *img1,
                           const DssimImage *img2,
                           double *out_score,
                           DssimSsimMaps **out_maps);

/**
 * Number of maps in a set returned by `dssim_compare2` (one per scale).
 * Returns 0 for a NULL set.
 */
size_t dssim_ssim_maps_count(const DssimSsimMaps *maps);

/**
 * Get the `index`-th map of a set (largest scale first), or NULL if the set
 * is NULL or the index is out of bounds. The returned pointer borrows from
 * the set: do not free it separately, and do not use it after the set is
 * freed with `dssim_free_ssim_maps`.
 */
const DssimSsimMap *dssim_ssim_maps_get(const DssimSsimMaps *maps, size_t index);

/**
 * Width of an SSIM map in pixels. Returns 0 for a NULL map.
 */
uint32_t dssim_ssim_map_width(const DssimSsimMap *map);

/**
 * Height of an SSIM map in pixels. Returns 0 for a NULL map.
 */
uint32_t dssim_ssim_map_height(const DssimSsimMap *map);

/**
 * Average SSIM of this scale (not DSSIM). Returns NaN for a NULL map.
 */
double dssim_ssim_map_ssim(const DssimSsimMap *map);

/**
 * Pointer to `width * height` tightly packed row-major floats, one SSIM
 * value per pixel in [0, 1] (1.0 = identical; `1 - value` = dissimilarity).
 *
 * The pointer borrows from the map: it stays valid until the map's set is
 * freed with `dssim_free_ssim_maps`. Returns NULL for a NULL map.
 */
const float *dssim_ssim_map_data(const DssimSsimMap *map);

/**
 * Free a set of SSIM maps returned by `dssim_compare2`.
 *
 * Frees the set and all maps in it, invalidating all pointers previously
 * obtained from `dssim_ssim_maps_get` and `dssim_ssim_map_data` for this
 * set. Safe to call with a NULL pointer.
 */
void dssim_free_ssim_maps(DssimSsimMaps *maps);
