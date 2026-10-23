#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_serialize.h"
#include "mupdf_page_helpers.h"

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

static void extract_highlight_struct(
    fz_context *ctx,
    int page_index,
    pdf_annot *annot,
    vocument_mupdf_highlight_t *dst
) {
    memset(dst, 0, sizeof(vocument_mupdf_highlight_t));
    dst->page_index = page_index;

    fz_rect r = pdf_bound_annot(ctx, annot);
    dst->x0_milli_pt = (int)(r.x0 * 1000.0f);
    dst->y0_milli_pt = (int)(r.y0 * 1000.0f);
    dst->x1_milli_pt = (int)(r.x1 * 1000.0f);
    dst->y1_milli_pt = (int)(r.y1 * 1000.0f);

    int n = 0;
    float color[4] = {1.0f, 0.9f, 0.3f, 0.0f};
    pdf_annot_color(ctx, annot, &n, color);
    if (n >= 3) {
        dst->color_r = (unsigned char)(color[0] * 255.0f + 0.5f);
        dst->color_g = (unsigned char)(color[1] * 255.0f + 0.5f);
        dst->color_b = (unsigned char)(color[2] * 255.0f + 0.5f);
    } else if (n == 1) {
        unsigned char gray = (unsigned char)(color[0] * 255.0f + 0.5f);
        dst->color_r = gray;
        dst->color_g = gray;
        dst->color_b = gray;
    } else {
        dst->color_r = 255;
        dst->color_g = 230;
        dst->color_b = 80;
    }

    float opacity = pdf_annot_opacity(ctx, annot);
    if (opacity < 0.0f) opacity = 0.0f;
    if (opacity > 1.0f) opacity = 1.0f;
    dst->opacity_milli = (unsigned int)(opacity * 1000.0f + 0.5f);

    const char *nm = pdf_annot_name(ctx, annot);
    if (nm) {
        strncpy(dst->annotation_id, nm, sizeof(dst->annotation_id) - 1);
    }
    const char *author = pdf_annot_author(ctx, annot);
    if (author) {
        strncpy(dst->author, author, sizeof(dst->author) - 1);
    }
    const char *contents = pdf_annot_contents(ctx, annot);
    if (contents) {
        strncpy(dst->contents, contents, sizeof(dst->contents) - 1);
    }
}

int vocument_mupdf_query_page_rotation(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    int *out_rotation_deg
) {
    if (!out_rotation_deg) {
        return 3;
    }
    *out_rotation_deg = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (page_index < 0 || page_index >= count) {
            status = 4;
        } else {
            pdf_document *pdf_doc = pdf_specifics(ctx, doc);
            if (!pdf_doc) {
                status = 2;
            } else {
                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, page_index);
                pdf_obj *rot_obj = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(Rotate));
                int deg = rot_obj ? pdf_to_int(ctx, rot_obj) : 0;
                deg = ((deg % 360) + 360) % 360;
                *out_rotation_deg = deg;
                status = 0;
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}

int vocument_mupdf_rotate_pages_ext(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    int delta_degrees,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *err_buf,
    size_t err_buf_len
) {
    if (!bytes || len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 3;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            for (size_t i = 0; i < page_indices_len; i++) {
                int idx = page_indices[i];
                if (idx < 0 || idx >= count) {
                    status = 1;
                    break;
                }
            }

            if (status == 0) {
                int sigs = count_active_signatures(ctx, pdf_doc);
                if (sigs != 0 && !allow_signature_invalidation) {
                    status = 4; /* Blocked by digital signatures fail-closed */
                } else if (sigs < 0) {
                    status = 1; /* Cannot safely inspect cyclic/broken structure */
                } else if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                    status = 1; /* Corrupted Tagged PDF structure fail-closed */
                } else {
                    for (size_t i = 0; i < page_indices_len; i++) {
                        int idx = page_indices[i];
                        pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, idx);
                        make_page_self_contained(ctx, page_obj);
                        pdf_obj *rot_obj = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(Rotate));
                        int cur_deg = rot_obj ? pdf_to_int(ctx, rot_obj) : 0;
                        int new_deg = ((cur_deg + delta_degrees) % 360 + 360) % 360;
                        pdf_dict_put_int(ctx, page_obj, PDF_NAME(Rotate), (int64_t)new_deg);
                    }
                    if (sigs > 0 && allow_signature_invalidation) {
                        mark_signatures_invalidated(ctx, pdf_doc);
                    }
                    vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                }
            }
        }
    }
    fz_always(ctx) {
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

int vocument_mupdf_rotate_pages(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    int delta_degrees,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *err_buf,
    size_t err_buf_len
) {
    return vocument_mupdf_rotate_pages_ext(
        bytes,
        len,
        page_indices,
        page_indices_len,
        delta_degrees,
        0,
        out_pdf_bytes,
        out_pdf_len,
        err_buf,
        err_buf_len
    );
}

int vocument_mupdf_query_page_highlights(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_highlight_t *out_annots,
    size_t max_annots,
    size_t *out_annot_count,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_annot_count) {
        return 3;
    }
    *out_annot_count = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_page *page = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (page_index < 0 || page_index >= count) {
            status = 4;
        } else {
            pdf_document *pdf_doc = pdf_specifics(ctx, doc);
            page = pdf_load_page(ctx, pdf_doc, page_index);
            size_t found = 0;
            for (pdf_annot *annot = pdf_first_annot(ctx, page); annot != NULL; annot = pdf_next_annot(ctx, annot)) {
                if (pdf_annot_type(ctx, annot) == PDF_ANNOT_HIGHLIGHT) {
                    if (out_annots && found < max_annots) {
                        extract_highlight_struct(ctx, page_index, annot, &out_annots[found]);
                    }
                    found++;
                }
            }
            *out_annot_count = found;
            status = 0;
        }
    }
    fz_always(ctx) {
        if (page) pdf_drop_page(ctx, page);
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

int vocument_mupdf_add_highlight_annot(
    const unsigned char *bytes,
    size_t len,
    const vocument_mupdf_highlight_t *spec,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *err_buf,
    size_t err_buf_len
) {
    if (!spec || !out_pdf_bytes || !out_pdf_len) {
        return 3;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_page *page = NULL;
    pdf_annot *annot = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(annot);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (spec->page_index < 0 || spec->page_index >= count) {
            status = 4;
        } else {
            pdf_document *pdf_doc = pdf_specifics(ctx, doc);
            page = pdf_load_page(ctx, pdf_doc, spec->page_index);
            annot = pdf_create_annot(ctx, page, PDF_ANNOT_HIGHLIGHT);

            fz_rect r = fz_make_rect(
                (float)spec->x0_milli_pt / 1000.0f,
                (float)spec->y0_milli_pt / 1000.0f,
                (float)spec->x1_milli_pt / 1000.0f,
                (float)spec->y1_milli_pt / 1000.0f
            );
            fz_quad q = fz_quad_from_rect(r);
            pdf_set_annot_quad_points(ctx, annot, 1, &q);

            float rgb[3] = {
                (float)spec->color_r / 255.0f,
                (float)spec->color_g / 255.0f,
                (float)spec->color_b / 255.0f
            };
            pdf_set_annot_color(ctx, annot, 3, rgb);
            pdf_set_annot_opacity(ctx, annot, (float)spec->opacity_milli / 1000.0f);

            if (spec->annotation_id[0] != '\0') {
                pdf_set_annot_name(ctx, annot, spec->annotation_id);
            }
            if (spec->author[0] != '\0') {
                pdf_set_annot_author(ctx, annot, spec->author);
            }
            if (spec->contents[0] != '\0') {
                pdf_set_annot_contents(ctx, annot, spec->contents);
            }

            pdf_update_annot(ctx, annot);
            pdf_update_page(ctx, page);
            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
            status = 0;
        }
    }
    fz_always(ctx) {
        if (annot) pdf_drop_annot(ctx, annot);
        if (page) pdf_drop_page(ctx, page);
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

int vocument_mupdf_remove_highlight_annot(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    const char *annotation_id,
    vocument_mupdf_highlight_t *out_removed,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *err_buf,
    size_t err_buf_len
) {
    if (!annotation_id || !out_removed || !out_pdf_bytes || !out_pdf_len) {
        return 3;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_page *page = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (page_index < 0 || page_index >= count) {
            status = 4;
        } else {
            pdf_document *pdf_doc = pdf_specifics(ctx, doc);
            page = pdf_load_page(ctx, pdf_doc, page_index);
            pdf_annot *target = NULL;
            for (pdf_annot *annot = pdf_first_annot(ctx, page); annot != NULL; annot = pdf_next_annot(ctx, annot)) {
                if (pdf_annot_type(ctx, annot) == PDF_ANNOT_HIGHLIGHT) {
                    const char *nm = pdf_annot_name(ctx, annot);
                    if (nm && strcmp(nm, annotation_id) == 0) {
                        target = annot;
                        break;
                    }
                }
            }
            if (!target) {
                status = 5;
            } else {
                extract_highlight_struct(ctx, page_index, target, out_removed);
                pdf_delete_annot(ctx, page, target);
                pdf_update_page(ctx, page);
                vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                status = 0;
            }
        }
    }
    fz_always(ctx) {
        if (page) pdf_drop_page(ctx, page);
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

typedef struct {
    int page_index;
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
} vocument_mupdf_rect_t;

/**
 * Removes the content of the given regions of a page for good.
 *
 * The text, the images and the line art under the region are removed from
 * the content of the page, not covered by a black rectangle. A rectangle
 * would only hide the content, it would leave it readable in the file.
 */
int vocument_mupdf_redact_page_regions(
    const unsigned char *bytes,
    size_t len,
    const vocument_mupdf_rect_t *rects,
    size_t rects_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    char *err_buf,
    size_t err_buf_len
) {
    if (!rects || !out_pdf_bytes || !out_pdf_len) {
        return 3;
    }
    if (rects_len == 0) {
        return 5;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_page *page = NULL;
    pdf_annot *annot = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(annot);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int count = fz_count_pages(ctx, doc);
        if (rects[0].page_index < 0 || rects[0].page_index >= count) {
            status = 4;
        } else {
            pdf_document *pdf_doc = pdf_specifics(ctx, doc);
            page = pdf_load_page(ctx, pdf_doc, rects[0].page_index);

            pdf_redact_options opts;
            memset(&opts, 0, sizeof(opts));
            /* No black box over the removed region. The box would hide
               content that survived the removal, and the verification has to
               see the truth: the picture of the page is rendered and read,
               and a covered region would look empty even when the pixels
               under it are still there. The removed region is blank in the
               image data and empty in the text layer — the picture of the
               page shows what is really left. */
            opts.black_boxes = 0;
            /* The pixels of the image that lie in the area are replaced.
               The image itself survives, so the page stays readable, and the
               original pixels are gone — they are not covered by a drawn
               rectangle but replaced in the image data. Removing the whole
               image would destroy the page, keeping it unchanged would leave
               the content readable. */
            opts.image_method = PDF_REDACT_IMAGE_PIXELS;
            /* Line art that is touched by the area is removed. */
            opts.line_art = PDF_REDACT_LINE_ART_REMOVE_IF_TOUCHED;
            /* The text is removed — not hidden. This is the secure option;
               PDF_REDACT_TEXT_NONE would leave the text in the file. */
            opts.text = PDF_REDACT_TEXT_REMOVE;

            for (size_t i = 0; i < rects_len; i++) {
                if (rects[i].page_index != rects[0].page_index) {
                    status = 6;
                    continue;
                }
                fz_rect r = fz_make_rect(
                    (float)rects[i].x0_milli_pt / 1000.0f,
                    (float)rects[i].y0_milli_pt / 1000.0f,
                    (float)rects[i].x1_milli_pt / 1000.0f,
                    (float)rects[i].y1_milli_pt / 1000.0f
                );
                if (fz_is_empty_rect(r)) {
                    status = 7;
                    continue;
                }
                annot = pdf_create_annot(ctx, page, PDF_ANNOT_REDACT);
                pdf_set_annot_rect(ctx, annot, r);
                pdf_apply_redaction(ctx, annot, &opts);
                pdf_drop_annot(ctx, annot);
                annot = NULL;
            }

            if (status == 0) {
                pdf_update_page(ctx, page);
                vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
            }
        }
    }
    fz_always(ctx) {
        if (annot) pdf_drop_annot(ctx, annot);
        if (page) pdf_drop_page(ctx, page);
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
