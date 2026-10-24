/*
 * MuPDF Stamp, Watermark, Header/Footer, Bates Numbering, and Bookmark Bridge (`ROADMAP.md` M3.4).
 */

#ifndef VOCUMENT_MUPDF_STAMP_BRIDGE_H
#define VOCUMENT_MUPDF_STAMP_BRIDGE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

enum vocument_bates_position {
    VOCUMENT_BATES_TOP_LEFT = 0,
    VOCUMENT_BATES_TOP_CENTER = 1,
    VOCUMENT_BATES_TOP_RIGHT = 2,
    VOCUMENT_BATES_BOTTOM_LEFT = 3,
    VOCUMENT_BATES_BOTTOM_CENTER = 4,
    VOCUMENT_BATES_BOTTOM_RIGHT = 5
};

/*
 * Applies Bates numbering overlay to selected pages.
 *
 * Status:
 *   0: success
 *   1: invalid arguments or corrupted PDF
 *   2: cannot open or parse PDF
 *   3: out of memory
 *   4: blocked by digital signatures (fail-closed)
 */
int vocument_mupdf_apply_bates_numbering(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const int *page_indices,
    size_t page_indices_len,
    const char *prefix,
    int start_number,
    int digit_padding,
    const char *suffix,
    int position,
    float font_size_pt,
    float margin_pt,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
);

/*
 * Applies multi-zone headers and footers with dynamic macros ({page}, {total}, {date}, {filename}).
 */
int vocument_mupdf_apply_header_footer(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const int *page_indices,
    size_t page_indices_len,
    const char *header_left,
    const char *header_center,
    const char *header_right,
    const char *footer_left,
    const char *footer_center,
    const char *footer_right,
    const char *doc_title,
    const char *date_str,
    float font_size_pt,
    float margin_pt,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
);

/*
 * Reads the bookmark / outlines hierarchy from the document as JSON.
 */
int vocument_mupdf_get_bookmarks_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    char **out_json_str
);

/*
 * Rewrites the document bookmark / outlines hierarchy from JSON.
 */
int vocument_mupdf_set_bookmarks_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const char *bookmarks_json,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
);

/*
 * Applies rubber stamp or watermark overlay to selected pages (`WATER-001`–`003`).
 */
int vocument_mupdf_apply_stamp_or_watermark(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const int *page_indices,
    size_t page_indices_len,
    const char *stamp_text,
    float opacity,
    float rotation_degrees,
    unsigned char r,
    unsigned char g,
    unsigned char b,
    int is_background,
    float rect_x0,
    float rect_y0,
    float rect_x1,
    float rect_y1,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
);

void vocument_mupdf_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif /* VOCUMENT_MUPDF_STAMP_BRIDGE_H */
