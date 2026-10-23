#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* One embedded file of a document (`ATTACH-001`).
   The engine reports the facts: the name, the type, the size, whether the
   content is really in the PDF and whether the checksum of the embedded file
   matches. The risk policy lives in the core. */
typedef struct {
    char file_name[512];
    char mime_type[256];
    int64_t size_bytes;
    int is_embedded;
    int checksum_is_valid;
} vocument_mupdf_attachment_t;

/* One signature field of a document (`SIGREV-001`).
   The decisive field is `changed_since_signing`: a document that was changed
   after it was signed carries a signature that no longer covers its content. */
typedef struct {
    char field_name[256];
    int is_signed;
    int changed_since_signing;
    char signer[256];
    char signed_at[64];
    char reason[256];
    char location[256];
} vocument_mupdf_signature_t;

static void copy_c_string(char *dst, size_t cap, const char *src) {
    if (!dst || cap == 0) {
        return;
    }
    if (!src) {
        dst[0] = '\0';
        return;
    }
    strncpy(dst, src, cap - 1);
    dst[cap - 1] = '\0';
}

/* Reports the embedded files of the document.
   The files live in the name tree `/Names /EmbeddedFiles`. */
int vocument_mupdf_inspect_attachments(
    const unsigned char *bytes,
    size_t len,
    vocument_mupdf_attachment_t *out_attachments,
    size_t max_attachments,
    size_t *out_attachment_count,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_attachment_count) {
        return 3;
    }
    *out_attachment_count = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_obj *tree = NULL;
    size_t count = 0;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(tree);
    fz_var(count);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 5;
        } else {
            tree = pdf_load_name_tree(ctx, pdf_doc, PDF_NAME(EmbeddedFiles));
            if (tree) {
                int entry_count = pdf_dict_len(ctx, tree);
                int truncated = 0;
                for (int i = 0; i < entry_count; i++) {
                    pdf_obj *key = pdf_dict_get_key(ctx, tree, i);
                    pdf_obj *value = key ? pdf_dict_get(ctx, tree, key) : NULL;
                    if (!pdf_is_dict(ctx, value)) {
                        continue;
                    }
                    if (!out_attachments || count < max_attachments) {
                        if (out_attachments) {
                            pdf_obj *filespec = value;
                            vocument_mupdf_attachment_t *dst = &out_attachments[count];
                            memset(dst, 0, sizeof(*dst));

                            pdf_filespec_params params;
                            memset(&params, 0, sizeof(params));
                            pdf_get_filespec_params(ctx, filespec, &params);
                            copy_c_string(dst->file_name, sizeof(dst->file_name), params.filename);
                            copy_c_string(dst->mime_type, sizeof(dst->mime_type), params.mimetype);
                            dst->size_bytes = params.size;
                            dst->is_embedded = pdf_is_embedded_file(ctx, filespec) ? 1 : 0;
                            dst->checksum_is_valid =
                                pdf_verify_embedded_file_checksum(ctx, filespec) ? 1 : 0;
                        }
                        count++;
                    } else {
                        truncated = 1;
                        count++;
                    }
                }
                *out_attachment_count = count;
                if (truncated) {
                    if (err_buf && err_buf_len > 0) {
                        snprintf(err_buf, err_buf_len,
                            "attachment inspection incomplete: count (%zu) exceeds capacity (%zu)",
                            count, max_attachments);
                    }
                    status = 3;
                } else {
                    status = 0;
                }
            } else {
                *out_attachment_count = 0;
                status = 0;
            }
        }
    }
    fz_always(ctx) {
        if (tree) pdf_drop_obj(ctx, tree);
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

typedef struct SigFieldCycleNode {
    pdf_obj *obj;
    struct SigFieldCycleNode *parent;
} SigFieldCycleNode;

static int sig_cycle_detect(fz_context *ctx, pdf_obj *obj, const SigFieldCycleNode *ancestors) {
    if (!obj) return 0;
    int target_num = pdf_to_num(ctx, obj);
    const SigFieldCycleNode *curr = ancestors;
    while (curr) {
        if (curr->obj == obj) return 1;
        if (target_num != 0 && pdf_to_num(ctx, curr->obj) == target_num) return 1;
        curr = curr->parent;
    }
    return 0;
}

static void extract_signature_field_data(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj *field,
    vocument_mupdf_signature_t *out_signatures,
    size_t max_signatures,
    size_t *count,
    int *truncated
) {
    if (!out_signatures || *count < max_signatures) {
        if (out_signatures) {
            vocument_mupdf_signature_t *dst = &out_signatures[*count];
            memset(dst, 0, sizeof(*dst));

            pdf_obj *title = pdf_dict_get_inheritable(ctx, field, PDF_NAME(T));
            copy_c_string(dst->field_name, sizeof(dst->field_name), pdf_to_text_string(ctx, title));

            dst->is_signed = pdf_signature_is_signed(ctx, pdf_doc, field) ? 1 : 0;

            pdf_obj *signature = pdf_dict_get_inheritable(ctx, field, PDF_NAME(V));
            if (pdf_is_dict(ctx, signature)) {
                copy_c_string(
                    dst->signer, sizeof(dst->signer),
                    pdf_to_text_string(ctx, pdf_dict_get(ctx, signature, PDF_NAME(Name))));
                copy_c_string(
                    dst->reason, sizeof(dst->reason),
                    pdf_to_text_string(ctx, pdf_dict_get(ctx, signature, PDF_NAME(Reason))));
                copy_c_string(
                    dst->location, sizeof(dst->location),
                    pdf_to_text_string(ctx, pdf_dict_get(ctx, signature, PDF_NAME(Location))));

                int64_t signed_at = pdf_to_date(ctx, pdf_dict_get(ctx, signature, PDF_NAME(M)));
                if (signed_at > 0) {
                    char buffer[64];
                    struct tm tm_value;
                    time_t seconds = (time_t)signed_at;
                    if (gmtime_r(&seconds, &tm_value)) {
                        strftime(buffer, sizeof(buffer), "%Y-%m-%dT%H:%M:%SZ", &tm_value);
                        copy_c_string(dst->signed_at, sizeof(dst->signed_at), buffer);
                    }
                }

                dst->changed_since_signing =
                    pdf_signature_incremental_change_since_signing(ctx, pdf_doc, field) ? 1 : 0;
            }
        }
        (*count)++;
    } else {
        *truncated = 1;
        (*count)++;
    }
}

/* Collects the signature fields of the form, recursively checking for cycles and max depth. */
static int collect_signature_fields_rec(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj *field,
    const SigFieldCycleNode *ancestors,
    int depth,
    vocument_mupdf_signature_t *out_signatures,
    size_t max_signatures,
    size_t *count,
    int *truncated
) {
    if (!field || !pdf_is_dict(ctx, field)) return 0;
    if (depth > 32 || sig_cycle_detect(ctx, field, ancestors)) {
        return -1; /* Fail-closed on cyclic or excessively deep hierarchy */
    }

    pdf_obj *kids = pdf_dict_get(ctx, field, PDF_NAME(Kids));
    if (pdf_is_array(ctx, kids) && pdf_array_len(ctx, kids) > 0) {
        SigFieldCycleNode current_node = { field, (SigFieldCycleNode *)ancestors };
        int n_kids = pdf_array_len(ctx, kids);
        for (int k = 0; k < n_kids; k++) {
            pdf_obj *kid = pdf_array_get(ctx, kids, k);
            int res = collect_signature_fields_rec(
                ctx, pdf_doc, kid, &current_node, depth + 1,
                out_signatures, max_signatures, count, truncated);
            if (res < 0) return -1;
        }
        return 0;
    }

    pdf_obj *field_type = pdf_dict_get_inheritable(ctx, field, PDF_NAME(FT));
    if (pdf_name_eq(ctx, field_type, PDF_NAME(Sig))) {
        extract_signature_field_data(
            ctx, pdf_doc, field, out_signatures, max_signatures, count, truncated);
    }
    return 0;
}

/* Reports the signatures of the document.
   The report tells the reviewer whether the document is signed at all, who
   signed it and — the decisive point — whether the document was changed
   after the signature. A signature that no longer covers the content of the
   document is worth nothing. */
int vocument_mupdf_inspect_signatures(
    const unsigned char *bytes,
    size_t len,
    vocument_mupdf_signature_t *out_signatures,
    size_t max_signatures,
    size_t *out_signature_count,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_signature_count) {
        return 3;
    }
    *out_signature_count = 0;
    if (err_buf && err_buf_len > 0) {
        err_buf[0] = '\0';
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    size_t count = 0;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(count);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 5;
        } else {
            int truncated = 0;
            pdf_obj *catalog = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
            pdf_obj *acroform = pdf_dict_get(ctx, catalog, PDF_NAME(AcroForm));
            pdf_obj *fields = pdf_dict_get(ctx, acroform, PDF_NAME(Fields));
            int res = 0;
            if (pdf_is_array(ctx, fields)) {
                int n_fields = pdf_array_len(ctx, fields);
                for (int i = 0; i < n_fields; i++) {
                    pdf_obj *f = pdf_array_get(ctx, fields, i);
                    res = collect_signature_fields_rec(
                        ctx, pdf_doc, f, NULL, 0, out_signatures, max_signatures, &count, &truncated);
                    if (res < 0) break;
                }
            }

            if (res < 0) {
                if (err_buf && err_buf_len > 0) {
                    snprintf(err_buf, err_buf_len,
                        "cyclic or excessively deep form field structure detected; signature inspection fail-closed");
                }
                *out_signature_count = 0;
                status = 2;
            } else {
                *out_signature_count = count;
                if (truncated) {
                    if (err_buf && err_buf_len > 0) {
                        snprintf(err_buf, err_buf_len,
                            "signature inspection incomplete: count (%zu) exceeds capacity (%zu)",
                            count, max_signatures);
                    }
                    status = 3;
                } else {
                    status = 0;
                }
            }
        }
    }
    fz_always(ctx) {
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
