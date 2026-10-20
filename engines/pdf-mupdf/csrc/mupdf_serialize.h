/* Writing a document back to bytes.
 *
 * The removed content of a redaction must not stay in the file as an
 * unreadable object — a text that is no longer referenced can still be read
 * from the file. The writer therefore collects the objects that are no
 * longer referenced. */

#ifndef VOCUMENT_MUPDF_SERIALIZE_H
#define VOCUMENT_MUPDF_SERIALIZE_H

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

/* Writes the document into a freshly allocated buffer.
 *
 * The caller owns the returned buffer and releases it with
 * `vocument_mupdf_free_bytes`. */
static inline int vocument_serialize_pdf_doc_to_malloc(
    fz_context *ctx,
    pdf_document *pdf_doc,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    fz_buffer *buf = NULL;
    fz_output *out = NULL;
    fz_var(buf);
    fz_var(out);

    fz_try(ctx) {
        buf = fz_new_buffer(ctx, 4096);
        out = fz_new_output_with_buffer(ctx, buf);
        pdf_write_options opts = pdf_default_write_options;
        opts.do_appearance = 1;
        opts.reproducible = 1;
        /* Objects that are no longer referenced are removed. Without this a
           removed content — the text of a redaction, for example — would stay
           in the file as an unread object and could still be read from it. */
        opts.do_garbage = 1;
        pdf_write_document(ctx, pdf_doc, out, &opts);
        fz_close_output(ctx, out);

        unsigned char *storage = NULL;
        size_t storage_len = fz_buffer_storage(ctx, buf, &storage);
        unsigned char *copied = (unsigned char *)malloc(storage_len > 0 ? storage_len : 1);
        if (!copied) {
            fz_throw(ctx, FZ_ERROR_SYSTEM, "malloc failed for serialized PDF buffer");
        }
        if (storage_len > 0 && storage) {
            memcpy(copied, storage, storage_len);
        }
        *out_pdf_bytes = copied;
        *out_pdf_len = storage_len;
    }
    fz_always(ctx) {
        if (out) {
            fz_drop_output(ctx, out);
        }
        if (buf) {
            fz_drop_buffer(ctx, buf);
        }
    }
    fz_catch(ctx) {
        fz_rethrow(ctx);
    }
    return 0;
}

#endif /* VOCUMENT_MUPDF_SERIALIZE_H */
