#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_serialize.h"

void vocument_mupdf_free_bytes(unsigned char *ptr);

/*
 * Reorders the pages of a PDF document according to `order` (0-indexed).
 * `order_len` must equal the page count of the document, and `order`
 * must contain a valid permutation of [0 .. order_len - 1].
 *
 * Status:
 *   0: success
 *   1: invalid arguments or invalid permutation
 *   2: cannot open or parse PDF
 *   3: out of memory
 */
int vocument_mupdf_reorder_pages(
    const unsigned char *bytes,
    size_t len,
    const int *order,
    size_t order_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !order || order_len == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_document *dst_doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(dst_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *src_doc = pdf_specifics(ctx, doc);
        if (!src_doc) {
            status = 2;
        } else {
            int count = fz_count_pages(ctx, doc);
            if ((size_t)count != order_len) {
                status = 1;
            } else {
                /* Validate permutation: each page 0..count-1 must appear once */
                int *seen = (int *)calloc((size_t)count, sizeof(int));
                if (!seen) {
                    fz_throw(ctx, FZ_ERROR_SYSTEM, "calloc failed");
                }
                for (size_t i = 0; i < order_len; i++) {
                    int p = order[i];
                    if (p < 0 || p >= count || seen[p]) {
                        status = 1;
                        break;
                    }
                    seen[p] = 1;
                }
                free(seen);

                if (status == 0) {
                    dst_doc = pdf_create_document(ctx);
                    for (size_t i = 0; i < order_len; i++) {
                        pdf_graft_page(ctx, dst_doc, -1, src_doc, order[i]);
                    }
                    vocument_serialize_pdf_doc_to_malloc(ctx, dst_doc, out_pdf_bytes, out_pdf_len);
                }
            }
        }
    }
    fz_always(ctx) {
        if (dst_doc) {
            pdf_drop_document(ctx, dst_doc);
        }
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

/*
 * Duplicates `page_index` (0-indexed) by inserting a copy immediately following it.
 *
 * Status:
 *   0: success
 *   1: invalid arguments or page index out of range
 *   2: cannot open or parse PDF
 *   3: out of memory
 */
int vocument_mupdf_duplicate_page(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || page_index < 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_document *dst_doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(dst_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *src_doc = pdf_specifics(ctx, doc);
        if (!src_doc) {
            status = 2;
        } else {
            int count = fz_count_pages(ctx, doc);
            if (page_index >= count) {
                status = 1;
            } else {
                dst_doc = pdf_create_document(ctx);
                for (int i = 0; i < count; i++) {
                    pdf_graft_page(ctx, dst_doc, -1, src_doc, i);
                    if (i == page_index) {
                        pdf_graft_page(ctx, dst_doc, -1, src_doc, i);
                    }
                }
                vocument_serialize_pdf_doc_to_malloc(ctx, dst_doc, out_pdf_bytes, out_pdf_len);
            }
        }
    }
    fz_always(ctx) {
        if (dst_doc) {
            pdf_drop_document(ctx, dst_doc);
        }
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

/*
 * Deletes the specified `page_indices` from the PDF document.
 * Refuses if all pages would be deleted (a document must keep >= 1 page).
 *
 * Status:
 *   0: success
 *   1: invalid arguments, invalid index, or attempt to delete all pages
 *   2: cannot open or parse PDF
 *   3: out of memory
 */
int vocument_mupdf_delete_pages(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_document *dst_doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(dst_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *src_doc = pdf_specifics(ctx, doc);
        if (!src_doc) {
            status = 2;
        } else {
            int count = fz_count_pages(ctx, doc);
            char *delete_mask = (char *)calloc((size_t)count, sizeof(char));
            if (!delete_mask) {
                fz_throw(ctx, FZ_ERROR_SYSTEM, "calloc failed");
            }

            size_t deleted_count = 0;
            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= count) {
                    status = 1;
                    break;
                }
                if (!delete_mask[p]) {
                    delete_mask[p] = 1;
                    deleted_count++;
                }
            }

            if (status == 0) {
                if (deleted_count >= (size_t)count) {
                    /* Refuse: document cannot be emptied */
                    status = 1;
                } else {
                    dst_doc = pdf_create_document(ctx);
                    for (int i = 0; i < count; i++) {
                        if (!delete_mask[i]) {
                            pdf_graft_page(ctx, dst_doc, -1, src_doc, i);
                        }
                    }
                    vocument_serialize_pdf_doc_to_malloc(ctx, dst_doc, out_pdf_bytes, out_pdf_len);
                }
            }
            free(delete_mask);
        }
    }
    fz_always(ctx) {
        if (dst_doc) {
            pdf_drop_document(ctx, dst_doc);
        }
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

/*
 * Extracts the requested `page_indices` into a new standalone PDF document.
 * The source document is left completely unchanged.
 *
 * Status:
 *   0: success
 *   1: invalid arguments or index out of range
 *   2: cannot open or parse PDF
 *   3: out of memory
 */
int vocument_mupdf_extract_pages(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_document *dst_doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(dst_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *src_doc = pdf_specifics(ctx, doc);
        if (!src_doc) {
            status = 2;
        } else {
            int count = fz_count_pages(ctx, doc);
            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= count) {
                    status = 1;
                    break;
                }
            }

            if (status == 0) {
                dst_doc = pdf_create_document(ctx);
                for (size_t i = 0; i < page_indices_len; i++) {
                    pdf_graft_page(ctx, dst_doc, -1, src_doc, page_indices[i]);
                }
                vocument_serialize_pdf_doc_to_malloc(ctx, dst_doc, out_pdf_bytes, out_pdf_len);
            }
        }
    }
    fz_always(ctx) {
        if (dst_doc) {
            pdf_drop_document(ctx, dst_doc);
        }
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
