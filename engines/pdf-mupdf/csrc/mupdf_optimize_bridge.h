/*
 * MuPDF Document Optimization, Compression, Image Extraction, and Preflight Bridge (`ROADMAP.md` M3.5).
 * Real native execution using MuPDF C API. Conforms to ISO 32000-1 and CODE-001 (<= 600 lines).
 */

#ifndef VOCUMENT_MUPDF_OPTIMIZE_BRIDGE_H
#define VOCUMENT_MUPDF_OPTIMIZE_BRIDGE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

enum vocument_optimization_preset {
    VOCUMENT_OPT_LOSSLESS = 0,
    VOCUMENT_OPT_BALANCED = 1,
    VOCUMENT_OPT_MAXIMUM = 2
};

/*
 * Optimizes PDF streams, deduplicates objects, and optionally linearizes for Fast Web View.
 *
 * Status:
 *   0: success
 *   1: invalid arguments
 *   2: cannot open/parse PDF
 *   3: out of memory
 *   4: blocked by digital signatures (fail-closed)
 */
int vocument_mupdf_optimize_document(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    int preset,
    int do_linearize,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
);

/*
 * Discovers all embedded raster images in the document and returns metadata JSON array:
 * [{"page_index": 0, "image_index": 0, "obj_num": 12, "width": 800, "height": 600,
 *   "format": "png", "colorspace": "DeviceRGB", "bits": 8, "byte_size": 45012}]
 */
int vocument_mupdf_get_document_images_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    char **out_json_str
);

/*
 * Extracts raw or PNG-encoded bytes for a specified page image.
 */
int vocument_mupdf_extract_image_bytes(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    int page_index,
    int image_index,
    char **out_format_str,
    int *out_width,
    int *out_height,
    unsigned char **out_img_bytes,
    size_t *out_img_len
);

/*
 * Performs PDF/X compliance preflight analysis (ISO 15930) and returns JSON report:
 * {"is_valid": true, "version": "PDF/X-1a:2001", "has_output_intent": true,
 *  "output_intent_profile": "Coated FOGRA39", "has_trapped_key": true,
 *  "has_bleed_or_trim_box": true, "has_unembedded_fonts": false,
 *  "has_rgb_content": false, "is_encrypted": false, "issues": []}
 */
int vocument_mupdf_preflight_pdfx_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    char **out_json_str
);

/*
 * Performs print readiness preflight analysis and returns JSON report:
 * {"page_count": 5, "is_uniform_size": true, "width_pt": 612, "height_pt": 792,
 *  "has_color_content": true, "has_bleed_box": true, "has_crop_box": true,
 *  "warnings": []}
 */
int vocument_mupdf_preflight_print_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    char **out_json_str
);

#ifdef __cplusplus
}
#endif

#endif /* VOCUMENT_MUPDF_OPTIMIZE_BRIDGE_H */
