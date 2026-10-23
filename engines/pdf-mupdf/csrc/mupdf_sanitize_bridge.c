/* Sanitizing a PDF document by stripping metadata, attachments,
 * actions, annotations, and form structures (`SAN-001`).
 *
 * Full sanitize cleans entire classes of hidden or active content from
 * the document so that no remnants survive.
 */

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_serialize.h"

/**
 * Sanitizes the PDF document according to the requested options.
 *
 * All unreferenced objects resulting from deletions are collected and
 * eliminated during serialization via `opts.do_garbage = 1`.
 */
int vocument_mupdf_sanitize_document(
    const unsigned char *bytes,
    size_t len,
    int remove_metadata,
    int remove_attachments,
    int remove_actions,
    int remove_annotations,
    int remove_form_fields,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len,
    int *out_metadata_cleared,
    int *out_xmp_cleared,
    int *out_attachments_removed,
    int *out_actions_removed,
    int *out_annotations_removed,
    int *out_form_fields_removed,
    char *err_buf,
    size_t err_buf_len
) {
    if (!bytes || !out_pdf_bytes || !out_pdf_len) {
        return 3;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;
    if (out_metadata_cleared) *out_metadata_cleared = 0;
    if (out_xmp_cleared) *out_xmp_cleared = 0;
    if (out_attachments_removed) *out_attachments_removed = 0;
    if (out_actions_removed) *out_actions_removed = 0;
    if (out_annotations_removed) *out_annotations_removed = 0;
    if (out_form_fields_removed) *out_form_fields_removed = 0;
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
            pdf_obj *trailer = pdf_trailer(ctx, pdf_doc);
            pdf_obj *catalog = trailer ? pdf_dict_gets(ctx, trailer, "Root") : NULL;

            if (remove_metadata) {
                if (trailer && pdf_dict_gets(ctx, trailer, "Info")) {
                    pdf_dict_dels(ctx, trailer, "Info");
                    if (out_metadata_cleared) *out_metadata_cleared = 1;
                }
                if (catalog) {
                    if (pdf_dict_gets(ctx, catalog, "Metadata")) {
                        pdf_dict_dels(ctx, catalog, "Metadata");
                        if (out_xmp_cleared) *out_xmp_cleared = 1;
                    }
                    if (pdf_dict_gets(ctx, catalog, "PieceInfo")) {
                        pdf_dict_dels(ctx, catalog, "PieceInfo");
                    }
                }
            }

            if (remove_attachments && catalog) {
                pdf_obj *names = pdf_dict_gets(ctx, catalog, "Names");
                if (names && pdf_dict_gets(ctx, names, "EmbeddedFiles")) {
                    pdf_dict_dels(ctx, names, "EmbeddedFiles");
                    if (out_attachments_removed) (*out_attachments_removed)++;
                }
                if (pdf_dict_gets(ctx, catalog, "EmbeddedFiles")) {
                    pdf_dict_dels(ctx, catalog, "EmbeddedFiles");
                    if (out_attachments_removed) (*out_attachments_removed)++;
                }
            }

            if (remove_actions && catalog) {
                if (pdf_dict_gets(ctx, catalog, "OpenAction")) {
                    pdf_dict_dels(ctx, catalog, "OpenAction");
                    if (out_actions_removed) (*out_actions_removed)++;
                }
                if (pdf_dict_gets(ctx, catalog, "AA")) {
                    pdf_dict_dels(ctx, catalog, "AA");
                    if (out_actions_removed) (*out_actions_removed)++;
                }
                pdf_obj *names = pdf_dict_gets(ctx, catalog, "Names");
                if (names && pdf_dict_gets(ctx, names, "JavaScript")) {
                    pdf_dict_dels(ctx, names, "JavaScript");
                    if (out_actions_removed) (*out_actions_removed)++;
                }
            }

            if (remove_form_fields && catalog) {
                if (pdf_dict_gets(ctx, catalog, "AcroForm")) {
                    pdf_dict_dels(ctx, catalog, "AcroForm");
                    if (out_form_fields_removed) (*out_form_fields_removed)++;
                }
            }

            int page_count = pdf_count_pages(ctx, pdf_doc);
            for (int i = 0; i < page_count; i++) {
                pdf_page *page = NULL;
                fz_var(page);
                fz_try(ctx) {
                    page = pdf_load_page(ctx, pdf_doc, i);
                    if (page && page->obj) {
                        if (remove_annotations) {
                            pdf_obj *annots = pdf_dict_gets(ctx, page->obj, "Annots");
                            if (annots) {
                                if (pdf_is_array(ctx, annots)) {
                                    int count = pdf_array_len(ctx, annots);
                                    if (out_annotations_removed) *out_annotations_removed += count;
                                }
                                pdf_dict_dels(ctx, page->obj, "Annots");
                            }
                        }
                        if (remove_actions) {
                            if (pdf_dict_gets(ctx, page->obj, "AA")) {
                                pdf_dict_dels(ctx, page->obj, "AA");
                                if (out_actions_removed) (*out_actions_removed)++;
                            }
                        }
                        if (remove_metadata) {
                            if (pdf_dict_gets(ctx, page->obj, "PieceInfo")) {
                                pdf_dict_dels(ctx, page->obj, "PieceInfo");
                            }
                            if (pdf_dict_gets(ctx, page->obj, "Metadata")) {
                                pdf_dict_dels(ctx, page->obj, "Metadata");
                                if (out_xmp_cleared) *out_xmp_cleared = 1;
                            }
                        }
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
        status = 1;
    }

    fz_drop_context(ctx);
    return status;
}
