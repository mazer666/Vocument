/*
 * Android placeholder MuPDF bridge.
 *
 * IMPORTANT — this is NOT a MuPDF implementation.
 *
 * The Android ARM64 target currently links this placeholder instead of a real MuPDF
 * build, because cross-compiling MuPDF for `aarch64-linux-android` is not yet part of
 * the build (`ROADMAP.md` M1.5 / `docs/project/RISKS.md`).
 *
 * Every function below returns deterministic placeholder data instead of parsing or
 * rendering a real PDF. Callers can detect this state via the engine identifier
 * `mupdf-stub-1.28`, which is deliberately distinct from the real engine identifier
 * `mupdf-<version>`.
 *
 * Per `QA-003` / `CR-0002`, functionality that is only a placeholder must not be
 * presented as accepted functionality. `scripts/validate-repo.sh` enforces that this
 * placeholder is explicitly acknowledged in the project documentation.
 */

#include <stdlib.h>
#include <string.h>

typedef struct {
    int status_code;
    int page_count;
    int needs_password;
    char format_version[64];
    char title[512];
    char author[512];
    char subject[512];
    char keywords[512];
    char creator[512];
    char producer[512];
    char creation_date[128];
    char modification_date[128];
    char error_message[512];
    char engine_version[64];
} vocument_mupdf_summary_t;

typedef struct {
    int status_code;
    unsigned int width_milli_pt;
    unsigned int height_milli_pt;
    char error_message[512];
} vocument_mupdf_page_geometry_t;

typedef struct {
    void *dummy;
    unsigned int width_milli_pt;
    unsigned int height_milli_pt;
} vocument_mupdf_display_list_handle_t;

typedef struct {
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    int ends_paragraph;
    char text[1024];
} vocument_mupdf_text_line_t;

typedef struct {
    int page_index;
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    unsigned char color_r;
    unsigned char color_g;
    unsigned char color_b;
    unsigned int opacity_milli;
    char annotation_id[128];
    char author[256];
    char contents[1024];
} vocument_mupdf_highlight_t;

void vocument_mupdf_free_bytes(unsigned char *ptr) {
    if (ptr) {
        free(ptr);
    }
}

int vocument_mupdf_inspect_pdf_bytes(
    const unsigned char *bytes,
    size_t len,
    vocument_mupdf_summary_t *out_summary
) {
    if (!out_summary) return 3;
    memset(out_summary, 0, sizeof(vocument_mupdf_summary_t));
    strncpy(out_summary->engine_version, "mupdf-stub-1.28", sizeof(out_summary->engine_version) - 1);
    if (!bytes || len < 5) {
        strncpy(out_summary->error_message, "buffer too small", sizeof(out_summary->error_message) - 1);
        out_summary->status_code = 2;
        return 2;
    }
    out_summary->page_count = 2;
    strncpy(out_summary->format_version, "PDF-1.7", sizeof(out_summary->format_version) - 1);
    strncpy(out_summary->title, "Vocument Document", sizeof(out_summary->title) - 1);
    out_summary->status_code = 0;
    return 0;
}

int vocument_mupdf_query_page_geometry(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_page_geometry_t *out_geom
) {
    (void)bytes;
    (void)len;
    (void)page_index;
    if (!out_geom) return 3;
    memset(out_geom, 0, sizeof(vocument_mupdf_page_geometry_t));
    out_geom->width_milli_pt = 595000;
    out_geom->height_milli_pt = 842000;
    out_geom->status_code = 0;
    return 0;
}

int vocument_mupdf_create_display_list(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_display_list_handle_t **out_handle,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)bytes;
    (void)len;
    (void)page_index;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_handle) return 3;
    vocument_mupdf_display_list_handle_t *h = (vocument_mupdf_display_list_handle_t *)malloc(sizeof(vocument_mupdf_display_list_handle_t));
    if (!h) return 3;
    h->dummy = NULL;
    h->width_milli_pt = 595000;
    h->height_milli_pt = 842000;
    *out_handle = h;
    return 0;
}

int vocument_mupdf_render_tile_from_display_list(
    vocument_mupdf_display_list_handle_t *handle,
    unsigned int scale_milli,
    int tile_x_px,
    int tile_y_px,
    int tile_width_px,
    int tile_height_px,
    unsigned char **out_rgba_bytes,
    size_t *out_rgba_len,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)handle;
    (void)scale_milli;
    (void)tile_x_px;
    (void)tile_y_px;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_rgba_bytes || !out_rgba_len) return 3;
    size_t total = (size_t)tile_width_px * (size_t)tile_height_px * 4;
    unsigned char *buf = (unsigned char *)malloc(total > 0 ? total : 1);
    if (!buf) return 3;
    memset(buf, 255, total);
    *out_rgba_bytes = buf;
    *out_rgba_len = total;
    return 0;
}

void vocument_mupdf_drop_display_list(vocument_mupdf_display_list_handle_t *handle) {
    if (handle) {
        free(handle);
    }
}

int vocument_mupdf_query_page_label(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    char *out_label_buf,
    size_t out_label_cap
) {
    (void)bytes;
    (void)len;
    if (!out_label_buf || out_label_cap == 0) return 3;
    snprintf(out_label_buf, out_label_cap, "%d", page_index + 1);
    return 0;
}

int vocument_mupdf_extract_page_text(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_text_line_t **out_lines,
    size_t *out_line_count,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)bytes;
    (void)len;
    (void)page_index;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_lines || !out_line_count) return 3;
    *out_lines = NULL;
    *out_line_count = 0;
    return 0;
}

int vocument_mupdf_query_page_rotation(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    unsigned int *out_rotation_degrees,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)bytes;
    (void)len;
    (void)page_index;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_rotation_degrees) return 3;
    *out_rotation_degrees = 0;
    return 0;
}

int vocument_mupdf_rotate_pages(
    const unsigned char *in_pdf_bytes,
    size_t in_pdf_len,
    const int *page_indices,
    size_t page_count,
    int delta_degrees,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)page_indices;
    (void)page_count;
    (void)delta_degrees;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_pdf_bytes || !out_pdf_len) return 3;
    unsigned char *dup = (unsigned char *)malloc(in_pdf_len > 0 ? in_pdf_len : 1);
    if (!dup) return 3;
    if (in_pdf_len > 0 && in_pdf_bytes) memcpy(dup, in_pdf_bytes, in_pdf_len);
    *out_pdf_bytes = dup;
    *out_pdf_len = in_pdf_len;
    return 0;
}

int vocument_mupdf_query_page_highlights(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_highlight_t **out_annots,
    size_t *out_count,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)bytes;
    (void)len;
    (void)page_index;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_annots || !out_count) return 3;
    *out_annots = NULL;
    *out_count = 0;
    return 0;
}

int vocument_mupdf_add_highlight_annot(
    const unsigned char *in_pdf_bytes,
    size_t in_pdf_len,
    const vocument_mupdf_highlight_t *highlight,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)highlight;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_pdf_bytes || !out_pdf_len) return 3;
    unsigned char *dup = (unsigned char *)malloc(in_pdf_len > 0 ? in_pdf_len : 1);
    if (!dup) return 3;
    if (in_pdf_len > 0 && in_pdf_bytes) memcpy(dup, in_pdf_bytes, in_pdf_len);
    *out_pdf_bytes = dup;
    *out_pdf_len = in_pdf_len;
    return 0;
}

int vocument_mupdf_remove_highlight_annot(
    const unsigned char *in_pdf_bytes,
    size_t in_pdf_len,
    int page_index,
    const char *annotation_id,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *out_error_buf,
    size_t out_error_cap
) {
    (void)page_index;
    (void)annotation_id;
    (void)out_error_buf;
    (void)out_error_cap;
    if (!out_pdf_bytes || !out_pdf_len) return 3;
    unsigned char *dup = (unsigned char *)malloc(in_pdf_len > 0 ? in_pdf_len : 1);
    if (!dup) return 3;
    if (in_pdf_len > 0 && in_pdf_bytes) memcpy(dup, in_pdf_bytes, in_pdf_len);
    *out_pdf_bytes = dup;
    *out_pdf_len = in_pdf_len;
    return 0;
}
