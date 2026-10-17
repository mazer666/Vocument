#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <string.h>

typedef struct {
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    int ends_paragraph;
    char text[1024];
} vocument_mupdf_text_line_t;

int vocument_mupdf_query_page_label(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    char *out_label_buf,
    size_t out_label_cap
) {
    if (!out_label_buf || out_label_cap == 0) {
        return 3;
    }
    out_label_buf[0] = '\0';

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
            if (pdf_doc) {
                pdf_page_label(ctx, pdf_doc, page_index, out_label_buf, out_label_cap);
            }
            status = 0;
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
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}

int vocument_mupdf_extract_page_text(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    vocument_mupdf_text_line_t *out_lines,
    size_t max_lines,
    size_t *out_line_count,
    char *out_label_buf,
    size_t out_label_cap,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_line_count) {
        return 3;
    }
    *out_line_count = 0;
    if (out_label_buf && out_label_cap > 0) {
        out_label_buf[0] = '\0';
    }
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    fz_stext_page *stext = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(stext);
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
            if (pdf_doc && out_label_buf && out_label_cap > 0) {
                pdf_page_label(ctx, pdf_doc, page_index, out_label_buf, out_label_cap);
            }

            stext = fz_new_stext_page_from_page_number(ctx, doc, page_index, NULL);
            size_t count_lines = 0;

            for (fz_stext_block *block = stext->first_block; block != NULL; block = block->next) {
                if (block->type != FZ_STEXT_BLOCK_TEXT) {
                    continue;
                }
                for (fz_stext_line *line = block->u.t.first_line; line != NULL; line = line->next) {
                    if (out_lines && count_lines < max_lines) {
                        vocument_mupdf_text_line_t *dst = &out_lines[count_lines];
                        memset(dst, 0, sizeof(vocument_mupdf_text_line_t));
                        dst->x0_milli_pt = (int)(line->bbox.x0 * 1000.0f);
                        dst->y0_milli_pt = (int)(line->bbox.y0 * 1000.0f);
                        dst->x1_milli_pt = (int)(line->bbox.x1 * 1000.0f);
                        dst->y1_milli_pt = (int)(line->bbox.y1 * 1000.0f);
                        dst->ends_paragraph = (line->next == NULL) ? 1 : 0;

                        size_t offset = 0;
                        for (fz_stext_char *ch = line->first_char; ch != NULL; ch = ch->next) {
                            char utf8[8];
                            int n = fz_runetochar(utf8, ch->c);
                            if (n > 0 && offset + (size_t)n < sizeof(dst->text) - 1) {
                                memcpy(dst->text + offset, utf8, (size_t)n);
                                offset += (size_t)n;
                            }
                        }
                        dst->text[offset] = '\0';
                    }
                    count_lines++;
                }
            }

            *out_line_count = count_lines;
            status = 0;
        }
    }
    fz_always(ctx) {
        if (stext) {
            fz_drop_stext_page(ctx, stext);
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

    fz_drop_context(ctx);
    return status;
}
