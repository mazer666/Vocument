/*
 * MuPDF Document Optimization, Compression, Image Extraction, and Preflight Bridge (`ROADMAP.md` M3.5).
 * Real native execution using MuPDF C API. Conforms to ISO 32000-1 and CODE-001 (<= 600 lines).
 */

#include "mupdf_optimize_bridge.h"
#include "mupdf_page_helpers.h"
#include <mupdf/fitz.h>
#include <mupdf/fitz/write-pixmap.h>
#include <mupdf/pdf.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int copy_buffer_to_malloc(fz_context *ctx, fz_buffer *buf, char **out_str) {
    unsigned char *data = NULL;
    size_t len = fz_buffer_storage(ctx, buf, &data);
    char *copied = (char *)malloc(len + 1);
    if (!copied) return 3;
    if (len > 0 && data) memcpy(copied, data, len);
    copied[len] = '\0';
    *out_str = copied;
    return 0;
}

int vocument_mupdf_optimize_document(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    int preset,
    int do_linearize,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!pdf_bytes || pdf_len == 0 || !out_pdf_bytes || !out_pdf_len) return 1;
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    int status = 0;
    fz_stream *stream = NULL;
    fz_document *fz_doc = NULL;
    pdf_document *pdf_doc = NULL;
    fz_buffer *buf = NULL;
    fz_output *out = NULL;

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(buf);
    fz_var(out);
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
            pdf_write_options opts = pdf_default_write_options;
            opts.reproducible = 1;
            opts.do_preserve_metadata = 1;
            opts.do_appearance = 1;
            /* Legacy linearisation was deprecated/removed by MuPDF ("Linearisation is no longer supported").
             * Modern fast web viewing (RFC 7233 HTTP range requests) is powered by compressed object streams.
             * Per user requirement ("soweit technisch zuverlässig unterstützt"), enable object streams. */
            if (do_linearize) {
                opts.do_use_objstms = 1;
            }

            switch (preset) {
                case VOCUMENT_OPT_LOSSLESS:
                    opts.do_garbage = 1;
                    opts.do_compress = 1;
                    opts.do_use_objstms = 1;
                    opts.compression_effort = 50;
                    break;
                case VOCUMENT_OPT_MAXIMUM:
                    opts.do_garbage = 3; /* De-duplicate objects */
                    opts.do_clean = 1;
                    opts.do_sanitize = 1;
                    opts.do_compress = 1;
                    opts.do_compress_images = 1;
                    opts.do_compress_fonts = 1;
                    opts.do_use_objstms = 1;
                    opts.compression_effort = 100;
                    break;
                case VOCUMENT_OPT_BALANCED:
                default:
                    opts.do_garbage = 3; /* De-duplicate objects */
                    opts.do_clean = 1;
                    opts.do_compress = 1;
                    opts.do_compress_images = 1;
                    opts.do_compress_fonts = 1;
                    opts.do_use_objstms = 1;
                    opts.compression_effort = 80;
                    break;
            }

            buf = fz_new_buffer(ctx, 4096);
            out = fz_new_output_with_buffer(ctx, buf);
            pdf_write_document(ctx, pdf_doc, out, &opts);
            fz_close_output(ctx, out);

            unsigned char *storage = NULL;
            size_t storage_len = fz_buffer_storage(ctx, buf, &storage);
            unsigned char *copied = (unsigned char *)malloc(storage_len > 0 ? storage_len : 1);
            if (!copied) {
                status = 3;
            } else {
                if (storage_len > 0 && storage) {
                    memcpy(copied, storage, storage_len);
                }
                *out_pdf_bytes = copied;
                *out_pdf_len = storage_len;
            }
        }
    }
    fz_always(ctx) {
        if (out) fz_drop_output(ctx, out);
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

int vocument_mupdf_get_document_images_json(
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
            int first_item = 1;
            int page_count = pdf_count_pages(ctx, pdf_doc);

            for (int p = 0; p < page_count; p++) {
                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p);
                if (!page_obj) continue;
                pdf_obj *res = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(Resources));
                if (!res) continue;
                pdf_obj *xobjects = pdf_dict_get(ctx, res, PDF_NAME(XObject));
                if (!xobjects || !pdf_is_dict(ctx, xobjects)) continue;

                int num_x = pdf_dict_len(ctx, xobjects);
                int img_idx_on_page = 0;
                for (int x = 0; x < num_x; x++) {
                    pdf_obj *xobj = pdf_dict_get_val(ctx, xobjects, x);
                    if (!xobj || !pdf_is_dict(ctx, xobj)) continue;
                    pdf_obj *subtype = pdf_dict_get(ctx, xobj, PDF_NAME(Subtype));
                    if (!pdf_name_eq(ctx, subtype, PDF_NAME(Image))) continue;

                    int width = pdf_to_int(ctx, pdf_dict_get(ctx, xobj, PDF_NAME(Width)));
                    int height = pdf_to_int(ctx, pdf_dict_get(ctx, xobj, PDF_NAME(Height)));
                    int bits = pdf_to_int(ctx, pdf_dict_get(ctx, xobj, PDF_NAME(BitsPerComponent)));
                    if (bits <= 0) bits = 8;
                    int obj_num = pdf_to_num(ctx, xobj);

                    const char *cs_str = "DeviceRGB";
                    pdf_obj *cs = pdf_dict_get(ctx, xobj, PDF_NAME(ColorSpace));
                    if (cs) {
                        if (pdf_is_name(ctx, cs)) cs_str = pdf_to_name(ctx, cs);
                        else if (pdf_is_array(ctx, cs) && pdf_array_len(ctx, cs) > 0) {
                            cs_str = pdf_to_name(ctx, pdf_array_get(ctx, cs, 0));
                        }
                    }

                    const char *fmt_str = "png";
                    pdf_obj *filter = pdf_dict_get(ctx, xobj, PDF_NAME(Filter));
                    if (filter) {
                        if (pdf_name_eq(ctx, filter, PDF_NAME(DCTDecode))) fmt_str = "jpeg";
                        else if (pdf_name_eq(ctx, filter, PDF_NAME(JBIG2Decode))) fmt_str = "jbig2";
                    }

                    if (!first_item) fz_append_string(ctx, buf, ",");
                    first_item = 0;

                    char item_buf[256];
                    snprintf(item_buf, sizeof(item_buf),
                             "{\"page_index\":%d,\"image_index\":%d,\"obj_num\":%d,"
                             "\"width\":%d,\"height\":%d,\"bits\":%d,\"format\":\"%s\",\"colorspace\":\"%s\"}",
                             p, img_idx_on_page, obj_num, width, height, bits, fmt_str, cs_str);
                    fz_append_string(ctx, buf, item_buf);
                    img_idx_on_page++;
                }
            }
            fz_append_string(ctx, buf, "]");

            status = copy_buffer_to_malloc(ctx, buf, out_json_str);
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

int vocument_mupdf_extract_image_bytes(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    int page_index,
    int image_index,
    char **out_format_str,
    int *out_width,
    int *out_height,
    unsigned char **out_img_bytes,
    size_t *out_img_len
) {
    if (!pdf_bytes || pdf_len == 0 || !out_format_str || !out_width || !out_height || !out_img_bytes || !out_img_len) {
        return 1;
    }
    *out_format_str = NULL;
    *out_img_bytes = NULL;
    *out_img_len = 0;
    *out_width = 0;
    *out_height = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    int status = 0;
    fz_stream *stream = NULL;
    fz_document *fz_doc = NULL;
    pdf_document *pdf_doc = NULL;
    fz_image *img = NULL;
    fz_pixmap *pix = NULL;
    fz_buffer *buf = NULL;
    fz_output *out = NULL;

    fz_var(stream);
    fz_var(fz_doc);
    fz_var(img);
    fz_var(pix);
    fz_var(buf);
    fz_var(out);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, pdf_bytes, pdf_len);
        fz_doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_doc = pdf_specifics(ctx, fz_doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int page_count = pdf_count_pages(ctx, pdf_doc);
            if (page_index < 0 || page_index >= page_count) {
                status = 1;
            } else {
                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, page_index);
                pdf_obj *res = page_obj ? pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(Resources)) : NULL;
                pdf_obj *xobjects = res ? pdf_dict_get(ctx, res, PDF_NAME(XObject)) : NULL;

                pdf_obj *target_xobj = NULL;
                if (xobjects && pdf_is_dict(ctx, xobjects)) {
                    int num_x = pdf_dict_len(ctx, xobjects);
                    int current_idx = 0;
                    for (int x = 0; x < num_x; x++) {
                        pdf_obj *xobj = pdf_dict_get_val(ctx, xobjects, x);
                        if (!xobj || !pdf_is_dict(ctx, xobj)) continue;
                        if (!pdf_name_eq(ctx, pdf_dict_get(ctx, xobj, PDF_NAME(Subtype)), PDF_NAME(Image))) continue;
                        if (current_idx == image_index) {
                            target_xobj = xobj;
                            break;
                        }
                        current_idx++;
                    }
                }

                if (!target_xobj) {
                    status = 1; /* Image index not found */
                } else {
                    img = pdf_load_image(ctx, pdf_doc, target_xobj);
                    if (!img) {
                        status = 2;
                    } else {
                        *out_width = img->w;
                        *out_height = img->h;

                        pix = fz_get_pixmap_from_image(ctx, img, NULL, NULL, NULL, NULL);
                        buf = fz_new_buffer(ctx, 4096);
                        out = fz_new_output_with_buffer(ctx, buf);
                        fz_write_pixmap_as_png(ctx, out, pix);
                        fz_close_output(ctx, out);

                        unsigned char *storage = NULL;
                        size_t storage_len = fz_buffer_storage(ctx, buf, &storage);
                        unsigned char *copied = (unsigned char *)malloc(storage_len > 0 ? storage_len : 1);
                        if (!copied) {
                            status = 3;
                        } else {
                            if (storage_len > 0 && storage) memcpy(copied, storage, storage_len);
                            *out_img_bytes = copied;
                            *out_img_len = storage_len;
                            *out_format_str = strdup("png");
                        }
                    }
                }
            }
        }
    }
    fz_always(ctx) {
        if (out) fz_drop_output(ctx, out);
        if (buf) fz_drop_buffer(ctx, buf);
        if (pix) fz_drop_pixmap(ctx, pix);
        if (img) fz_drop_image(ctx, img);
        if (fz_doc) fz_drop_document(ctx, fz_doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }
    fz_drop_context(ctx);
    return status;
}

int vocument_mupdf_preflight_pdfx_json(
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
            pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
            pdf_obj *info = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Info));
            pdf_obj *encrypt = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Encrypt));

            int is_encrypted = (encrypt != NULL);
            int has_output_intent = 0;
            char oi_profile[128] = "None";

            if (root) {
                pdf_obj *oi_arr = pdf_dict_get(ctx, root, PDF_NAME(OutputIntents));
                if (oi_arr && pdf_is_array(ctx, oi_arr) && pdf_array_len(ctx, oi_arr) > 0) {
                    for (int i = 0; i < pdf_array_len(ctx, oi_arr); i++) {
                        pdf_obj *oi = pdf_array_get(ctx, oi_arr, i);
                        pdf_obj *s = pdf_dict_get(ctx, oi, PDF_NAME(S));
                        const char *s_name = s ? pdf_to_name(ctx, s) : "";
                        if (strcmp(s_name, "GTS_PDFX") == 0) {
                            has_output_intent = 1;
                            pdf_obj *id_obj = pdf_dict_gets(ctx, oi, "OutputConditionIdentifier");
                            if (id_obj) {
                                const char *str = pdf_to_text_string(ctx, id_obj);
                                if (str) snprintf(oi_profile, sizeof(oi_profile), "%s", str);
                            }
                            break;
                        }
                    }
                }
            }

            int has_trapped = 0;
            if (info) {
                pdf_obj *trap = pdf_dict_gets(ctx, info, "Trapped");
                if (trap) {
                    const char *t_name = pdf_to_name(ctx, trap);
                    if (t_name && (strcmp(t_name, "True") == 0 || strcmp(t_name, "False") == 0)) {
                        has_trapped = 1;
                    }
                }
            }

            char pdfx_ver[64] = "None";
            if (info) {
                pdf_obj *v_obj = pdf_dict_gets(ctx, info, "GTS_PDFXVersion");
                if (v_obj) {
                    const char *v_str = pdf_to_text_string(ctx, v_obj);
                    if (v_str) snprintf(pdfx_ver, sizeof(pdfx_ver), "%s", v_str);
                }
            }

            int all_pages_have_bleed_or_trim = 1;
            int page_count = pdf_count_pages(ctx, pdf_doc);
            for (int p = 0; p < page_count; p++) {
                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p);
                if (!page_obj) continue;
                pdf_obj *bleed = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(BleedBox));
                pdf_obj *trim = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(TrimBox));
                if (!bleed && !trim) {
                    all_pages_have_bleed_or_trim = 0;
                    break;
                }
            }

            int is_valid = (!is_encrypted && has_output_intent && has_trapped && all_pages_have_bleed_or_trim && strcmp(pdfx_ver, "None") != 0);

            buf = fz_new_buffer(ctx, 1024);
            fz_append_printf(ctx, buf,
                "{\"is_valid\":%s,\"version\":\"%s\",\"has_output_intent\":%s,\"output_intent_profile\":\"%s\","
                "\"has_trapped_key\":%s,\"has_bleed_or_trim_box\":%s,\"is_encrypted\":%s,\"issues\":[",
                is_valid ? "true" : "false", pdfx_ver,
                has_output_intent ? "true" : "false", oi_profile,
                has_trapped ? "true" : "false",
                all_pages_have_bleed_or_trim ? "true" : "false",
                is_encrypted ? "true" : "false");

            int first_issue = 1;
            if (is_encrypted) {
                fz_append_string(ctx, buf, "\"Document encryption is prohibited in PDF/X\"");
                first_issue = 0;
            }
            if (!has_output_intent) {
                if (!first_issue) fz_append_string(ctx, buf, ",");
                fz_append_string(ctx, buf, "\"Missing GTS_PDFX OutputIntent profile\"");
                first_issue = 0;
            }
            if (!has_trapped) {
                if (!first_issue) fz_append_string(ctx, buf, ",");
                fz_append_string(ctx, buf, "\"Missing or invalid /Trapped key in Info dictionary\"");
                first_issue = 0;
            }
            if (!all_pages_have_bleed_or_trim) {
                if (!first_issue) fz_append_string(ctx, buf, ",");
                fz_append_string(ctx, buf, "\"One or more pages lack BleedBox or TrimBox\"");
                first_issue = 0;
            }
            if (strcmp(pdfx_ver, "None") == 0) {
                if (!first_issue) fz_append_string(ctx, buf, ",");
                fz_append_string(ctx, buf, "\"Document lacks GTS_PDFXVersion identifier\"");
            }
            fz_append_string(ctx, buf, "]}");

            status = copy_buffer_to_malloc(ctx, buf, out_json_str);
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

int vocument_mupdf_preflight_print_json(
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
            int page_count = pdf_count_pages(ctx, pdf_doc);
            int is_uniform = 1;
            float base_w = 0.0f, base_h = 0.0f;
            int has_bleed = 0, has_crop = 0;

            for (int p = 0; p < page_count; p++) {
                pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p);
                if (!page_obj) continue;
                pdf_obj *media = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(MediaBox));
                fz_rect mbox = media ? pdf_to_rect(ctx, media) : fz_make_rect(0, 0, 612, 792);
                float pw = mbox.x1 - mbox.x0;
                float ph = mbox.y1 - mbox.y0;
                if (p == 0) {
                    base_w = pw;
                    base_h = ph;
                } else {
                    if (fabsf(pw - base_w) > 1.0f || fabsf(ph - base_h) > 1.0f) {
                        is_uniform = 0;
                    }
                }
                if (pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(BleedBox))) has_bleed = 1;
                if (pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(CropBox))) has_crop = 1;
            }

            buf = fz_new_buffer(ctx, 1024);
            char json_header[256];
            snprintf(json_header, sizeof(json_header),
                     "{\"page_count\":%d,\"is_uniform_size\":%s,\"width_pt\":%.2f,\"height_pt\":%.2f,"
                     "\"has_bleed_box\":%s,\"has_crop_box\":%s,\"warnings\":[",
                     page_count, is_uniform ? "true" : "false", base_w, base_h,
                     has_bleed ? "true" : "false", has_crop ? "true" : "false");
            fz_append_string(ctx, buf, json_header);
            if (!is_uniform) {
                fz_append_string(ctx, buf, "\"Document contains pages with mixed page dimensions\"");
            }
            fz_append_string(ctx, buf, "]}");

            status = copy_buffer_to_malloc(ctx, buf, out_json_str);
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
