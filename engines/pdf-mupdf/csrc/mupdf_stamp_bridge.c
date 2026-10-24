/*
 * MuPDF Stamp, Watermark, Header/Footer, and Bates Numbering Bridge (`ROADMAP.md` M3.4).
 * Real native execution using MuPDF C API. Conforms to ISO 32000-1 and CODE-001 (<= 600 lines).
 */

#include "mupdf_stamp_bridge.h"
#include "mupdf_serialize.h"
#include "mupdf_page_helpers.h"
#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

static void ensure_page_resources(fz_context *ctx, pdf_document *doc, pdf_obj *page_obj) {
    make_page_self_contained(ctx, page_obj);

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
    pdf_obj *name_f1 = pdf_new_name(ctx, "VocF1");
    if (!pdf_dict_get(ctx, fonts, name_f1)) {
        pdf_obj *f1 = pdf_new_dict(ctx, doc, 5);
        pdf_dict_put(ctx, f1, PDF_NAME(Type), PDF_NAME(Font));
        pdf_dict_put(ctx, f1, PDF_NAME(Subtype), PDF_NAME(Type1));
        pdf_dict_put_drop(ctx, f1, PDF_NAME(BaseFont), pdf_new_name(ctx, "Helvetica"));
        pdf_dict_put(ctx, f1, PDF_NAME(Encoding), PDF_NAME(WinAnsiEncoding));
        pdf_dict_put_drop(ctx, fonts, name_f1, f1);
    }
    pdf_obj *name_f1b = pdf_new_name(ctx, "VocF1B");
    if (!pdf_dict_get(ctx, fonts, name_f1b)) {
        pdf_obj *f1b = pdf_new_dict(ctx, doc, 5);
        pdf_dict_put(ctx, f1b, PDF_NAME(Type), PDF_NAME(Font));
        pdf_dict_put(ctx, f1b, PDF_NAME(Subtype), PDF_NAME(Type1));
        pdf_dict_put_drop(ctx, f1b, PDF_NAME(BaseFont), pdf_new_name(ctx, "Helvetica-Bold"));
        pdf_dict_put(ctx, f1b, PDF_NAME(Encoding), PDF_NAME(WinAnsiEncoding));
        pdf_dict_put_drop(ctx, fonts, name_f1b, f1b);
    }
}

static void ensure_extgstate(
    fz_context *ctx,
    pdf_document *doc,
    pdf_obj *page_obj,
    float opacity,
    char *out_name,
    size_t out_len
) {
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
    snprintf(out_name, out_len, "VocGS_A%d", op_int);
    pdf_obj *gs_name = pdf_new_name(ctx, out_name);
    if (!pdf_dict_get(ctx, extg, gs_name)) {
        pdf_obj *gs = pdf_new_dict(ctx, doc, 3);
        pdf_dict_put(ctx, gs, PDF_NAME(Type), PDF_NAME(ExtGState));
        pdf_dict_put_real(ctx, gs, PDF_NAME(ca), opacity);
        pdf_dict_put_real(ctx, gs, PDF_NAME(CA), opacity);
        pdf_dict_put_drop(ctx, extg, gs_name, gs);
    }
}

static size_t utf8_glyph_count(const char *s) {
    if (!s) return 0;
    size_t count = 0;
    const unsigned char *p = (const unsigned char *)s;
    while (*p) {
        if ((*p & 0xC0) != 0x80) count++;
        p++;
    }
    return count;
}

static void escape_pdf_str(const char *in, char *out, size_t out_len) {
    if (!in || !out || out_len == 0) return;
    size_t o = 0;
    const unsigned char *s = (const unsigned char *)in;
    while (*s && o + 6 < out_len) {
        if (*s < 0x80) {
            if (*s == '(' || *s == ')' || *s == '\\') {
                out[o++] = '\\';
                out[o++] = *s++;
            } else if (*s >= 0x20 && *s <= 0x7E) {
                out[o++] = *s++;
            } else if (*s == '\n') {
                out[o++] = '\\'; out[o++] = 'n'; s++;
            } else if (*s == '\r') {
                out[o++] = '\\'; out[o++] = 'r'; s++;
            } else if (*s == '\t') {
                out[o++] = '\\'; out[o++] = 't'; s++;
            } else {
                o += snprintf(out + o, out_len - o, "\\%03o", *s++);
            }
        } else {
            unsigned int cp = 0;
            int bytes = 0;
            if ((*s & 0xE0) == 0xC0) {
                cp = *s & 0x1F; bytes = 2;
            } else if ((*s & 0xF0) == 0xE0) {
                cp = *s & 0x0F; bytes = 3;
            } else if ((*s & 0xF8) == 0xF0) {
                cp = *s & 0x07; bytes = 4;
            } else {
                s++; continue;
            }
            int valid = 1;
            for (int k = 1; k < bytes; k++) {
                if ((s[k] & 0xC0) != 0x80) { valid = 0; break; }
                cp = (cp << 6) | (s[k] & 0x3F);
            }
            if (!valid) { s++; continue; }
            s += bytes;

            unsigned char w_byte = 0;
            if (cp >= 0xA0 && cp <= 0xFF) {
                w_byte = (unsigned char)cp;
            } else {
                switch (cp) {
                    case 0x20AC: w_byte = 0x80; break; // €
                    case 0x201A: w_byte = 0x82; break; // ‚
                    case 0x0192: w_byte = 0x83; break; // ƒ
                    case 0x201E: w_byte = 0x84; break; // „
                    case 0x2026: w_byte = 0x85; break; // …
                    case 0x2020: w_byte = 0x86; break; // †
                    case 0x2021: w_byte = 0x87; break; // ‡
                    case 0x02C6: w_byte = 0x88; break; // ˆ
                    case 0x2030: w_byte = 0x89; break; // ‰
                    case 0x0160: w_byte = 0x8A; break; // Š
                    case 0x2039: w_byte = 0x8B; break; // ‹
                    case 0x0152: w_byte = 0x8C; break; // Œ
                    case 0x017D: w_byte = 0x8E; break; // Ž
                    case 0x2018: w_byte = 0x91; break; // ‘
                    case 0x2019: w_byte = 0x92; break; // ’
                    case 0x201C: w_byte = 0x93; break; // “
                    case 0x201D: w_byte = 0x94; break; // ”
                    case 0x2022: w_byte = 0x95; break; // •
                    case 0x2013: w_byte = 0x96; break; // –
                    case 0x2014: w_byte = 0x97; break; // —
                    case 0x02DC: w_byte = 0x98; break; // ˜
                    case 0x2122: w_byte = 0x99; break; // ™
                    case 0x0161: w_byte = 0x9A; break; // š
                    case 0x203A: w_byte = 0x9B; break; // ›
                    case 0x0153: w_byte = 0x9C; break; // œ
                    case 0x017E: w_byte = 0x9E; break; // ž
                    case 0x0178: w_byte = 0x9F; break; // Ÿ
                    default: w_byte = '?'; break;
                }
            }
            if (w_byte == '(' || w_byte == ')' || w_byte == '\\') {
                out[o++] = '\\';
                out[o++] = w_byte;
            } else {
                o += snprintf(out + o, out_len - o, "\\%03o", w_byte);
            }
        }
    }
    out[o] = '\0';
}

static void inject_content_stream(
    fz_context *ctx,
    pdf_document *doc,
    pdf_obj *page_obj,
    const char *stream_str,
    int is_background
) {
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
        } else if (count_active_signatures(ctx, pdf_doc) != 0 && !allow_signature_invalidation) {
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
                char num_buf[32];
                snprintf(num_buf, sizeof(num_buf), "%0*d", digit_padding, num);

                char bates_text[256];
                snprintf(bates_text, sizeof(bates_text), "%s%s%s",
                         prefix ? prefix : "", num_buf, suffix ? suffix : "");

                size_t glyphs = utf8_glyph_count(bates_text);
                float text_w = (float)glyphs * font_size_pt * 0.55f;
                float page_w = box.x1 - box.x0;
                float x = box.x0 + margin_pt;
                float y = box.y0 + margin_pt;

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
                         "q 0 0 0 rg BT /VocF1 %.2f Tf 1 0 0 1 %.2f %.2f Tm (%s) Tj ET Q\n",
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
    const char *template_str,
    int page_num,
    int total_pages,
    const char *doc_title,
    const char *date_str,
    char *out,
    size_t out_len
) {
    if (!template_str || !out || out_len == 0) return;
    size_t o = 0;
    for (size_t i = 0; template_str[i] && o + 1 < out_len;) {
        if (template_str[i] == '{') {
            if (strncmp(&template_str[i], "{page}", 6) == 0) {
                o += snprintf(&out[o], out_len - o, "%d", page_num);
                i += 6;
                continue;
            } else if (strncmp(&template_str[i], "{total}", 7) == 0) {
                o += snprintf(&out[o], out_len - o, "%d", total_pages);
                i += 7;
                continue;
            } else if (strncmp(&template_str[i], "{date}", 6) == 0) {
                o += snprintf(&out[o], out_len - o, "%s", date_str ? date_str : "");
                i += 6;
                continue;
            } else if (strncmp(&template_str[i], "{filename}", 10) == 0 ||
                       strncmp(&template_str[i], "{title}", 7) == 0) {
                size_t skip = template_str[i+1] == 'f' ? 10 : 7;
                o += snprintf(&out[o], out_len - o, "%s", doc_title ? doc_title : "");
                i += skip;
                continue;
            }
        }
        out[o++] = template_str[i++];
    }
    out[o] = '\0';
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
        } else if (count_active_signatures(ctx, pdf_doc) != 0 && !allow_signature_invalidation) {
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
                float max_zone_w = (page_w - 2.0f * margin_pt) / 3.0f - 8.0f;

                char op_stream[4096];
                size_t op_len = 0;
                op_stream[0] = '\0';

                const char *zones[6] = { header_left, header_center, header_right, footer_left, footer_center, footer_right };
                for (int z = 0; z < 6; z++) {
                    if (!zones[z] || !zones[z][0]) continue;
                    char exp[512];
                    expand_macros(zones[z], p + 1, total_pages, doc_title, date_str, exp, sizeof(exp));
                    if (!exp[0]) continue;

                    size_t glyphs = utf8_glyph_count(exp);
                    float zone_font_size = font_size_pt;
                    float text_w = (float)glyphs * zone_font_size * 0.55f;
                    if (text_w > max_zone_w && max_zone_w > 20.0f) {
                        zone_font_size = (max_zone_w / (float)glyphs) / 0.55f;
                        if (zone_font_size < 6.0f) zone_font_size = 6.0f;
                        text_w = (float)glyphs * zone_font_size * 0.55f;
                    }

                    char esc[512];
                    escape_pdf_str(exp, esc, sizeof(esc));

                    float x = box.x0 + margin_pt;
                    float y = (z < 3) ? header_y : footer_y;
                    if (z % 3 == 1) {
                        x = box.x0 + (page_w - text_w) / 2.0f;
                    } else if (z % 3 == 2) {
                        x = box.x1 - margin_pt - text_w;
                    }

                    op_len += snprintf(&op_stream[op_len], sizeof(op_stream) - op_len,
                                       "q 0.2 0.2 0.2 rg BT /VocF1 %.2f Tf 1 0 0 1 %.2f %.2f Tm (%s) Tj ET Q\n",
                                       zone_font_size, x, y, esc);
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
        } else if (count_active_signatures(ctx, pdf_doc) != 0 && !allow_signature_invalidation) {
            status = 4;
        } else {
            int total_pages = pdf_count_pages(ctx, pdf_doc);
            if (opacity <= 0.0f) opacity = 0.5f;
            if (opacity > 1.0f) opacity = 1.0f;

            char esc[512];
            escape_pdf_str(stamp_text, esc, sizeof(esc));
            size_t glyphs = utf8_glyph_count(stamp_text);
            if (glyphs == 0) glyphs = 1;

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

                // Scale font size according to page bounds or explicit bounding box
                float max_w = (box.x1 - box.x0) * 0.75f;
                float est_w = (float)glyphs * font_size * 0.55f;
                if (est_w > max_w && max_w > 50.0f) {
                    font_size = max_w / ((float)glyphs * 0.55f);
                    if (font_size < 12.0f) font_size = 12.0f;
                }
                float text_w = (float)glyphs * font_size * 0.55f;

                if (rect_x1 > rect_x0 && rect_y1 > rect_y0) {
                    cx = (rect_x0 + rect_x1) / 2.0f;
                    cy = (rect_y0 + rect_y1) / 2.0f;
                    float rw = rect_x1 - rect_x0;
                    font_size = rw / ((float)glyphs * 0.65f);
                    if (font_size > 36.0f) font_size = 36.0f;
                    if (font_size < 10.0f) font_size = 10.0f;
                    text_w = (float)glyphs * font_size * 0.55f;
                }

                char op_stream[2048];
                float rad = rotation_degrees * (float)M_PI / 180.0f;
                float cos_a = cosf(rad);
                float sin_a = sinf(rad);

                snprintf(op_stream, sizeof(op_stream),
                         "q /%s gs %.3f %.3f %.3f rg %.3f %.3f %.3f RG "
                         "%.4f %.4f %.4f %.4f %.2f %.2f cm "
                         "-%.2f -%.2f %.2f %.2f re 2 w S "
                         "BT /VocF1B %.2f Tf 1 0 0 1 -%.2f -%.2f Tm (%s) Tj ET Q\n",
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
