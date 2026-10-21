/* The comment of the reviewer on a page (`ANNOT-001`).
 *
 * A comment is a standard PDF `/Text` annotation with a popup: the note
 * stands on the page at the place the reviewer marked, the text of the note
 * opens in the popup. It is a part of the document — it is saved with it, and
 * a tool of another vendor can read it.
 *
 * The colour of the note is written as the colour of the annotation. The
 * *meaning* of the colour lives in the core (the rail of the profession), not
 * here: the engine knows no rule of the profession, it only writes and reads
 * what it is told.
 *
 * The identifiers of the notes stand in the PDF `/NM` key, the author in `/T`,
 * the text in `/Contents`, the time of the change in `/M`. A note without an
 * identifier cannot be found again, and a note without a text is not a note —
 * both are refused before the document is touched. */

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mupdf_serialize.h"

/* One comment of the reviewer. */
typedef struct {
    int page_index;
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    unsigned char color_r;
    unsigned char color_g;
    unsigned char color_b;
    char annotation_id[128];
    char author[256];
    char contents[2048];
} vocument_mupdf_comment_t;

/* The size of the popup of a note. The popup opens next to the note; its
   size does not depend on the text, because the viewer grows it. */
#define VOCUMENT_COMMENT_POPUP_WIDTH 220.0f
#define VOCUMENT_COMMENT_POPUP_HEIGHT 120.0f

/* Reads the note out of the annotation. */
static void extract_comment_struct(
    fz_context *ctx,
    int page_index,
    pdf_annot *annot,
    vocument_mupdf_comment_t *dst
) {
    memset(dst, 0, sizeof(vocument_mupdf_comment_t));
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

/* Answers whether the note carries something other than blanks. A note of
   blanks is not a note: the user would not see anything. */
static int vocument_comment_has_text(const char *contents) {
    if (!contents) {
        return 0;
    }
    for (const char *c = contents; *c != '\0'; c++) {
        if (*c != ' ' && *c != '\t' && *c != '\r' && *c != '\n') {
            return 1;
        }
    }
    return 0;
}

/* Answers whether the annotation is a comment of the reviewer. */
static int is_comment_annot(fz_context *ctx, pdf_annot *annot) {
    enum pdf_annot_type type = pdf_annot_type(ctx, annot);
    return type == PDF_ANNOT_TEXT || type == PDF_ANNOT_FREE_TEXT;
}

/* Lists the comments of a page. */
int vocument_mupdf_query_page_comments(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_comment_t *out_annots,
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
                if (is_comment_annot(ctx, annot)) {
                    if (out_annots && found < max_annots) {
                        extract_comment_struct(ctx, page_index, annot, &out_annots[found]);
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

/* Writes a comment onto a page. */
int vocument_mupdf_add_comment_annot(
    const unsigned char *bytes,
    size_t len,
    const vocument_mupdf_comment_t *spec,
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
        } else if (!vocument_comment_has_text(spec->contents)) {
            /* A note without a text is not a note. */
            status = 5;
        } else {
            pdf_document *pdf_doc = pdf_specifics(ctx, doc);
            page = pdf_load_page(ctx, pdf_doc, spec->page_index);
            annot = pdf_create_annot(ctx, page, PDF_ANNOT_TEXT);

            fz_rect r = fz_make_rect(
                (float)spec->x0_milli_pt / 1000.0f,
                (float)spec->y0_milli_pt / 1000.0f,
                (float)spec->x1_milli_pt / 1000.0f,
                (float)spec->y1_milli_pt / 1000.0f
            );
            pdf_set_annot_rect(ctx, annot, r);

            /* The popup opens next to the note. */
            pdf_set_annot_popup(
                ctx,
                annot,
                fz_make_rect(
                    r.x1 + 8.0f,
                    r.y0,
                    r.x1 + 8.0f + VOCUMENT_COMMENT_POPUP_WIDTH,
                    r.y0 + VOCUMENT_COMMENT_POPUP_HEIGHT
                )
            );

            float rgb[3] = {
                (float)spec->color_r / 255.0f,
                (float)spec->color_g / 255.0f,
                (float)spec->color_b / 255.0f
            };
            pdf_set_annot_color(ctx, annot, 3, rgb);
            /* A text annotation has no border of its own — MuPDF refuses to
               set one ("Text annotations have no BS property"). */

            if (spec->annotation_id[0] != '\0') {
                pdf_set_annot_name(ctx, annot, spec->annotation_id);
            }
            if (spec->author[0] != '\0') {
                pdf_set_annot_author(ctx, annot, spec->author);
            }
            pdf_set_annot_contents(ctx, annot, spec->contents);

            /* The time of the change belongs to the note: a reviewer has to
               see when the note was written. */
            pdf_set_annot_creation_date(ctx, annot, (int64_t)time(NULL));
            pdf_set_annot_modification_date(ctx, annot, (int64_t)time(NULL));

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

/* Removes a comment from a page and hands the removed note back. */
int vocument_mupdf_remove_comment_annot(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    const char *annotation_id,
    vocument_mupdf_comment_t *out_removed,
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
    memset(out_removed, 0, sizeof(vocument_mupdf_comment_t));
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
    pdf_annot *target = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(page);
    fz_var(target);
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
            for (pdf_annot *annot = pdf_first_annot(ctx, page); annot != NULL; annot = pdf_next_annot(ctx, annot)) {
                if (!is_comment_annot(ctx, annot)) {
                    continue;
                }
                const char *nm = pdf_annot_name(ctx, annot);
                if (nm && strcmp(nm, annotation_id) == 0) {
                    extract_comment_struct(ctx, page_index, annot, out_removed);
                    target = annot;
                    break;
                }
            }
            if (!target) {
                status = 6;
            } else {
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
