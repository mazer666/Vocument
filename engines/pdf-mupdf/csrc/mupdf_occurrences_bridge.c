/* Finding the places of a document that still carry a redacted word.
 *
 * The redaction of a page removes the text of the content stream. The text
 * can survive in other places: in the text of an annotation, in the value of
 * a form field, in the metadata, in the XMP packet, in the content of an
 * embedded file. A redaction that is reported as successful while the text is
 * still in the document is a lie — and the text does not have to be plain
 * text: it can be stored as UTF-16 or as the hex string of the UTF-16 bytes,
 * and a comparison of the plain text alone does not see it.
 *
 * This bridge therefore looks at every place of the document and reports
 * where a word still occurs. The caller decides what that means. */

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_text_search.h"

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

/* Reports the words in the content stream of a page. */
static void vocument_report_page_contents(
    fz_context *ctx,
    pdf_page *page,
    int page_index,
    const char *const *words,
    size_t word_count,
    vocument_report_t *report
) {
    pdf_obj *contents = pdf_dict_get(ctx, page->obj, PDF_NAME(Contents));
    if (!contents) {
        return;
    }
    if (pdf_is_array(ctx, contents)) {
        int count = pdf_array_len(ctx, contents);
        for (int i = 0; i < count; i++) {
            pdf_obj *part = pdf_array_get(ctx, contents, i);
            if (!pdf_is_stream(ctx, part)) {
                continue;
            }
            fz_buffer *buffer = pdf_load_stream(ctx, part);
            if (buffer) {
                unsigned char *data = NULL;
                size_t len = fz_buffer_storage(ctx, buffer, &data);
                if (vocument_buffer_carries_word(data, len, words, word_count)) {
                    vocument_report_line(report, "content", page_index, "content stream");
                }
                fz_drop_buffer(ctx, buffer);
            }
        }
        return;
    }
    if (!pdf_is_stream(ctx, contents)) {
        return;
    }
    fz_buffer *buffer = pdf_load_stream(ctx, contents);
    if (buffer) {
        unsigned char *data = NULL;
        size_t len = fz_buffer_storage(ctx, buffer, &data);
        if (vocument_buffer_carries_word(data, len, words, word_count)) {
            vocument_report_line(report, "content", page_index, "content stream");
        }
        fz_drop_buffer(ctx, buffer);
    }
}

/* Reports the words in the annotations and the form fields of a page. */
static void vocument_report_page_annots(
    fz_context *ctx,
    pdf_page *page,
    int page_index,
    const char *const *words,
    size_t word_count,
    vocument_report_t *report
) {
    pdf_obj *annots = pdf_dict_get(ctx, page->obj, PDF_NAME(Annots));
    if (!pdf_is_array(ctx, annots)) {
        return;
    }
    int count = pdf_array_len(ctx, annots);
    for (int i = 0; i < count; i++) {
        pdf_obj *annot = pdf_resolve_indirect(ctx, pdf_array_get(ctx, annots, i));
        if (!pdf_is_dict(ctx, annot)) {
            continue;
        }
        /* The text of an annotation, its title, its alternative text, its
           subject, its reply and the name of the file: all of them can carry
           the text of the user. */
        static const char *const text_fields[] = {
            "Contents", "T", "RC", "Alt", "Subj", "R", "IT", "DS"
        };
        for (size_t f = 0; f < sizeof(text_fields) / sizeof(text_fields[0]); f++) {
            pdf_obj *value = pdf_dict_gets(ctx, annot, text_fields[f]);
            if (pdf_is_array(ctx, value)) {
                int entries = pdf_array_len(ctx, value);
                for (int e = 0; e < entries; e++) {
                    pdf_obj *entry = pdf_resolve_indirect(ctx, pdf_array_get(ctx, value, e));
                    if (vocument_object_carries_word(ctx, entry, words, word_count)) {
                        vocument_report_line(report, "annotation", page_index, text_fields[f]);
                    }
                }
                continue;
            }
            if (vocument_object_carries_word(ctx, value, words, word_count)) {
                vocument_report_line(report, "annotation", page_index, text_fields[f]);
            }
        }
        /* The value of a form field. */
        if (pdf_dict_get(ctx, annot, PDF_NAME(FT))) {
            pdf_obj *value = pdf_dict_get(ctx, annot, PDF_NAME(V));
            if (vocument_object_carries_word(ctx, value, words, word_count)) {
                vocument_report_line(report, "form", page_index, "form value");
            }
            pdf_obj *parent = pdf_dict_get(ctx, annot, PDF_NAME(Parent));
            if (pdf_is_dict(ctx, parent)) {
                if (vocument_object_carries_word(ctx, pdf_dict_get(ctx, parent, PDF_NAME(V)), words, word_count)) {
                    vocument_report_line(report, "form", page_index, "form value");
                }
            }
        }
    }
}

/* Reports the words in the embedded files of the document. */
static void vocument_report_attachments(
    fz_context *ctx,
    pdf_document *pdf_doc,
    const char *const *words,
    size_t word_count,
    vocument_report_t *report
) {
    pdf_obj *tree = pdf_load_name_tree(ctx, pdf_doc, PDF_NAME(EmbeddedFiles));
    if (!tree) {
        return;
    }
    int count = pdf_dict_len(ctx, tree);
    for (int i = 0; i < count; i++) {
        pdf_obj *value = pdf_array_get(ctx, tree, i);
        if (pdf_is_dict(ctx, value)) {
            pdf_obj *filespec = pdf_dict_get(ctx, value, PDF_NAME(F));
            if (!pdf_is_dict(ctx, filespec)) {
                filespec = value;
            }
            if (!pdf_is_embedded_file(ctx, filespec)) {
                continue;
            }
            fz_buffer *buffer = pdf_load_embedded_file_contents(ctx, filespec);
            if (!buffer) {
                continue;
            }
            unsigned char *data = NULL;
            size_t len = fz_buffer_storage(ctx, buffer, &data);
            if (vocument_buffer_carries_word(data, len, words, word_count)) {
                pdf_filespec_params params;
                memset(&params, 0, sizeof(params));
                pdf_get_filespec_params(ctx, filespec, &params);
                vocument_report_line(
                    report,
                    "attachment",
                    -1,
                    params.filename ? params.filename : "embedded file"
                );
            }
            fz_drop_buffer(ctx, buffer);
        }
    }
}

/**
 * Reports the places of the document that still carry one of the words.
 *
 * The report is a list of lines: the kind of the place, the page it lies on
 * (-1 for the document itself) and a short description. The kinds are
 * `content`, `annotation`, `form`, `attachment`, `info` and `xmp`.
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
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(pdf_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int page_count = fz_count_pages(ctx, doc);
            for (int i = 0; i < page_count; i++) {
                pdf_page *page = pdf_load_page(ctx, pdf_doc, i);
                fz_try(ctx) {
                    vocument_report_page_contents(ctx, page, i, words, word_count, &report);
                    vocument_report_page_annots(ctx, page, i, words, word_count, &report);
                }
                fz_always(ctx) {
                    pdf_drop_page(ctx, page);
                }
                fz_catch(ctx) {
                    fz_rethrow(ctx);
                }
            }

            /* The document information dictionary. */
            pdf_obj *trailer = pdf_trailer(ctx, pdf_doc);
            pdf_obj *info = pdf_dict_get(ctx, trailer, PDF_NAME(Info));
            if (pdf_is_dict(ctx, info)) {
                static const char *const fields[] = {
                    "Title", "Subject", "Author", "Keywords", "Creator", "Producer"
                };
                for (size_t f = 0; f < sizeof(fields) / sizeof(fields[0]); f++) {
                    pdf_obj *value = pdf_dict_gets(ctx, info, fields[f]);
                    if (vocument_object_carries_word(ctx, value, words, word_count)) {
                        vocument_report_line(&report, "info", -1, fields[f]);
                    }
                }
            }

            /* The XMP packet of the document. */
            pdf_obj *root = pdf_dict_get(ctx, trailer, PDF_NAME(Root));
            pdf_obj *metadata = pdf_dict_get(ctx, root, PDF_NAME(Metadata));
            if (pdf_is_stream(ctx, metadata)) {
                fz_buffer *buffer = pdf_load_stream(ctx, metadata);
                if (buffer) {
                    unsigned char *data = NULL;
                    size_t xmp_len = fz_buffer_storage(ctx, buffer, &data);
                    if (vocument_buffer_carries_word(data, xmp_len, words, word_count)) {
                        vocument_report_line(&report, "xmp", -1, "XMP metadata");
                    }
                    fz_drop_buffer(ctx, buffer);
                }
            }

            /* The XMP packet of the pages. A document may carry one on every
               page — the PDF/A and the PDF/X files do. */
            for (int i = 0; i < page_count; i++) {
                pdf_page *page = pdf_load_page(ctx, pdf_doc, i);
                if (!page) {
                    continue;
                }
                pdf_obj *page_metadata = pdf_dict_get(ctx, page->obj, PDF_NAME(Metadata));
                if (pdf_is_stream(ctx, page_metadata)) {
                    fz_buffer *buffer = pdf_load_stream(ctx, page_metadata);
                    if (buffer) {
                        unsigned char *data = NULL;
                        size_t xmp_len = fz_buffer_storage(ctx, buffer, &data);
                        if (vocument_buffer_carries_word(data, xmp_len, words, word_count)) {
                            vocument_report_line(&report, "xmp", i, "XMP metadata of the page");
                        }
                        fz_drop_buffer(ctx, buffer);
                    }
                }
                pdf_drop_page(ctx, page);
            }

            /* The embedded files. */
            vocument_report_attachments(ctx, pdf_doc, words, word_count, &report);
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
