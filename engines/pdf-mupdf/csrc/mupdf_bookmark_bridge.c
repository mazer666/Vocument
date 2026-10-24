/*
 * MuPDF Hierarchical Bookmark Bridge (`ROADMAP.md` M3.4).
 * Real native execution using MuPDF C API. Conforms to ISO 32000-1 and CODE-001 (<= 600 lines).
 */

#include "mupdf_bookmark_bridge.h"
#include "mupdf_serialize.h"
#include "mupdf_page_helpers.h"
#include <mupdf/fitz.h>
#include <mupdf/fitz/json.h>
#include <mupdf/pdf.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void serialize_outline_item_json(
    fz_context *ctx,
    pdf_document *doc,
    pdf_obj *item,
    fz_buffer *buf,
    int *is_first
) {
    if (!item || !pdf_is_dict(ctx, item)) return;

    if (!*is_first) {
        fz_append_string(ctx, buf, ",");
    }
    *is_first = 0;

    fz_append_string(ctx, buf, "{\"title\":\"");
    pdf_obj *title_obj = pdf_dict_get(ctx, item, PDF_NAME(Title));
    const char *title = title_obj ? pdf_to_text_string(ctx, title_obj) : "";
    for (size_t i = 0; title && title[i]; i++) {
        if (title[i] == '"' || title[i] == '\\') {
            fz_append_byte(ctx, buf, '\\');
        }
        fz_append_byte(ctx, buf, (unsigned char)title[i]);
    }

    int target_page = 0;
    pdf_obj *dest = pdf_dict_get(ctx, item, PDF_NAME(Dest));
    if (!dest) {
        pdf_obj *a = pdf_dict_get(ctx, item, PDF_NAME(A));
        if (a) dest = pdf_dict_get(ctx, a, PDF_NAME(D));
    }
    if (dest) {
        if (pdf_is_name(ctx, dest) || pdf_is_string(ctx, dest)) {
            pdf_obj *named_dest = pdf_lookup_dest(ctx, doc, dest);
            if (named_dest) dest = named_dest;
        }
        pdf_obj *res_dest = pdf_resolve_indirect(ctx, dest);
        if (pdf_is_array(ctx, res_dest) && pdf_array_len(ctx, res_dest) > 0) {
            pdf_obj *t = pdf_array_get(ctx, res_dest, 0);
            if (pdf_is_int(ctx, t)) {
                target_page = pdf_to_int(ctx, t);
            } else {
                target_page = pdf_lookup_page_number(ctx, doc, t);
            }
            if (target_page < 0) target_page = 0;
        }
    }
    char page_str[32];
    snprintf(page_str, sizeof(page_str), "\",\"target_page\":%d,\"children\":[", target_page);
    fz_append_string(ctx, buf, page_str);

    pdf_obj *child = pdf_dict_get(ctx, item, PDF_NAME(First));
    int child_first = 1;
    while (child && pdf_is_dict(ctx, child)) {
        serialize_outline_item_json(ctx, doc, child, buf, &child_first);
        child = pdf_dict_get(ctx, child, PDF_NAME(Next));
    }
    fz_append_string(ctx, buf, "]}");
}

int vocument_mupdf_get_bookmarks_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    char **out_json_str
) {
    if (!pdf_bytes || pdf_len == 0 || !out_json_str) return 1;
    *out_json_str = NULL;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    int status = 0;
    fz_stream *stream = NULL;
    fz_document *fz_doc = NULL;
    pdf_document *pdf_doc = NULL;
    fz_buffer *buf = NULL;

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(buf);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            buf = fz_new_buffer(ctx, 1024);
            fz_append_string(ctx, buf, "[");

            pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
            if (root) {
                pdf_obj *outlines = pdf_dict_get(ctx, root, PDF_NAME(Outlines));
                if (outlines && pdf_is_dict(ctx, outlines)) {
                    pdf_obj *first = pdf_dict_get(ctx, outlines, PDF_NAME(First));
                    int is_first = 1;
                    while (first && pdf_is_dict(ctx, first)) {
                        serialize_outline_item_json(ctx, pdf_doc, first, buf, &is_first);
                        first = pdf_dict_get(ctx, first, PDF_NAME(Next));
                    }
                }
            }
            fz_append_string(ctx, buf, "]");

            unsigned char *data = NULL;
            size_t len = fz_buffer_storage(ctx, buf, &data);
            char *copied = (char *)malloc(len + 1);
            if (!copied) {
                status = 3;
            } else {
                if (len > 0 && data) memcpy(copied, data, len);
                copied[len] = '\0';
                *out_json_str = copied;
            }
        }
    }
    fz_always(ctx) {
        if (buf) fz_drop_buffer(ctx, buf);
        if (fz_doc) fz_drop_document(ctx, fz_doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }
    fz_drop_context(ctx);
    return status;
}

static pdf_obj *build_outline_item_from_json(
    fz_context *ctx,
    pdf_document *doc,
    fz_json *node,
    pdf_obj *parent_obj,
    int *out_descendant_count
) {
    if (!node || !fz_json_is_object(ctx, node)) return NULL;

    const char *title = fz_json_to_string(ctx, fz_json_object_get(ctx, node, "title"));
    if (!title) title = "Bookmark";
    int page_idx = (int)fz_json_to_number(ctx, fz_json_object_get(ctx, node, "target_page"));
    int total_pages = pdf_count_pages(ctx, doc);
    if (page_idx < 0) page_idx = 0;
    if (page_idx >= total_pages) page_idx = total_pages > 0 ? total_pages - 1 : 0;

    pdf_obj *item = pdf_new_dict(ctx, doc, 8);
    pdf_obj *item_ref = pdf_add_object(ctx, doc, item);
    pdf_drop_obj(ctx, item);

    pdf_dict_put_text_string(ctx, item_ref, PDF_NAME(Title), title);
    pdf_dict_put(ctx, item_ref, PDF_NAME(Parent), parent_obj);

    pdf_obj *target_page_obj = pdf_lookup_page_obj(ctx, doc, page_idx);
    if (target_page_obj) {
        pdf_obj *dest = pdf_new_array(ctx, doc, 2);
        pdf_array_push(ctx, dest, target_page_obj);
        pdf_array_push(ctx, dest, PDF_NAME(Fit));
        pdf_dict_put_drop(ctx, item_ref, PDF_NAME(Dest), dest);
    }

    int children_count = 0;
    fz_json *children = fz_json_object_get(ctx, node, "children");
    if (children && fz_json_is_array(ctx, children)) {
        int num_kids = fz_json_array_length(ctx, children);
        pdf_obj *first_kid = NULL;
        pdf_obj *prev_kid = NULL;

        for (int k = 0; k < num_kids; k++) {
            fz_json *child_node = fz_json_array_get(ctx, children, k);
            int sub_count = 0;
            pdf_obj *child_ref = build_outline_item_from_json(ctx, doc, child_node, item_ref, &sub_count);
            if (child_ref) {
                children_count += 1 + sub_count;
                if (!first_kid) first_kid = child_ref;
                if (prev_kid) {
                    pdf_dict_put(ctx, prev_kid, PDF_NAME(Next), child_ref);
                    pdf_dict_put(ctx, child_ref, PDF_NAME(Prev), prev_kid);
                }
                prev_kid = child_ref;
            }
        }
        if (first_kid) pdf_dict_put(ctx, item_ref, PDF_NAME(First), first_kid);
        if (prev_kid) pdf_dict_put(ctx, item_ref, PDF_NAME(Last), prev_kid);
        if (children_count > 0) pdf_dict_put_int(ctx, item_ref, PDF_NAME(Count), children_count);
    }

    if (out_descendant_count) *out_descendant_count = children_count;
    return item_ref;
}

int vocument_mupdf_set_bookmarks_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const char *bookmarks_json,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!pdf_bytes || pdf_len == 0 || !bookmarks_json || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    int status = 0;
    fz_stream *stream = NULL;
    fz_document *fz_doc = NULL;
    pdf_document *pdf_doc = NULL;
    fz_pool *pool = NULL;

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(pool);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else if (count_active_signatures(ctx, pdf_doc) != 0 && !allow_signature_invalidation) {
            status = 4;
        } else {
            pool = fz_new_pool(ctx);
            fz_json *root_json = fz_parse_json(ctx, pool, bookmarks_json);
            if (!root_json || !fz_json_is_array(ctx, root_json)) {
                status = 1;
            } else {
                pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
                int num_roots = fz_json_array_length(ctx, root_json);
                if (num_roots == 0) {
                    pdf_dict_del(ctx, root, PDF_NAME(Outlines));
                } else {
                    pdf_obj *outlines = pdf_new_dict(ctx, pdf_doc, 4);
                    pdf_obj *outlines_ref = pdf_add_object(ctx, pdf_doc, outlines);
                    pdf_drop_obj(ctx, outlines);

                    pdf_dict_put(ctx, outlines_ref, PDF_NAME(Type), PDF_NAME(Outlines));
                    pdf_dict_put(ctx, root, PDF_NAME(Outlines), outlines_ref);

                    pdf_obj *first_root = NULL;
                    pdf_obj *prev_root = NULL;
                    int total_count = 0;

                    for (int i = 0; i < num_roots; i++) {
                        fz_json *node = fz_json_array_get(ctx, root_json, i);
                        int sub_count = 0;
                        pdf_obj *item_ref = build_outline_item_from_json(ctx, pdf_doc, node, outlines_ref, &sub_count);
                        if (item_ref) {
                            total_count += 1 + sub_count;
                            if (!first_root) first_root = item_ref;
                            if (prev_root) {
                                pdf_dict_put(ctx, prev_root, PDF_NAME(Next), item_ref);
                                pdf_dict_put(ctx, item_ref, PDF_NAME(Prev), prev_root);
                            }
                            prev_root = item_ref;
                        }
                    }
                    if (first_root) pdf_dict_put(ctx, outlines_ref, PDF_NAME(First), first_root);
                    if (prev_root) pdf_dict_put(ctx, outlines_ref, PDF_NAME(Last), prev_root);
                    pdf_dict_put_int(ctx, outlines_ref, PDF_NAME(Count), total_count);
                }
                vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
            }
        }
    }
    fz_always(ctx) {
        if (pool) fz_drop_pool(ctx, pool);
        if (fz_doc) fz_drop_document(ctx, fz_doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }
    fz_drop_context(ctx);
    return status;
}
