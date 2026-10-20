/* Removing the redacted text from the places of a document that do not lie
 * on the page.
 *
 * The redaction of MuPDF removes the content of a page: the text of the
 * content stream and the pixels of the images. Everything else that carries
 * the text stays where it is — the text of an annotation, the value of a form
 * field, the subject of the document, the XMP packet, the content of an
 * embedded file. A redaction that reports success while the text is still in
 * the document is a lie.
 *
 * This bridge removes the text from the places where that can be done
 * without destroying the document, and it leaves the places that cannot be
 * cleaned alone — the caller has to report them. */

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_serialize.h"
#include "mupdf_text_search.h"

/* The text that replaces a redacted word in the metadata. */
#define VOCUMENT_REDACTION_PLACEHOLDER "[redacted]"

/* Answers whether the object carries one of the words.
 *
 * The text of a PDF object is decoded by MuPDF: a string that is stored as
 * UTF-16 or as a hex string is handed out as UTF-8. The comparison of the
 * plain text is therefore enough here. */
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

/* Empties the text of the annotations of the page.
 *
 * The annotations are read from the `/Annots` array of the page instead of
 * the annotation handles of MuPDF: the handle does not give access to the
 * object of the annotation, and the array is where the text lives. */
static int vocument_scrub_annotations(
    fz_context *ctx,
    pdf_page *page,
    const char *const *words,
    size_t word_count,
    int *out_changed
) {
    int changed = 0;
    pdf_obj *annots = pdf_dict_get(ctx, page->obj, PDF_NAME(Annots));
    if (!pdf_is_array(ctx, annots)) {
        return 0;
    }
    int count = pdf_array_len(ctx, annots);
    for (int i = 0; i < count; i++) {
        pdf_obj *annot = pdf_resolve_indirect(ctx, pdf_array_get(ctx, annots, i));
        if (!pdf_is_dict(ctx, annot)) {
            continue;
        }
        pdf_obj *contents = pdf_dict_get(ctx, annot, PDF_NAME(Contents));
        if (pdf_is_array(ctx, contents)) {
            /* A text array holds the text in several languages. */
            int entries = pdf_array_len(ctx, contents);
            for (int e = 0; e < entries; e++) {
                pdf_obj *entry = pdf_resolve_indirect(ctx, pdf_array_get(ctx, contents, e));
                if (!vocument_object_carries_word(ctx, entry, words, word_count)) {
                    continue;
                }
                pdf_array_put(ctx, contents, e, pdf_new_string(ctx, "", 0));
                changed++;
            }
            continue;
        }
        /* The text of the annotation is emptied, the annotation itself stays:
           it belongs to the document and may carry information that the user
           wants to keep.

           An annotation carries more than its text: the title, the
           alternative text, the subject, the reply and the name of the file.
           All of them can carry the text of the user, and all of them are
           emptied. */
        static const char *const text_fields[] = {
            "Contents", "T", "RC", "Alt", "Subj", "R", "IT", "DS"
        };
        for (size_t f = 0; f < sizeof(text_fields) / sizeof(text_fields[0]); f++) {
            pdf_obj *value = pdf_dict_gets(ctx, annot, text_fields[f]);
            if (!vocument_object_carries_word(ctx, value, words, word_count)) {
                continue;
            }
            pdf_dict_puts(ctx, annot, text_fields[f], pdf_new_string(ctx, "", 0));
            changed++;
        }
    }

    if (out_changed) {
        *out_changed += changed;
    }
    return changed;
}

/* Empties the value of a form field. */
static int vocument_scrub_field(
    fz_context *ctx,
    pdf_obj *field,
    const char *const *words,
    size_t word_count
) {
    if (!field || !pdf_is_dict(ctx, field)) {
        return 0;
    }
    int changed = 0;
    /* The value of the field and its default value. */
    if (vocument_object_carries_word(ctx, pdf_dict_get(ctx, field, PDF_NAME(V)), words, word_count)) {
        pdf_dict_puts(ctx, field, "V", pdf_new_string(ctx, "", 0));
        changed++;
    }
    if (vocument_object_carries_word(ctx, pdf_dict_get(ctx, field, PDF_NAME(DV)), words, word_count)) {
        pdf_dict_puts(ctx, field, "DV", pdf_new_string(ctx, "", 0));
        changed++;
    }
    return changed;
}

/* Empties the values of the form fields of the page. */
static int vocument_scrub_form_values(
    fz_context *ctx,
    pdf_page *page,
    const char *const *words,
    size_t word_count,
    int *out_changed
) {
    int changed = 0;
    pdf_obj *annots = pdf_dict_get(ctx, page->obj, PDF_NAME(Annots));
    if (!pdf_is_array(ctx, annots)) {
        return 0;
    }
    int count = pdf_array_len(ctx, annots);
    for (int i = 0; i < count; i++) {
        pdf_obj *widget = pdf_resolve_indirect(ctx, pdf_array_get(ctx, annots, i));
        if (!pdf_is_dict(ctx, widget)) {
            continue;
        }
        /* Only the widgets carry a form field. */
        if (!pdf_dict_get(ctx, widget, PDF_NAME(FT))) {
            continue;
        }
        /* A widget carries the value itself, or it inherits it from the
           field it belongs to. */
        changed += vocument_scrub_field(ctx, widget, words, word_count);
        pdf_obj *parent = pdf_dict_get(ctx, widget, PDF_NAME(Parent));
        if (pdf_is_dict(ctx, parent)) {
            changed += vocument_scrub_field(ctx, parent, words, word_count);
        }
    }

    if (out_changed) {
        *out_changed += changed;
    }
    return changed;
}

/* Copies the buffer and replaces every occurrence of one of the words with
   the placeholder. The result is a newly allocated buffer that the caller
   has to release with `free`. */
static unsigned char *vocument_replace_words(
    const unsigned char *data,
    size_t len,
    const char *const *words,
    size_t word_count,
    size_t *out_len
) {
    size_t capacity = len + 64;
    unsigned char *out = (unsigned char *)malloc(capacity);
    if (!out) {
        return NULL;
    }
    size_t used = 0;
    size_t i = 0;
    while (i < len) {
        int replaced = 0;
        for (size_t w = 0; w < word_count && !replaced; w++) {
            if (!words[w] || !words[w][0]) {
                continue;
            }
            size_t word_len = strlen(words[w]);
            if (i + word_len <= len &&
                vocument_bytes_equal_ci(data + i, (const unsigned char *)words[w], word_len)) {
                size_t placeholder_len = sizeof(VOCUMENT_REDACTION_PLACEHOLDER) - 1;
                if (used + placeholder_len + 1 > capacity) {
                    size_t new_capacity = (used + placeholder_len + 1) * 2;
                    unsigned char *grown = (unsigned char *)realloc(out, new_capacity);
                    if (!grown) {
                        free(out);
                        return NULL;
                    }
                    out = grown;
                    capacity = new_capacity;
                }
                memcpy(out + used, VOCUMENT_REDACTION_PLACEHOLDER, placeholder_len);
                used += placeholder_len;
                i += word_len;
                replaced = 1;
            }
        }
        if (!replaced) {
            if (used + 2 > capacity) {
                size_t new_capacity = capacity * 2;
                unsigned char *grown = (unsigned char *)realloc(out, new_capacity);
                if (!grown) {
                    free(out);
                    return NULL;
                }
                out = grown;
                capacity = new_capacity;
            }
            out[used++] = data[i++];
        }
    }
    out[used] = '\0';
    *out_len = used;
    return out;
}

static int vocument_scrub_xmp_stream(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj *metadata,
    const char *const *words,
    size_t word_count
) {
    int changed = 0;
    fz_buffer *xmp_buffer = NULL;
    fz_var(xmp_buffer);

    if (!pdf_is_stream(ctx, metadata)) {
        return 0;
    }

    fz_try(ctx) {
        xmp_buffer = pdf_load_stream(ctx, metadata);
        if (xmp_buffer) {
            unsigned char *data = NULL;
            size_t len = fz_buffer_storage(ctx, xmp_buffer, &data);
            int carries = 0;
            for (size_t w = 0; w < word_count && !carries; w++) {
                if (words[w] && words[w][0]) {
                    carries = vocument_contains_word(data, len, words[w]);
                }
            }
            if (carries) {
                size_t new_len = 0;
                unsigned char *replaced = vocument_replace_words(data, len, words, word_count, &new_len);
                if (replaced) {
                    fz_buffer *new_buffer = fz_new_buffer_from_copied_data(ctx, replaced, new_len);
                    free(replaced);
                    if (new_buffer) {
                        pdf_update_stream(ctx, pdf_doc, metadata, new_buffer, 0);
                        fz_drop_buffer(ctx, new_buffer);
                        changed++;
                    }
                }
            }
        }
    }
    fz_always(ctx) {
        if (xmp_buffer) {
            fz_drop_buffer(ctx, xmp_buffer);
        }
    }
    fz_catch(ctx) {
        fz_rethrow(ctx);
    }
    return changed;
}

static int vocument_scrub_page_metadata(
    fz_context *ctx,
    pdf_document *pdf_doc,
    const char *const *words,
    size_t word_count,
    int *out_changed
) {
    int changed = 0;
    int page_count = pdf_count_pages(ctx, pdf_doc);
    for (int i = 0; i < page_count; i++) {
        pdf_page *page = pdf_load_page(ctx, pdf_doc, i);
        if (!page) {
            continue;
        }
        /* The metadata of the page: the object of the dictionary is the
           stream itself, an indirect object that MuPDF knows. */
        pdf_obj *metadata = pdf_dict_get(ctx, page->obj, PDF_NAME(Metadata));
        changed += vocument_scrub_xmp_stream(ctx, pdf_doc, metadata, words, word_count);
        pdf_drop_page(ctx, page);
    }
    if (out_changed) {
        *out_changed += changed;
    }
    return 0;
}

/* Empties the metadata of the document: the fields of the document
   information dictionary and the XMP packet. */
static int vocument_scrub_metadata(
    fz_context *ctx,
    pdf_document *pdf_doc,
    const char *const *words,
    size_t word_count,
    int *out_changed
) {
    int changed = 0;
    fz_buffer *xmp_buffer = NULL;
    fz_var(xmp_buffer);

    fz_try(ctx) {
        /* The document information dictionary: the subject, the keywords and
           the other text fields. */
        pdf_obj *trailer = pdf_trailer(ctx, pdf_doc);
        pdf_obj *info = pdf_dict_get(ctx, trailer, PDF_NAME(Info));
        if (pdf_is_dict(ctx, info)) {
            static const char *const fields[] = {
                "Title", "Subject", "Author", "Keywords", "Creator", "Producer"
            };
            for (size_t f = 0; f < sizeof(fields) / sizeof(fields[0]); f++) {
                pdf_obj *value = pdf_dict_gets(ctx, info, fields[f]);
                if (!vocument_object_carries_word(ctx, value, words, word_count)) {
                    continue;
                }
                pdf_dict_puts(ctx, info, fields[f], pdf_new_string(ctx, "", 0));
                changed++;
            }
        }

        /* The XMP packet of the document. */
        pdf_obj *root = pdf_dict_get(ctx, trailer, PDF_NAME(Root));
        pdf_obj *metadata = pdf_dict_get(ctx, root, PDF_NAME(Metadata));
        changed += vocument_scrub_xmp_stream(ctx, pdf_doc, metadata, words, word_count);
    }
    fz_always(ctx) {
        if (xmp_buffer) {
            fz_drop_buffer(ctx, xmp_buffer);
        }
    }
    fz_catch(ctx) {
        fz_rethrow(ctx);
    }

    if (out_changed) {
        *out_changed += changed;
    }
    return 0;
}

/**
 * Removes the given words from the annotations, the form fields and the
 * metadata of the document.
 *
 * The embedded files are not touched: a file that is attached to the
 * document cannot be cleaned partly without destroying it, and destroying
 * the data of the user is worse than a redaction that is refused. The caller
 * has to report the text that is still in the document.
 *
 * The counters tell how many annotations, form values and metadata fields
 * were changed.
 */
int vocument_mupdf_scrub_document(
    const unsigned char *bytes,
    size_t len,
    const char *const *words,
    size_t word_count,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    int *out_scrubbed_annotations,
    int *out_scrubbed_form_values,
    int *out_scrubbed_metadata,
    char *err_buf,
    size_t err_buf_len
) {
    if (!bytes || !out_pdf_bytes || !out_pdf_len || !words) {
        return 3;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;
    if (out_scrubbed_annotations) *out_scrubbed_annotations = 0;
    if (out_scrubbed_form_values) *out_scrubbed_form_values = 0;
    if (out_scrubbed_metadata) *out_scrubbed_metadata = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

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
                    vocument_scrub_annotations(ctx, page, words, word_count, out_scrubbed_annotations);
                    vocument_scrub_form_values(ctx, page, words, word_count, out_scrubbed_form_values);
                }
                fz_always(ctx) {
                    pdf_drop_page(ctx, page);
                }
                fz_catch(ctx) {
                    fz_rethrow(ctx);
                }
            }
            status = vocument_scrub_metadata(ctx, pdf_doc, words, word_count, out_scrubbed_metadata);
            if (status == 0) {
                status = vocument_scrub_page_metadata(ctx, pdf_doc, words, word_count, out_scrubbed_metadata);
            }
            if (status == 0) {
                vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
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
