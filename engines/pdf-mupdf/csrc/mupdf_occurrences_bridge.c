/* Finding the places of a document that still carry a redacted word.
 *
 * The redaction of a page removes the content of the page. The text can
 * survive in other places: in the text of an annotation, in the popup of an
 * annotation, in the value of a form field, in the metadata, in the XMP
 * packet, in the structure tree, in the name tree, in the optional content,
 * in the private data of a page, in the content of an embedded file. A
 * redaction that is reported as successful while the text is still in the
 * document is a lie — and the text does not have to be plain text: it can be
 * stored as UTF-16 or as the hex string of the UTF-16 bytes, and a
 * comparison of the plain text alone does not see it.
 *
 * This bridge therefore looks at **every** place of the document and reports
 * where a word still occurs. The walk over the objects of the document is
 * the one of `mupdf_walk.h`: it visits the pages, the catalog, the
 * annotations, the form fields, the metadata, the structure tree, the name
 * tree, the optional content, the private data and the embedded files — the
 * places that the third round of the red team showed are missed by a list of
 * known places.
 *
 * The kinds of the report are:
 *
 * - `content`: the content stream of a page — the visible text of the page.
 *   The core decides what a word on another page means: the user marked a
 *   region, and the word belongs to the document.
 * - `hidden`: every other place — the text of an annotation, the value of a
 *   form field, the metadata, the structure tree, the name tree, the
 *   optional content, the private data, the content of an embedded file.
 *   The core empties these places before it looks for them; a place that
 *   still carries the word afterwards stops the redaction. */

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_text_search.h"
#include "mupdf_walk.h"

/* The maximum number of reported places. */
#define VOCUMENT_MAX_OCCURRENCES 200

/* The buffer that collects the report. */
typedef struct {
    char *data;
    size_t capacity;
    size_t used;
    int count;
    int truncated;
} vocument_report_t;

/* Appends a line to the report. */
static void vocument_report_line(
    vocument_report_t *report,
    const char *kind,
    int page_index,
    const char *where
) {
    if (report->count >= VOCUMENT_MAX_OCCURRENCES) {
        report->truncated = 1;
        return;
    }
    int written = snprintf(
        report->data + report->used,
        report->capacity - report->used,
        "%s\t%d\t%s\n",
        kind,
        page_index,
        where ? where : ""
    );
    if (written < 0 || (size_t)written >= report->capacity - report->used) {
        report->truncated = 1;
        return;
    }
    report->used += (size_t)written;
    report->count++;
}

/* Answers whether the buffer carries one of the words. */
static int vocument_buffer_carries_word(
    const unsigned char *data,
    size_t len,
    const char *const *words,
    size_t word_count
) {
    for (size_t i = 0; i < word_count; i++) {
        if (words[i] && words[i][0] && vocument_contains_word(data, len, words[i])) {
            return 1;
        }
    }
    return 0;
}

/* Answers whether the **text** of the content carries one of the words.
 *
 * A writer can split a word over several show operations: `(GE) (HEIM) Tj`
 * puts "GEHEIM" on the page, but the bytes of the stream do not carry the
 * word in one piece. The comparison of bytes is blind for it, and the fifth
 * round of the red team measured that the verification of a redaction
 * accepted a document whose hidden form carried the secret in such pieces.
 * The text of the stream is therefore read as well. */
static int vocument_text_carries_word(
    const unsigned char *data,
    size_t len,
    const char *const *words,
    size_t word_count
) {
    for (size_t i = 0; i < word_count; i++) {
        if (words[i] && words[i][0] && vocument_text_run_carries_word(data, len, words[i])) {
            return 1;
        }
    }
    return 0;
}

/* Answers whether the object carries one of the words. */
static int vocument_object_carries_word(
    fz_context *ctx,
    pdf_obj *obj,
    const char *const *words,
    size_t word_count
) {
    if (!obj || !pdf_is_string(ctx, obj)) {
        return 0;
    }
    /* The buffer belongs to the object of the PDF: it must not be released.
       Its length is not the length of a C string — a text can contain NUL
       characters. */
    const char *text = pdf_to_str_buf(ctx, obj);
    size_t text_len = pdf_to_str_len(ctx, obj);
    int found = 0;
    for (size_t i = 0; i < word_count && !found; i++) {
        if (words[i] && words[i][0]) {
            /* The text of a PDF object can be plain text, UTF-16 or the
               hex string of the UTF-16 bytes. All of them have to be seen. */
            found = vocument_contains_word(
                (const unsigned char *)text,
                text_len,
                words[i]
            );
        }
    }
    return found;
}

/* Answers whether the buffer is the beginning of a container.
 *
 * A container hides its content: the text of a file that is stored in a ZIP
 * archive is compressed, and nobody can find it without unpacking the
 * archive. The redaction does not claim that the text is gone from such a
 * place — it says that it could not look into it. */
static int vocument_buffer_is_container(const unsigned char *data, size_t len) {
    if (len >= 4 && data[0] == 'P' && data[1] == 'K' &&
        ((data[2] == 0x03 && data[3] == 0x04) ||
         (data[2] == 0x05 && data[3] == 0x06) ||
         (data[2] == 0x07 && data[3] == 0x08))) {
        return 1;
    }
    if (len >= 4 && data[0] == 0x1F && data[1] == 0x8B) {
        return 1;
    }
    if (len >= 6 && memcmp(data, "Rar!", 4) == 0) {
        return 1;
    }
    if (len >= 6 && memcmp(data, "7z\xBC\xAF\x27\x1C", 6) == 0) {
        return 1;
    }
    if (len >= 3 && memcmp(data, "BZh", 3) == 0) {
        return 1;
    }
    return 0;
}

/* The state of the walk over the places of the document. */
typedef struct {
    fz_context *ctx;
    vocument_report_t *report;
    const char *const *words;
    size_t word_count;
} vocument_occurrences_t;

/* Reports the places of the document that still carry one of the words.
 *
 * Every string and every stream of the document is offered here. A string
 * is hidden text — the text of an annotation, the value of a form field, the
 * metadata, the structure tree, the name tree, the optional content, the
 * private data. A stream is the content of a page or the content of
 * something else: the XMP packet, the file of an attachment, the content of
 * a form object. */
static int vocument_report_place(
    void *user,
    const vocument_walk_item *item
) {
    vocument_occurrences_t *state = (vocument_occurrences_t *)user;
    fz_context *ctx = state->ctx;

    if (pdf_is_string(ctx, item->object)) {
        if (vocument_object_carries_word(ctx, item->object, state->words, state->word_count)) {
            vocument_report_line(
                state->report, "hidden", item->page_index, item->path
            );
        }
        return 1;
    }

    if (!pdf_is_stream(ctx, item->object)) {
        return 1;
    }

    fz_buffer *buffer = NULL;
    fz_var(buffer);
    fz_try(ctx) {
        buffer = pdf_load_stream(ctx, item->object);
    }
    fz_catch(ctx) {
        /* A stream that cannot be read cannot carry a word that is read: the
           redaction reports the place as uncleaned, so that nothing is
           silently ignored. */
        vocument_report_line(
            state->report, "hidden", item->page_index, item->path
        );
        return 1;
    }
    if (buffer) {
        unsigned char *data = NULL;
        size_t len = fz_buffer_storage(ctx, buffer, &data);
        /* A file that is attached to the document can be a container: the
           text inside it is compressed, and the redaction cannot look into
           it. The place is reported as uninspected — the redaction does not
           claim that the text is gone from there. */
        /* A container is a container, wherever it hangs. The fourth round of
           the red team showed that a test on the path is no test at all: the
           same archive was refused under `/Names/EmbeddedFiles` and accepted
           under `/AF`, and the redaction reported success while the secret
           stayed in the archive. */
        if (!item->is_page_content && vocument_buffer_is_container(data, len)) {
            vocument_report_line(
                state->report, "uninspected", item->page_index, item->path
            );
        } else if (vocument_buffer_carries_word(data, len, state->words, state->word_count) ||
                   vocument_text_carries_word(data, len, state->words, state->word_count)) {
            vocument_report_line(
                state->report,
                item->is_page_content ? "content" : "hidden",
                item->page_index,
                item->is_page_content ? "content stream of the page" : item->path
            );
        }
        fz_drop_buffer(ctx, buffer);
    }
    return 1;
}

/**
 * Reports the places of the document that still carry one of the words.
 *
 * The report is a list of lines: the kind of the place, the page it lies on
 * (-1 for the document itself) and a description of the place. The kinds are
 * `content` (the content stream of a page) and `hidden` (every other place).
 */
int vocument_mupdf_find_text_occurrences(
    const unsigned char *bytes,
    size_t len,
    const char *const *words,
    size_t word_count,
    char *out_buf,
    size_t out_cap,
    char *err_buf,
    size_t err_buf_len
) {
    if (!bytes || !out_buf || out_cap == 0 || !words) {
        return 3;
    }
    out_buf[0] = '\0';
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    vocument_report_t report;
    report.data = out_buf;
    report.capacity = out_cap;
    report.used = 0;
    report.count = 0;
    report.truncated = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_document *pdf_doc = NULL;
    vocument_walk_t walk;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(pdf_doc);
    fz_var(status);
    fz_var(walk);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            vocument_occurrences_t state;
            state.ctx = ctx;
            state.report = &report;
            state.words = words;
            state.word_count = word_count;

            vocument_walk_init(&walk, ctx, vocument_report_place, &state);
            fz_try(ctx) {
                vocument_walk_document(&walk, pdf_doc);
            }
            fz_always(ctx) {
                vocument_walk_free(&walk);
            }
            fz_catch(ctx) {
                fz_rethrow(ctx);
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        if (err_buf && err_buf_len > 0) {
            snprintf(err_buf, err_buf_len, "%s", fz_caught_message(ctx));
        }
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}
