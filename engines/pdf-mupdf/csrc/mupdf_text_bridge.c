#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <math.h>
#include <string.h>

/* Grows a box so that it contains the given point. */
static void vocument_grow_box(float *x0, float *y0, float *x1, float *y1, fz_point p)
{
    if (p.x < *x0) *x0 = p.x;
    if (p.x > *x1) *x1 = p.x;
    if (p.y < *y0) *y0 = p.y;
    if (p.y > *y1) *y1 = p.y;
}

/* Grows a box so that it contains the whole character.
   The four corners of a character are not ordered upper-left, upper-right,
   lower-left, lower-right on a rotated page: on a page that is turned by 90
   degrees the "upper" corner lies on the right. Only the smallest and the
   largest value of each axis are therefore well defined, and the box of a
   word is the bounding box of all of its characters. A box that is built
   from the upper-left and the lower-right corner alone becomes inverted on
   such a page — and an inverted box matches nothing when a region is
   redacted, so the text survives the redaction. */
static void vocument_grow_box_for_character(float *x0, float *y0, float *x1, float *y1, const fz_quad *quad)
{
    vocument_grow_box(x0, y0, x1, y1, quad->ul);
    vocument_grow_box(x0, y0, x1, y1, quad->ur);
    vocument_grow_box(x0, y0, x1, y1, quad->ll);
    vocument_grow_box(x0, y0, x1, y1, quad->lr);
}

typedef struct {
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    int ends_paragraph;
    char text[1024];
} vocument_mupdf_text_line_t;

/* A word of a line with its own bounding box. The word boxes are the
   ground truth of a redaction: only the words that lie in the redacted
   region have to disappear from the document. */
typedef struct {
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    char text[128];
} vocument_mupdf_text_word_t;

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
    vocument_mupdf_text_word_t *out_words,
    size_t max_words,
    size_t *out_word_count,
    char *out_label_buf,
    size_t out_label_cap,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_line_count) {
        return 3;
    }
    *out_line_count = 0;
    if (out_word_count) {
        *out_word_count = 0;
    }
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
            size_t count_words = 0;

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

                        /* The words of the line. A word ends at a space or
                           at a visible gap between two characters. The box
                           of the word is the union of the boxes of its
                           characters. */
                        if (out_words && count_words < max_words) {
                            char word_text[128];
                            size_t word_offset = 0;
                            int word_started = 0;
                            float word_x0 = 0.0f, word_y0 = 0.0f;
                            float word_x1 = 0.0f, word_y1 = 0.0f;
                            fz_stext_char *previous = NULL;

                            for (fz_stext_char *ch = line->first_char; ch != NULL; ch = ch->next) {
                                int is_space = (ch->c == ' ' || ch->c == '\t' || ch->c == 0xA0);
                                int gap = 0;
                                if (previous && !is_space && !word_started) {
                                    /* A character that starts far away from the
                                       previous one begins a new word. The
                                       distance is measured along the reading
                                       direction of the line: on a page that is
                                       turned by 90 degrees the text runs
                                       downwards, and a gap that is only
                                       measured along the x axis would be
                                       invisible there. */
                                    float dir_x = ch->quad.lr.x - ch->quad.ul.x;
                                    float dir_y = ch->quad.lr.y - ch->quad.ul.y;
                                    float dir_len = sqrtf(dir_x * dir_x + dir_y * dir_y);
                                    if (dir_len > 0.0001f) {
                                        dir_x /= dir_len;
                                        dir_y /= dir_len;
                                    }
                                    float advance = (ch->quad.ul.x - previous->quad.ul.x) * dir_x
                                                 + (ch->quad.ul.y - previous->quad.ul.y) * dir_y;
                                    if (advance > 0.25f * (dir_len + 0.001f)) {
                                        gap = 1;
                                    }
                                }

                                if (is_space || gap) {
                                    if (word_started) {
                                        vocument_mupdf_text_word_t *word = &out_words[count_words++];
                                        memset(word, 0, sizeof(*word));
                                        word->x0_milli_pt = (int)(word_x0 * 1000.0f);
                                        word->y0_milli_pt = (int)(word_y0 * 1000.0f);
                                        word->x1_milli_pt = (int)(word_x1 * 1000.0f);
                                        word->y1_milli_pt = (int)(word_y1 * 1000.0f);
                                        memcpy(word->text, word_text, word_offset);
                                        word->text[word_offset] = '\0';
                                        word_started = 0;
                                        word_offset = 0;
                                    }
                                    if (is_space) {
                                        previous = ch;
                                        continue;
                                    }
                                }

                                char utf8[8];
                                int n = fz_runetochar(utf8, ch->c);
                                if (n > 0 && word_offset + (size_t)n < sizeof(word_text) - 1) {
                                    memcpy(word_text + word_offset, utf8, (size_t)n);
                                    word_offset += (size_t)n;
                                }
                                if (!word_started) {
                                    word_started = 1;
                                    word_x0 = ch->quad.ul.x;
                                    word_y0 = ch->quad.ul.y;
                                    word_x1 = ch->quad.ul.x;
                                    word_y1 = ch->quad.ul.y;
                                }
                                /* The box of the word is the bounding box of
                                   all of its characters. */
                                vocument_grow_box_for_character(&word_x0, &word_y0, &word_x1, &word_y1, &ch->quad);
                                previous = ch;
                            }

                            if (word_started && count_words < max_words) {
                                vocument_mupdf_text_word_t *word = &out_words[count_words++];
                                memset(word, 0, sizeof(*word));
                                word->x0_milli_pt = (int)(word_x0 * 1000.0f);
                                word->y0_milli_pt = (int)(word_y0 * 1000.0f);
                                word->x1_milli_pt = (int)(word_x1 * 1000.0f);
                                word->y1_milli_pt = (int)(word_y1 * 1000.0f);
                                memcpy(word->text, word_text, word_offset);
                                word->text[word_offset] = '\0';
                            }
                        }
                    }
                    count_lines++;
                }
            }

            *out_line_count = count_lines;
            if (out_word_count) {
                *out_word_count = count_words;
            }
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
