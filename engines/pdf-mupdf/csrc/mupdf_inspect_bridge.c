#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

/* One link or action of a document (`LINKSEC-001`).
   The kind is the name of the action type in the PDF, the target is the
   address, the file, the script or the destination. */
typedef struct {
    int has_page;
    int page_index;
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    char kind[64];
    char target[1024];
} vocument_mupdf_action_t;

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

/* Reads a PDF string or name as text. */
static void read_text_value(fz_context *ctx, pdf_obj *obj, char *dst, size_t cap) {
    dst[0] = '\0';
    if (!obj) {
        return;
    }
    if (pdf_is_string(ctx, obj)) {
        copy_c_string(dst, cap, pdf_to_text_string(ctx, obj));
    } else if (pdf_is_name(ctx, obj)) {
        copy_c_string(dst, cap, pdf_to_name(ctx, obj));
    } else if (pdf_is_array(ctx, obj)) {
        copy_c_string(dst, cap, "[destination]");
    }
}

/* Reads the target of a file specification (`/F`, `/UF`). */
static void read_file_specification(fz_context *ctx, pdf_obj *obj, char *dst, size_t cap) {
    dst[0] = '\0';
    if (!obj) {
        return;
    }
    if (pdf_is_string(ctx, obj) || pdf_is_name(ctx, obj)) {
        read_text_value(ctx, obj, dst, cap);
        return;
    }
    if (pdf_is_dict(ctx, obj)) {
        pdf_obj *unicode = pdf_dict_get(ctx, obj, PDF_NAME(UF));
        if (unicode) {
            read_text_value(ctx, unicode, dst, cap);
            if (dst[0] != '\0') {
                return;
            }
        }
        read_text_value(ctx, pdf_dict_get(ctx, obj, PDF_NAME(F)), dst, cap);
    }
}

/* Describes an action dictionary: its type and its target. */
static void describe_action(
    fz_context *ctx,
    pdf_obj *action,
    char *kind,
    size_t kind_cap,
    char *target,
    size_t target_cap
) {
    copy_c_string(kind, kind_cap, "other");
    target[0] = '\0';

    if (!action) {
        return;
    }
    const char *subtype = pdf_to_name(ctx, pdf_dict_get(ctx, action, PDF_NAME(S)));
    if (!subtype) {
        subtype = "";
    }

    if (!strcmp(subtype, "GoTo")) {
        copy_c_string(kind, kind_cap, "internal_destination");
        read_text_value(ctx, pdf_dict_get(ctx, action, PDF_NAME(D)), target, target_cap);
    } else if (!strcmp(subtype, "GoToR")) {
        copy_c_string(kind, kind_cap, "external_destination");
        read_file_specification(ctx, pdf_dict_get(ctx, action, PDF_NAME(F)), target, target_cap);
    } else if (!strcmp(subtype, "GoToE")) {
        copy_c_string(kind, kind_cap, "external_destination");
        read_file_specification(ctx, pdf_dict_get(ctx, action, PDF_NAME(F)), target, target_cap);
    } else if (!strcmp(subtype, "URI")) {
        copy_c_string(kind, kind_cap, "uri");
        read_text_value(ctx, pdf_dict_get(ctx, action, PDF_NAME(URI)), target, target_cap);
    } else if (!strcmp(subtype, "Launch")) {
        copy_c_string(kind, kind_cap, "launch_file");
        read_file_specification(ctx, pdf_dict_get(ctx, action, PDF_NAME(F)), target, target_cap);
        if (target[0] == '\0') {
            pdf_obj *win = pdf_new_name(ctx, "Win");
            read_file_specification(ctx, pdf_dict_get(ctx, action, win), target, target_cap);
            pdf_drop_obj(ctx, win);
        }
    } else if (!strcmp(subtype, "JavaScript")) {
        copy_c_string(kind, kind_cap, "javascript");
        pdf_obj *script = pdf_dict_get(ctx, action, PDF_NAME(JS));
        if (script && pdf_is_stream(ctx, script)) {
            copy_c_string(target, target_cap, "<script stream>");
        } else {
            read_text_value(ctx, script, target, target_cap);
        }
    } else if (!strcmp(subtype, "SubmitForm")) {
        copy_c_string(kind, kind_cap, "form_submission");
        read_text_value(ctx, pdf_dict_get(ctx, action, PDF_NAME(F)), target, target_cap);
    } else if (!strcmp(subtype, "ImportData")) {
        copy_c_string(kind, kind_cap, "import_data");
        read_file_specification(ctx, pdf_dict_get(ctx, action, PDF_NAME(F)), target, target_cap);
    } else if (!strcmp(subtype, "NamedAction")) {
        copy_c_string(kind, kind_cap, "named_action");
        read_text_value(ctx, pdf_dict_get(ctx, action, PDF_NAME(N)), target, target_cap);
    } else if (!strcmp(subtype, "ResetForm")) {
        copy_c_string(kind, kind_cap, "reset_form");
    } else if (!strcmp(subtype, "Hide")) {
        copy_c_string(kind, kind_cap, "hide_annotations");
    } else if (!strcmp(subtype, "Movie") || !strcmp(subtype, "Sound") ||
               !strcmp(subtype, "Rendition")) {
        copy_c_string(kind, kind_cap, "media");
    } else if (!strcmp(subtype, "SetOCGState") || !strcmp(subtype, "Trans")) {
        copy_c_string(kind, kind_cap, "layer_state");
    } else {
        copy_c_string(kind, kind_cap, "other");
        copy_c_string(target, target_cap, subtype);
    }
}

/* Collects the actions of the document and of its pages.
   The result is what a professional review needs before a user follows a
   link or opens a document: where does it lead, and does it run code
   (`LINKSEC-001`). */
int vocument_mupdf_inspect_document_actions(
    const unsigned char *bytes,
    size_t len,
    vocument_mupdf_action_t *out_actions,
    size_t max_actions,
    size_t *out_action_count,
    char *err_buf,
    size_t err_buf_len
) {
    if (!out_action_count) {
        return 3;
    }
    *out_action_count = 0;
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
    size_t count = 0;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(pdf_doc);
    fz_var(count);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 5;
        } else {
            /* The action that runs when the document is opened. It is the
               most important one: it happens without the user asking. */
            pdf_obj *catalog = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
            pdf_obj *open_action_name = pdf_new_name(ctx, "OpenAction");
            pdf_obj *open_action = pdf_dict_get(ctx, catalog, open_action_name);
            pdf_drop_obj(ctx, open_action_name);
            int truncated = 0;
            if (open_action) {
                if (!out_actions || count < max_actions) {
                    if (out_actions) {
                        vocument_mupdf_action_t *dst = &out_actions[count];
                        memset(dst, 0, sizeof(*dst));
                        dst->has_page = 0;
                        if (pdf_is_dict(ctx, open_action)) {
                            describe_action(ctx, open_action, dst->kind, sizeof(dst->kind),
                                            dst->target, sizeof(dst->target));
                        } else {
                            copy_c_string(dst->kind, sizeof(dst->kind), "internal_destination");
                            read_text_value(ctx, open_action, dst->target, sizeof(dst->target));
                        }
                    }
                    count++;
                } else {
                    truncated = 1;
                    count++;
                }
            }

            /* The links of every page.
               MuPDF does not keep the link annotations in the list of the
               annotations of a page (they are loaded separately as clickable
               areas), and that list carries only the target, not the type of
               the action. The annotations of the page are therefore read
               directly — only a reviewer wants to know that a link runs a
               script. */
            int page_count = pdf_count_pages(ctx, pdf_doc);
            for (int page_index = 0; page_index < page_count; page_index++) {
                pdf_page *page = pdf_load_page(ctx, pdf_doc, page_index);
                if (!page) {
                    continue;
                }
                fz_rect cropbox;
                fz_matrix page_ctm;
                pdf_page_transform(ctx, page, &cropbox, &page_ctm);

                pdf_obj *annots = pdf_dict_get(ctx, page->obj, PDF_NAME(Annots));
                int annot_count = pdf_array_len(ctx, annots);
                for (int i = 0; i < annot_count; i++) {
                    pdf_obj *annot = pdf_array_get(ctx, annots, i);
                    if (!pdf_is_dict(ctx, annot)) {
                        continue;
                    }
                    pdf_obj *subtype = pdf_dict_get(ctx, annot, PDF_NAME(Subtype));
                    if (!pdf_name_eq(ctx, subtype, PDF_NAME(Link))) {
                        continue;
                    }

                    if (!out_actions || count < max_actions) {
                        if (out_actions) {
                            vocument_mupdf_action_t *dst = &out_actions[count];
                            memset(dst, 0, sizeof(*dst));
                            dst->has_page = 1;
                            dst->page_index = page_index;

                            /* The rectangle of the annotation lies in the user space
                               of the PDF (origin at the bottom left); the page space
                               has its origin at the top left. The page matrix moves
                               one into the other. */
                            fz_rect user_rect = fz_empty_rect;
                            pdf_obj *rect = pdf_dict_get(ctx, annot, PDF_NAME(Rect));
                            if (pdf_is_array(ctx, rect) && pdf_array_len(ctx, rect) == 4) {
                                float x0 = pdf_to_real(ctx, pdf_array_get(ctx, rect, 0));
                                float y0 = pdf_to_real(ctx, pdf_array_get(ctx, rect, 1));
                                float x1 = pdf_to_real(ctx, pdf_array_get(ctx, rect, 2));
                                float y1 = pdf_to_real(ctx, pdf_array_get(ctx, rect, 3));
                                user_rect = fz_make_rect(x0, y0, x1, y1);
                            }
                            fz_rect page_rect = fz_transform_rect(user_rect, page_ctm);
                            dst->x0_milli_pt = (int)(page_rect.x0 * 1000.0f);
                            dst->y0_milli_pt = (int)(page_rect.y0 * 1000.0f);
                            dst->x1_milli_pt = (int)(page_rect.x1 * 1000.0f);
                            dst->y1_milli_pt = (int)(page_rect.y1 * 1000.0f);

                            pdf_obj *action = pdf_dict_get(ctx, annot, PDF_NAME(A));
                            if (action && pdf_is_dict(ctx, action)) {
                                describe_action(ctx, action, dst->kind, sizeof(dst->kind),
                                                dst->target, sizeof(dst->target));
                            } else {
                                pdf_obj *dest = pdf_dict_get(ctx, annot, PDF_NAME(Dest));
                                copy_c_string(dst->kind, sizeof(dst->kind), "internal_destination");
                                read_text_value(ctx, dest, dst->target, sizeof(dst->target));
                            }
                        }
                        count++;
                    } else {
                        truncated = 1;
                        count++;
                    }
                }
                pdf_drop_page(ctx, page);
            }
            *out_action_count = count;
            if (truncated) {
                if (err_buf && err_buf_len > 0) {
                    snprintf(err_buf, err_buf_len,
                        "document action inspection incomplete: action count (%zu) exceeds capacity (%zu)",
                        count, max_actions);
                }
                status = 3;
            } else {
                status = 0;
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
