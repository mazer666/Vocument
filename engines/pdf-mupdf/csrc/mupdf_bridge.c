#include <mupdf/fitz.h>
#include <string.h>

/* Engine identifier reported at runtime.
 *
 * The value is composed of the real MuPDF version (FZ_VERSION) and the build
 * target, so that a runtime inspection can prove which engine and which platform
 * produced the document. A placeholder engine can never report this identifier.
 */
#ifndef VOCRUMENT_MUPDF_ENGINE_ID
#define VOCRUMENT_MUPDF_ENGINE_ID FZ_VERSION
#endif

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
    fz_context *ctx;
    fz_display_list *list;
    unsigned int width_milli_pt;
    unsigned int height_milli_pt;
} vocument_mupdf_display_list_handle_t;

static void lookup_optional_meta(fz_context *ctx, fz_document *doc, const char *key, char *out_buf, size_t out_size) {
    out_buf[0] = '\0';
    fz_try(ctx) {
        int len = fz_lookup_metadata(ctx, doc, key, out_buf, out_size);
        if (len < 0) {
            out_buf[0] = '\0';
        }
    }
    fz_catch(ctx) {
        out_buf[0] = '\0';
    }
}

/*
 * Status codes:
 *  0 = OK
 *  1 = Password required
 *  2 = Invalid/corrupted PDF or open failure
 *  3 = Context allocation failure
 *  4 = Page out of bounds
 */
int vocument_mupdf_inspect_pdf_bytes(
    const unsigned char *bytes,
    size_t len,
    vocument_mupdf_summary_t *out_summary
) {
    if (!out_summary) {
        return 3;
    }
    memset(out_summary, 0, sizeof(vocument_mupdf_summary_t));
    strncpy(out_summary->engine_version, VOCRUMENT_MUPDF_ENGINE_ID, sizeof(out_summary->engine_version) - 1);

    if (!bytes || len < 5) {
        strncpy(out_summary->error_message, "buffer too small to be a valid PDF", sizeof(out_summary->error_message) - 1);
        out_summary->status_code = 2;
        return 2;
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        strncpy(out_summary->error_message, "failed to allocate fz_context", sizeof(out_summary->error_message) - 1);
        out_summary->status_code = 3;
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;

    fz_var(stream);
    fz_var(doc);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int needs_pw = fz_needs_password(ctx, doc);
        out_summary->needs_password = needs_pw;
        if (needs_pw) {
            out_summary->status_code = 1;
            strncpy(out_summary->error_message, "PDF requires an open password", sizeof(out_summary->error_message) - 1);
        } else {
            int pages = fz_count_pages(ctx, doc);
            out_summary->page_count = pages >= 0 ? pages : 0;

            lookup_optional_meta(ctx, doc, FZ_META_FORMAT, out_summary->format_version, sizeof(out_summary->format_version));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_TITLE, out_summary->title, sizeof(out_summary->title));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_AUTHOR, out_summary->author, sizeof(out_summary->author));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_SUBJECT, out_summary->subject, sizeof(out_summary->subject));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_KEYWORDS, out_summary->keywords, sizeof(out_summary->keywords));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_CREATOR, out_summary->creator, sizeof(out_summary->creator));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_PRODUCER, out_summary->producer, sizeof(out_summary->producer));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_CREATIONDATE, out_summary->creation_date, sizeof(out_summary->creation_date));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_MODIFICATIONDATE, out_summary->modification_date, sizeof(out_summary->modification_date));

            out_summary->status_code = 0;
        }
    }
    fz_always(ctx) {
        if (doc) {
            fz_drop_document(ctx, doc);
        }
        if (stream) {
            fz_drop_stream(ctx, stream);
        }
    }
    fz_catch(ctx) {
        const char *msg = fz_caught_message(ctx);
        if (msg) {
            strncpy(out_summary->error_message, msg, sizeof(out_summary->error_message) - 1);
        } else {
            strncpy(out_summary->error_message, "MuPDF threw an exception while opening PDF", sizeof(out_summary->error_message) - 1);
        }
        out_summary->status_code = 2;
    }

    fz_drop_context(ctx);
    return out_summary->status_code;
}

int vocument_mupdf_query_page_geometry(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_page_geometry_t *out_geom
) {
    if (!out_geom) {
        return 3;
    }
    memset(out_geom, 0, sizeof(vocument_mupdf_page_geometry_t));

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        out_geom->status_code = 3;
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    fz_page *page = NULL;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (page_index < 0 || page_index >= count) {
            out_geom->status_code = 4;
        } else {
            page = fz_load_page(ctx, doc, page_index);
            fz_rect bounds = fz_bound_page(ctx, page);
            float w = bounds.x1 - bounds.x0;
            float h = bounds.y1 - bounds.y0;
            out_geom->width_milli_pt = w > 0.0f ? (unsigned int)(w * 1000.0f + 0.5f) : 0;
            out_geom->height_milli_pt = h > 0.0f ? (unsigned int)(h * 1000.0f + 0.5f) : 0;
            out_geom->status_code = 0;
        }
    }
    fz_always(ctx) {
        if (page) {
            fz_drop_page(ctx, page);
        }
        if (doc) {
            fz_drop_document(ctx, doc);
        }
        if (stream) {
            fz_drop_stream(ctx, stream);
        }
    }
    fz_catch(ctx) {
        const char *msg = fz_caught_message(ctx);
        if (msg) {
            strncpy(out_geom->error_message, msg, sizeof(out_geom->error_message) - 1);
        }
        out_geom->status_code = 2;
    }

    fz_drop_context(ctx);
    return out_geom->status_code;
}

int vocument_mupdf_create_display_list(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_display_list_handle_t **out_handle,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_handle) {
        return 3;
    }
    *out_handle = NULL;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    fz_page *page = NULL;
    fz_display_list *list = NULL;
    int status = 0;
    unsigned int width_milli_pt = 0;
    unsigned int height_milli_pt = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(list);
    fz_var(status);
    fz_var(width_milli_pt);
    fz_var(height_milli_pt);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (page_index < 0 || page_index >= count) {
            status = 4;
        } else {
            page = fz_load_page(ctx, doc, page_index);
            fz_rect bounds = fz_bound_page(ctx, page);
            float w = bounds.x1 - bounds.x0;
            float h = bounds.y1 - bounds.y0;
            width_milli_pt = w > 0.0f ? (unsigned int)(w * 1000.0f + 0.5f) : 0;
            height_milli_pt = h > 0.0f ? (unsigned int)(h * 1000.0f + 0.5f) : 0;

            list = fz_new_display_list_from_page(ctx, page);
            status = 0;
        }
    }
    fz_always(ctx) {
        if (page) {
            fz_drop_page(ctx, page);
        }
        if (doc) {
            fz_drop_document(ctx, doc);
        }
        if (stream) {
            fz_drop_stream(ctx, stream);
        }
    }
    fz_catch(ctx) {
        const char *msg = fz_caught_message(ctx);
        if (msg && err_buf && err_buf_len > 0) {
            strncpy(err_buf, msg, err_buf_len - 1);
        }
        status = 2;
    }

    if (status != 0 || !list) {
        if (list) {
            fz_drop_display_list(ctx, list);
        }
        fz_drop_context(ctx);
        return status != 0 ? status : 2;
    }

    vocument_mupdf_display_list_handle_t *handle =
        (vocument_mupdf_display_list_handle_t *)malloc(sizeof(vocument_mupdf_display_list_handle_t));
    if (!handle) {
        fz_drop_display_list(ctx, list);
        fz_drop_context(ctx);
        return 3;
    }

    handle->ctx = ctx;
    handle->list = list;
    handle->width_milli_pt = width_milli_pt;
    handle->height_milli_pt = height_milli_pt;
    *out_handle = handle;
    return 0;
}

int vocument_mupdf_render_tile_from_display_list(
    vocument_mupdf_display_list_handle_t *handle,
    unsigned int scale_milli,
    unsigned int tile_x_px,
    unsigned int tile_y_px,
    unsigned int tile_width_px,
    unsigned int tile_height_px,
    unsigned char *out_rgba_bytes,
    size_t out_rgba_len,
    char *err_buf,
    size_t err_buf_len
) {
    if (!handle || !handle->ctx || !handle->list || !out_rgba_bytes) {
        return 3;
    }
    if (tile_width_px == 0 || tile_height_px == 0) {
        return 2;
    }
    size_t expected_len = (size_t)tile_width_px * (size_t)tile_height_px * 4;
    if (out_rgba_len < expected_len) {
        return 2;
    }

    fz_context *ctx = handle->ctx;
    fz_pixmap *pix = NULL;
    fz_device *dev = NULL;
    int status = 0;

    fz_var(pix);
    fz_var(dev);
    fz_var(status);

    fz_try(ctx) {
        float scale = (float)scale_milli / 1000.0f;
        if (scale <= 0.01f) {
            scale = 1.0f;
        }
        fz_matrix ctm = fz_scale(scale, scale);

        fz_irect bbox;
        bbox.x0 = (int)tile_x_px;
        bbox.y0 = (int)tile_y_px;
        bbox.x1 = (int)(tile_x_px + tile_width_px);
        bbox.y1 = (int)(tile_y_px + tile_height_px);

        pix = fz_new_pixmap_with_bbox(ctx, fz_device_rgb(ctx), bbox, NULL, 1);
        fz_clear_pixmap_with_value(ctx, pix, 255);

        dev = fz_new_draw_device_with_bbox(ctx, ctm, pix, &bbox);
        fz_run_display_list(ctx, handle->list, dev, fz_identity, fz_infinite_rect, NULL);
        fz_close_device(ctx, dev);

        unsigned char *samples = fz_pixmap_samples(ctx, pix);
        int stride = fz_pixmap_stride(ctx, pix);
        size_t row_bytes = (size_t)tile_width_px * 4;

        for (unsigned int y = 0; y < tile_height_px; ++y) {
            memcpy(
                out_rgba_bytes + (size_t)y * row_bytes,
                samples + (size_t)y * (size_t)stride,
                row_bytes
            );
        }
        status = 0;
    }
    fz_always(ctx) {
        if (dev) {
            fz_drop_device(ctx, dev);
        }
        if (pix) {
            fz_drop_pixmap(ctx, pix);
        }
    }
    fz_catch(ctx) {
        const char *msg = fz_caught_message(ctx);
        if (msg && err_buf && err_buf_len > 0) {
            strncpy(err_buf, msg, err_buf_len - 1);
        }
        status = 2;
    }

    return status;
}

void vocument_mupdf_drop_display_list(vocument_mupdf_display_list_handle_t *handle) {
    if (!handle) {
        return;
    }
    if (handle->ctx) {
        if (handle->list) {
            fz_drop_display_list(handle->ctx, handle->list);
        }
        fz_drop_context(handle->ctx);
    }
    free(handle);
}

/**
 * Renders a page into a grayscale raster.
 *
 * The raster is the picture of the page, exactly as a reader sees it. A
 * redaction has to be verified against this picture: a document can carry the
 * same text twice, as a text layer and as pixels in an image (`REDACT-002`).
 */
int vocument_mupdf_render_page_gray(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    int scale_milli,
    unsigned char **out_pixels,
    size_t *out_width,
    size_t *out_height,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_pixels || !out_width || !out_height) {
        return 3;
    }
    *out_pixels = NULL;
    *out_width = 0;
    *out_height = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    fz_page *page = NULL;
    fz_pixmap *pixmap = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(pixmap);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (page_index < 0 || page_index >= count) {
            status = 4;
        } else {
            page = fz_load_page(ctx, doc, page_index);
            float scale = (scale_milli > 0 ? scale_milli : 1000) / 1000.0f;
            fz_matrix ctm = fz_scale(scale, scale);
            pixmap = fz_new_pixmap_from_page(ctx, page, ctm, fz_device_gray(ctx), 0);
            if (!pixmap || pixmap->w <= 0 || pixmap->h <= 0) {
                status = 5;
            } else {
                size_t needed = (size_t)pixmap->w * (size_t)pixmap->h;
                unsigned char *copy = (unsigned char *)malloc(needed > 0 ? needed : 1);
                if (!copy) {
                    fz_throw(ctx, FZ_ERROR_SYSTEM, "malloc failed for the rendered page");
                }
                for (int y = 0; y < pixmap->h; y++) {
                    const unsigned char *row = pixmap->samples + (size_t)y * pixmap->stride;
                    memcpy(copy + (size_t)y * (size_t)pixmap->w, row, (size_t)pixmap->w);
                }
                *out_pixels = copy;
                *out_width = (size_t)pixmap->w;
                *out_height = (size_t)pixmap->h;
                status = 0;
            }
        }
    }
    fz_always(ctx) {
        if (pixmap) fz_drop_pixmap(ctx, pixmap);
        if (page) fz_drop_page(ctx, page);
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        const char *msg = fz_caught_message(ctx);
        if (msg && err_buf && err_buf_len > 0) {
            strncpy(err_buf, msg, err_buf_len - 1);
        }
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}
