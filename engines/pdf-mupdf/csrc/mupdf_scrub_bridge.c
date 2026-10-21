/* Removing the redacted words from the places of a document that do not lie
 * on the page.
 *
 * The redaction of MuPDF removes the content of a page. The same text can lie
 * in other places of the document: in the text of an annotation, in the popup
 * of an annotation, in the value of a form field, in the metadata, in the
 * XMP packet, in the structure tree, in the name tree, in the optional
 * content, in the private data of a page. A redaction that reports success
 * while the text is still in the document is a lie.
 *
 * The second round of the red team worked with a list of known places. The
 * third round showed what a list misses: a popup that does not stand in
 * `/Annots`, the structure tree, the optional content, the name tree — every
 * one of them carried the secret, and the redaction reported success. The
 * scrub therefore walks through **every** object of the document
 * (`mupdf_walk.h`) and empties every string that carries the word.
 *
 * The embedded files are not touched: a file that is attached to the document
 * cannot be cleaned partly without destroying it, and destroying the data of
 * the user is worse than a redaction that is refused. The caller has to
 * report the text that is still in the document. */

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_serialize.h"
#include "mupdf_text_search.h"
#include "mupdf_walk.h"

/* The placeholder that replaces a word in a text that cannot be emptied. */
#define VOCUMENT_REDACTION_PLACEHOLDER ""

/* The keys whose value names something of the document and not the text of
 * the user: the name of a font, the name of an encoding, the identifier of
 * the document. Emptying them would break the document instead of removing a
 * secret. A word in such a key is reported by the search instead. */
static const char *const VOCUMENT_PROTECTED_KEYS[] = {
    "BaseFont", "FontName", "Encoding", "ID", "Filter", "Length", "Type", "Subtype"
};

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
    for (size_t i = 0; i < word_count; i++) {
        if (words[i] && words[i][0] && vocument_contains_word(
                (const unsigned char *)text, text_len, words[i])) {
            return 1;
        }
    }
    return 0;
}

/* Answers whether the key names something of the document and not the text
   of the user. */
static int vocument_key_is_protected(fz_context *ctx, pdf_obj *key) {
    if (!key) {
        return 0;
    }
    const char *name = pdf_to_name(ctx, key);
    if (!name) {
        return 0;
    }
    for (size_t i = 0; i < sizeof(VOCUMENT_PROTECTED_KEYS) / sizeof(VOCUMENT_PROTECTED_KEYS[0]); i++) {
        if (strcmp(name, VOCUMENT_PROTECTED_KEYS[i]) == 0) {
            return 1;
        }
    }
    return 0;
}

/* The state of the scrub. */
typedef struct {
    fz_context *ctx;
    const char *const *words;
    size_t word_count;
    int annotations;
    int form_values;
    int other;
} vocument_scrub_state;

/* Empties a string of the document that carries one of the words.
 *
 * The string is replaced by an empty one in the place it stands: the
 * document keeps its structure, the text of the user is gone. */
static int vocument_scrub_place(void *user, const vocument_walk_item *item) {
    vocument_scrub_state *state = (vocument_scrub_state *)user;
    fz_context *ctx = state->ctx;

    if (!pdf_is_string(ctx, item->object)) {
        return 1;
    }
    if (vocument_key_is_protected(ctx, item->key)) {
        return 1;
    }
    if (!vocument_object_carries_word(ctx, item->object, state->words, state->word_count)) {
        return 1;
    }

    pdf_obj *empty = pdf_new_string(ctx, "", 0);
    if (!empty) {
        return 1;
    }
    if (item->parent && item->key) {
        pdf_dict_put(ctx, item->parent, item->key, empty);
    } else if (item->parent && item->index >= 0) {
        pdf_array_put(ctx, item->parent, item->index, empty);
    } else {
        pdf_drop_obj(ctx, empty);
        return 1;
    }

    /* The counters tell what was cleaned. A value of a form field is the
       value of a widget — an annotation that carries a field. */
    int is_form_value = 0;
    if (item->parent && pdf_dict_get(ctx, item->parent, PDF_NAME(FT))) {
        is_form_value = 1;
    }
    if (is_form_value) {
        state->form_values++;
    } else if (strstr(item->path, "/Annots") || strstr(item->path, "/Popup")) {
        state->annotations++;
    } else {
        state->other++;
    }
    return 1;
}

/* Copies the buffer and replaces every occurrence of one of the words with
 * the placeholder. The result is a newly allocated buffer that the caller
 * has to release with `free`. */
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

/* Removes the words from the XMP packet of a stream.
 *
 * The XMP packet is an XML text: it cannot be emptied like a string of the
 * document, because the packet has to stay a valid packet. The words are
 * replaced by a placeholder instead. */
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

/* Removes the words from the XMP packets of the document and of its pages. */
static int vocument_scrub_xmp_packets(
    fz_context *ctx,
    pdf_document *pdf_doc,
    const char *const *words,
    size_t word_count
) {
    int changed = 0;
    pdf_obj *trailer = pdf_trailer(ctx, pdf_doc);
    pdf_obj *root = pdf_dict_get(ctx, trailer, PDF_NAME(Root));
    changed += vocument_scrub_xmp_stream(
        ctx, pdf_doc, pdf_dict_get(ctx, root, PDF_NAME(Metadata)), words, word_count
    );

    int page_count = pdf_count_pages(ctx, pdf_doc);
    for (int i = 0; i < page_count; i++) {
        pdf_page *page = NULL;
        fz_var(page);
        fz_try(ctx) {
            page = pdf_load_page(ctx, pdf_doc, i);
            if (page) {
                /* The metadata of the page: the object of the dictionary is
                   the stream itself, an indirect object that MuPDF knows. */
                changed += vocument_scrub_xmp_stream(
                    ctx,
                    pdf_doc,
                    pdf_dict_get(ctx, page->obj, PDF_NAME(Metadata)),
                    words,
                    word_count
                );
            }
        }
        fz_always(ctx) {
            if (page) {
                pdf_drop_page(ctx, page);
            }
        }
        fz_catch(ctx) {
            fz_rethrow(ctx);
        }
    }
    return changed;
}

/**
 * Removes the given words from the places of the document that do not lie
 * on the page.
 *
 * Every string of the document is emptied when it carries one of the words:
 * the text of an annotation, the popup of an annotation, the value of a
 * form field, the metadata, the structure tree, the name tree, the optional
 * content, the private data of a page.
 *
 * The embedded files are not touched: a file that is attached to the
 * document cannot be cleaned partly without destroying it, and destroying
 * the data of the user is worse than a redaction that is refused. The caller
 * has to report the text that is still in the document.
 *
 * The counters tell how many annotations, form values and other places were
 * changed.
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
            vocument_scrub_state state;
            state.ctx = ctx;
            state.words = words;
            state.word_count = word_count;
            state.annotations = 0;
            state.form_values = 0;
            state.other = 0;

            vocument_walk_init(&walk, ctx, vocument_scrub_place, &state);
            fz_try(ctx) {
                vocument_walk_document(&walk, pdf_doc);
            }
            fz_always(ctx) {
                vocument_walk_free(&walk);
            }
            fz_catch(ctx) {
                fz_rethrow(ctx);
            }

            if (out_scrubbed_annotations) *out_scrubbed_annotations = state.annotations;
            if (out_scrubbed_form_values) *out_scrubbed_form_values = state.form_values;
            if (out_scrubbed_metadata) *out_scrubbed_metadata = state.other;

            /* The XMP packets are XML: their words are replaced by a
               placeholder instead of being emptied. */
            int xmp_changed = vocument_scrub_xmp_packets(ctx, pdf_doc, words, word_count);
            if (xmp_changed > 0 && out_scrubbed_metadata) {
                *out_scrubbed_metadata += xmp_changed;
            }

            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
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
