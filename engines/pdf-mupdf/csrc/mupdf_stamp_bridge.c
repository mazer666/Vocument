/*
 * MuPDF Stamp, Watermark, Header/Footer, Bates Numbering, and Bookmark Bridge (`ROADMAP.md` M3.4).
 * Real native execution using MuPDF C API. Conforms to ISO 32000-1 and CODE-001 (<= 600 lines).
 */

#include "mupdf_stamp_bridge.h"
#include "mupdf_serialize.h"
#include <mupdf/fitz.h>
#include <mupdf/fitz/json.h>
#include <mupdf/pdf.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

static int count_active_signatures(fz_context *ctx, pdf_document *doc) {
    if (!doc) return 0;
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, doc), PDF_NAME(Root));
    if (!root) return 0;
    pdf_obj *acroform = pdf_dict_get(ctx, root, PDF_NAME(AcroForm));
    if (!acroform) return 0;
    pdf_obj *fields = pdf_dict_get(ctx, acroform, PDF_NAME(Fields));
    if (!pdf_is_array(ctx, fields)) return 0;
    int count = 0;
    int len = pdf_array_len(ctx, fields);
    for (int i = 0; i < len; i++) {
        pdf_obj *field = pdf_array_get(ctx, fields, i);
        if (pdf_name_eq(ctx, pdf_dict_get(ctx, field, PDF_NAME(FT)), PDF_NAME(Sig)) &&
            pdf_dict_get(ctx, field, PDF_NAME(V)) != NULL) {
            count++;
        }
    }
    return count;
}

static void ensure_page_resources(fz_context *ctx, pdf_document *doc, pdf_obj *page_obj) {
    pdf_obj *res = pdf_dict_get(ctx, page_obj, PDF_NAME(Resources));
    if (!res) {
        res = pdf_new_dict(ctx, doc, 2);
        pdf_dict_put_drop(ctx, page_obj, PDF_NAME(Resources), res);
    }
    pdf_obj *fonts = pdf_dict_get(ctx, res, PDF_NAME(Font));
    if (!fonts) {
        fonts = pdf_new_dict(ctx, doc, 2);
        pdf_dict_put_drop(ctx, res, PDF_NAME(Font), fonts);
    }
    pdf_obj *name_f1 = pdf_new_name(ctx, "F1");
    if (!pdf_dict_get(ctx, fonts, name_f1)) {
        pdf_obj *f1 = pdf_new_dict(ctx, doc, 4);
        pdf_dict_put(ctx, f1, PDF_NAME(Type), PDF_NAME(Font));
        pdf_dict_put(ctx, f1, PDF_NAME(Subtype), PDF_NAME(Type1));
        pdf_dict_put_drop(ctx, f1, PDF_NAME(BaseFont), pdf_new_name(ctx, "Helvetica"));
        pdf_dict_put_drop(ctx, fonts, name_f1, f1);
    }
    pdf_obj *name_f1b = pdf_new_name(ctx, "F1B");
    if (!pdf_dict_get(ctx, fonts, name_f1b)) {
        pdf_obj *f1b = pdf_new_dict(ctx, doc, 4);
        pdf_dict_put(ctx, f1b, PDF_NAME(Type), PDF_NAME(Font));
        pdf_dict_put(ctx, f1b, PDF_NAME(Subtype), PDF_NAME(Type1));
        pdf_dict_put_drop(ctx, f1b, PDF_NAME(BaseFont), pdf_new_name(ctx, "Helvetica-Bold"));
        pdf_dict_put_drop(ctx, fonts, name_f1b, f1b);
    }
}

static void ensure_extgstate(fz_context *ctx, pdf_document *doc, pdf_obj *page_obj, float opacity, char *out_name, size_t out_len) {
    pdf_obj *res = pdf_dict_get(ctx, page_obj, PDF_NAME(Resources));
    if (!res) {
        res = pdf_new_dict(ctx, doc, 2);
        pdf_dict_put_drop(ctx, page_obj, PDF_NAME(Resources), res);
    }
    pdf_obj *extg = pdf_dict_get(ctx, res, PDF_NAME(ExtGState));
    if (!extg) {
        extg = pdf_new_dict(ctx, doc, 2);
        pdf_dict_put_drop(ctx, res, PDF_NAME(ExtGState), extg);
    }
    int op_int = (int)(opacity * 100.0f);
    snprintf(out_name, out_len, "GS_A%d", op_int);
    pdf_obj *gs_name = pdf_new_name(ctx, out_name);
    if (!pdf_dict_get(ctx, extg, gs_name)) {
        pdf_obj *gs = pdf_new_dict(ctx, doc, 3);
        pdf_dict_put(ctx, gs, PDF_NAME(Type), PDF_NAME(ExtGState));
        pdf_dict_put_real(ctx, gs, PDF_NAME(ca), opacity);
        pdf_dict_put_real(ctx, gs, PDF_NAME(CA), opacity);
        pdf_dict_put_drop(ctx, extg, gs_name, gs);
    }
}

static void escape_pdf_str(const char *in, char *out, size_t out_len) {
    size_t o = 0;
    for (size_t i = 0; in && in[i] && o + 2 < out_len; i++) {
        if (in[i] == '(' || in[i] == ')' || in[i] == '\\') {
            out[o++] = '\\';
        }
        out[o++] = in[i];
    }
    out[o] = '\0';
}

static void inject_content_stream(fz_context *ctx, pdf_document *doc, pdf_obj *page_obj, const char *stream_str, int is_background) {
    fz_buffer *buf = fz_new_buffer_from_copied_data(ctx, (const unsigned char *)stream_str, strlen(stream_str));
    pdf_obj *stream_obj = pdf_add_stream(ctx, doc, buf, NULL, 0);
    fz_drop_buffer(ctx, buf);

    pdf_obj *contents = pdf_dict_get(ctx, page_obj, PDF_NAME(Contents));
    pdf_obj *new_contents = pdf_new_array(ctx, doc, 4);

    if (is_background) {
        pdf_array_push(ctx, new_contents, stream_obj);
    }
    if (contents) {
        if (pdf_is_array(ctx, contents)) {
            int len = pdf_array_len(ctx, contents);
            for (int i = 0; i < len; i++) {
                pdf_array_push(ctx, new_contents, pdf_array_get(ctx, contents, i));
            }
        } else {
            pdf_array_push(ctx, new_contents, contents);
        }
    }
    if (!is_background) {
        pdf_array_push(ctx, new_contents, stream_obj);
    }
    pdf_dict_put_drop(ctx, page_obj, PDF_NAME(Contents), new_contents);
}

int vocument_mupdf_apply_bates_numbering(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const int *page_indices,
    size_t page_indices_len,
    const char *prefix,
    int start_number,
    int digit_padding,
    const char *suffix,
    int position,
    float font_size_pt,
    float margin_pt,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!pdf_bytes || pdf_len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
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

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else if (count_active_signatures(ctx, pdf_doc) > 0 && !allow_signature_invalidation) {
            status = 4;
        } else {
            int total_pages = pdf_count_pages(ctx, pdf_doc);
            if (digit_padding <= 0) digit_padding = 1;
            if (font_size_pt <= 0.0f) font_size_pt = 10.0f;
            if (margin_pt < 0.0f) margin_pt = 36.0f;

            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= total_pages) continue;

                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p);
                if (!page_obj) continue;
                ensure_page_resources(ctx, pdf_doc, page_obj);

                fz_rect box = fz_make_rect(0, 0, 612, 792);
                pdf_obj *crop = pdf_dict_get(ctx, page_obj, PDF_NAME(CropBox));
                if (!crop) crop = pdf_dict_get(ctx, page_obj, PDF_NAME(MediaBox));
                if (crop) box = pdf_to_rect(ctx, crop);

                int num = start_number + (int)i;
                char bates_text[256];
                snprintf(bates_text, sizeof(bates_text), "%s%0*d%s", prefix ? prefix : "", digit_padding, num, suffix ? suffix : "");

                float text_w = (float)strlen(bates_text) * font_size_pt * 0.55f;
                float x = box.x0 + margin_pt;
                float y = box.y0 + margin_pt;
                float page_w = box.x1 - box.x0;

                switch (position) {
                    case VOCUMENT_BATES_TOP_LEFT:
                        x = box.x0 + margin_pt;
                        y = box.y1 - margin_pt - font_size_pt;
                        break;
                    case VOCUMENT_BATES_TOP_CENTER:
                        x = box.x0 + (page_w - text_w) / 2.0f;
                        y = box.y1 - margin_pt - font_size_pt;
                        break;
                    case VOCUMENT_BATES_TOP_RIGHT:
                        x = box.x1 - margin_pt - text_w;
                        y = box.y1 - margin_pt - font_size_pt;
                        break;
                    case VOCUMENT_BATES_BOTTOM_LEFT:
                        x = box.x0 + margin_pt;
                        y = box.y0 + margin_pt;
                        break;
                    case VOCUMENT_BATES_BOTTOM_CENTER:
                        x = box.x0 + (page_w - text_w) / 2.0f;
                        y = box.y0 + margin_pt;
                        break;
                    case VOCUMENT_BATES_BOTTOM_RIGHT:
                    default:
                        x = box.x1 - margin_pt - text_w;
                        y = box.y0 + margin_pt;
                        break;
                }

                char esc[512];
                escape_pdf_str(bates_text, esc, sizeof(esc));

                char op_stream[1024];
                snprintf(op_stream, sizeof(op_stream),
                         "q 0 0 0 rg BT /F1 %.2f Tf 1 0 0 1 %.2f %.2f Tm (%s) Tj ET Q\n",
                         font_size_pt, x, y, esc);

                inject_content_stream(ctx, pdf_doc, page_obj, op_stream, 0);
            }
            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
        }
    }
    fz_always(ctx) {
        if (fz_doc) fz_drop_document(ctx, fz_doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }
    fz_drop_context(ctx);
    return status;
}

static void expand_macros(
    const char *tmpl,
    int page_num,
    int total_pages,
    const char *doc_title,
    const char *date_str,
    char *out,
    size_t out_len
) {
    out[0] = '\0';
    if (!tmpl) return;
    size_t o = 0;
    for (size_t i = 0; tmpl[i] && o + 32 < out_len;) {
        if (strncmp(&tmpl[i], "{page}", 6) == 0) {
            o += snprintf(&out[o], out_len - o, "%d", page_num);
            i += 6;
        } else if (strncmp(&tmpl[i], "{total}", 7) == 0) {
            o += snprintf(&out[o], out_len - o, "%d", total_pages);
            i += 7;
        } else if (strncmp(&tmpl[i], "{date}", 6) == 0) {
            o += snprintf(&out[o], out_len - o, "%s", date_str ? date_str : "");
            i += 6;
        } else if (strncmp(&tmpl[i], "{filename}", 10) == 0) {
            o += snprintf(&out[o], out_len - o, "%s", doc_title ? doc_title : "");
            i += 10;
        } else {
            out[o++] = tmpl[i++];
            out[o] = '\0';
        }
    }
}

int vocument_mupdf_apply_header_footer(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const int *page_indices,
    size_t page_indices_len,
    const char *header_left,
    const char *header_center,
    const char *header_right,
    const char *footer_left,
    const char *footer_center,
    const char *footer_right,
    const char *doc_title,
    const char *date_str,
    float font_size_pt,
    float margin_pt,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!pdf_bytes || pdf_len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
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

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else if (count_active_signatures(ctx, pdf_doc) > 0 && !allow_signature_invalidation) {
            status = 4;
        } else {
            int total_pages = pdf_count_pages(ctx, pdf_doc);
            if (font_size_pt <= 0.0f) font_size_pt = 9.0f;
            if (margin_pt < 0.0f) margin_pt = 36.0f;

            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= total_pages) continue;

                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p);
                if (!page_obj) continue;
                ensure_page_resources(ctx, pdf_doc, page_obj);

                fz_rect box = fz_make_rect(0, 0, 612, 792);
                pdf_obj *crop = pdf_dict_get(ctx, page_obj, PDF_NAME(CropBox));
                if (!crop) crop = pdf_dict_get(ctx, page_obj, PDF_NAME(MediaBox));
                if (crop) box = pdf_to_rect(ctx, crop);

                float page_w = box.x1 - box.x0;
                float header_y = box.y1 - margin_pt - font_size_pt;
                float footer_y = box.y0 + margin_pt;

                char op_stream[4096];
                size_t op_len = 0;
                op_stream[0] = '\0';

                const char *zones[6] = { header_left, header_center, header_right, footer_left, footer_center, footer_right };
                for (int z = 0; z < 6; z++) {
                    if (!zones[z] || !zones[z][0]) continue;
                    char exp[512];
                    expand_macros(zones[z], p + 1, total_pages, doc_title, date_str, exp, sizeof(exp));
                    if (!exp[0]) continue;

                    char esc[512];
                    escape_pdf_str(exp, esc, sizeof(esc));
                    float text_w = (float)strlen(exp) * font_size_pt * 0.55f;

                    float x = box.x0 + margin_pt;
                    float y = (z < 3) ? header_y : footer_y;
                    if (z % 3 == 1) { // Center
                        x = box.x0 + (page_w - text_w) / 2.0f;
                    } else if (z % 3 == 2) { // Right
                        x = box.x1 - margin_pt - text_w;
                    }

                    op_len += snprintf(&op_stream[op_len], sizeof(op_stream) - op_len,
                                       "q 0.2 0.2 0.2 rg BT /F1 %.2f Tf 1 0 0 1 %.2f %.2f Tm (%s) Tj ET Q\n",
                                       font_size_pt, x, y, esc);
                }

                if (op_len > 0) {
                    inject_content_stream(ctx, pdf_doc, page_obj, op_stream, 0);
                }
            }
            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
        }
    }
    fz_always(ctx) {
        if (fz_doc) fz_drop_document(ctx, fz_doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }
    fz_drop_context(ctx);
    return status;
}

static void serialize_outline_item_json(fz_context *ctx, pdf_document *doc, pdf_obj *item, fz_buffer *buf, int *is_first) {
    if (!item || !pdf_is_dict(ctx, item)) return;

    if (!*is_first) fz_append_string(ctx, buf, ",");
    *is_first = 0;

    fz_append_string(ctx, buf, "{\"title\":\"");
    pdf_obj *title_obj = pdf_dict_get(ctx, item, PDF_NAME(Title));
    const char *title = title_obj ? pdf_to_text_string(ctx, title_obj) : "";
    for (size_t i = 0; title && title[i]; i++) {
        if (title[i] == '"' || title[i] == '\\') fz_append_byte(ctx, buf, '\\');
        fz_append_byte(ctx, buf, (unsigned char)title[i]);
    }

    int target_page = 0;
    pdf_obj *dest = pdf_dict_get(ctx, item, PDF_NAME(Dest));
    if (!dest) {
        pdf_obj *a = pdf_dict_get(ctx, item, PDF_NAME(A));
        if (a) dest = pdf_dict_get(ctx, a, PDF_NAME(D));
    }
    if (dest) {
        pdf_obj *res_dest = pdf_resolve_indirect(ctx, dest);
        if (pdf_is_array(ctx, res_dest) && pdf_array_len(ctx, res_dest) > 0) {
            pdf_obj *t = pdf_array_get(ctx, res_dest, 0);
            if (pdf_is_int(ctx, t)) target_page = pdf_to_int(ctx, t);
            else target_page = pdf_lookup_page_number(ctx, doc, t);
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
    fz_buffer *buf = NULL;

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(buf);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            buf = fz_new_buffer(ctx, 1024);
            fz_append_string(ctx, buf, "[");
            pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
            pdf_obj *outlines = root ? pdf_dict_get(ctx, root, PDF_NAME(Outlines)) : NULL;
            if (outlines && pdf_is_dict(ctx, outlines)) {
                pdf_obj *item = pdf_dict_get(ctx, outlines, PDF_NAME(First));
                int is_first = 1;
                while (item && pdf_is_dict(ctx, item)) {
                    serialize_outline_item_json(ctx, pdf_doc, item, buf, &is_first);
                    item = pdf_dict_get(ctx, item, PDF_NAME(Next));
                }
            }
            fz_append_string(ctx, buf, "]");

            unsigned char *storage = NULL;
            size_t len = fz_buffer_storage(ctx, buf, &storage);
            char *json_out = (char *)malloc(len + 1);
            if (json_out) {
                memcpy(json_out, storage, len);
                json_out[len] = '\0';
                *out_json_str = json_out;
            } else {
                status = 3;
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
        pdf_obj *dest = pdf_new_array(ctx, doc, 5);
        pdf_array_push(ctx, dest, target_page_obj);
        pdf_array_push(ctx, dest, PDF_NAME(XYZ));
        pdf_array_push_int(ctx, dest, 0);
        pdf_array_push_int(ctx, dest, 0);
        pdf_array_push_int(ctx, dest, 0);
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
        } else if (count_active_signatures(ctx, pdf_doc) > 0 && !allow_signature_invalidation) {
            status = 4;
        } else {
            pool = fz_new_pool(ctx);
            fz_json *root_json = fz_parse_json(ctx, pool, bookmarks_json);
            if (!root_json || !fz_json_is_array(ctx, root_json)) {
                status = 1;
            } else {
                pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
                pdf_dict_del(ctx, root, PDF_NAME(Outlines));

                int num_top = fz_json_array_length(ctx, root_json);
                if (num_top > 0) {
                    pdf_obj *outlines = pdf_new_dict(ctx, pdf_doc, 4);
                    pdf_obj *outlines_ref = pdf_add_object(ctx, pdf_doc, outlines);
                    pdf_drop_obj(ctx, outlines);

                    pdf_dict_put(ctx, outlines_ref, PDF_NAME(Type), PDF_NAME(Outlines));
                    pdf_dict_put(ctx, root, PDF_NAME(Outlines), outlines_ref);

                    pdf_obj *first_top = NULL;
                    pdf_obj *prev_top = NULL;
                    int total_count = 0;

                    for (int i = 0; i < num_top; i++) {
                        fz_json *item_node = fz_json_array_get(ctx, root_json, i);
                        int sub_count = 0;
                        pdf_obj *top_ref = build_outline_item_from_json(ctx, pdf_doc, item_node, outlines_ref, &sub_count);
                        if (top_ref) {
                            total_count += 1 + sub_count;
                            if (!first_top) first_top = top_ref;
                            if (prev_top) {
                                pdf_dict_put(ctx, prev_top, PDF_NAME(Next), top_ref);
                                pdf_dict_put(ctx, top_ref, PDF_NAME(Prev), prev_top);
                            }
                            prev_top = top_ref;
                        }
                    }
                    if (first_top) pdf_dict_put(ctx, outlines_ref, PDF_NAME(First), first_top);
                    if (prev_top) pdf_dict_put(ctx, outlines_ref, PDF_NAME(Last), prev_top);
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

int vocument_mupdf_apply_stamp_or_watermark(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const int *page_indices,
    size_t page_indices_len,
    const char *stamp_text,
    float opacity,
    float rotation_degrees,
    unsigned char r,
    unsigned char g,
    unsigned char b,
    int is_background,
    float rect_x0,
    float rect_y0,
    float rect_x1,
    float rect_y1,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!pdf_bytes || pdf_len == 0 || !page_indices || page_indices_len == 0 || !stamp_text || !out_pdf_bytes || !out_pdf_len) {
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

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else if (count_active_signatures(ctx, pdf_doc) > 0 && !allow_signature_invalidation) {
            status = 4;
        } else {
            int total_pages = pdf_count_pages(ctx, pdf_doc);
            if (opacity <= 0.0f) opacity = 0.5f;
            if (opacity > 1.0f) opacity = 1.0f;

            char esc[512];
            escape_pdf_str(stamp_text, esc, sizeof(esc));

            float rf = (float)r / 255.0f;
            float gf = (float)g / 255.0f;
            float bf = (float)b / 255.0f;

            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= total_pages) continue;

                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p);
                if (!page_obj) continue;
                ensure_page_resources(ctx, pdf_doc, page_obj);

                char gs_name[32];
                ensure_extgstate(ctx, pdf_doc, page_obj, opacity, gs_name, sizeof(gs_name));

                fz_rect box = fz_make_rect(0, 0, 612, 792);
                pdf_obj *crop = pdf_dict_get(ctx, page_obj, PDF_NAME(CropBox));
                if (!crop) crop = pdf_dict_get(ctx, page_obj, PDF_NAME(MediaBox));
                if (crop) box = pdf_to_rect(ctx, crop);

                float cx = (box.x0 + box.x1) / 2.0f;
                float cy = (box.y0 + box.y1) / 2.0f;
                float font_size = 48.0f;
                float text_w = (float)strlen(stamp_text) * font_size * 0.55f;

                char op_stream[2048];
                float rad = rotation_degrees * (float)M_PI / 180.0f;
                float cos_a = cosf(rad);
                float sin_a = sinf(rad);

                if (rect_x1 > rect_x0 && rect_y1 > rect_y0) {
                    cx = (rect_x0 + rect_x1) / 2.0f;
                    cy = (rect_y0 + rect_y1) / 2.0f;
                    float rw = rect_x1 - rect_x0;
                    font_size = rw / ((float)strlen(stamp_text) * 0.65f);
                    if (font_size > 36.0f) font_size = 36.0f;
                    if (font_size < 12.0f) font_size = 12.0f;
                    text_w = (float)strlen(stamp_text) * font_size * 0.55f;
                }

                snprintf(op_stream, sizeof(op_stream),
                         "q /%s gs %.3f %.3f %.3f rg %.3f %.3f %.3f RG "
                         "%.4f %.4f %.4f %.4f %.2f %.2f cm "
                         "-%.2f -%.2f %.2f %.2f re 2 w S "
                         "BT /F1B %.2f Tf 1 0 0 1 -%.2f -%.2f Tm (%s) Tj ET Q\n",
                         gs_name, rf, gf, bf, rf, gf, bf,
                         cos_a, sin_a, -sin_a, cos_a, cx, cy,
                         (text_w / 2.0f) + 12.0f, (font_size / 2.0f) + 6.0f,
                         text_w + 24.0f, font_size + 12.0f,
                         font_size, text_w / 2.0f, font_size * 0.35f, esc);

                inject_content_stream(ctx, pdf_doc, page_obj, op_stream, is_background);
            }

            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
        }
    }
    fz_always(ctx) {
        if (fz_doc) fz_drop_document(ctx, fz_doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }
    fz_drop_context(ctx);
    return status;
}

void vocument_mupdf_free_string(char *s) {
    if (s) free(s);
}
