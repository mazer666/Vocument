#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

#include "mupdf_serialize.h"
#include "mupdf_page_helpers.h"
#include "mupdf_geometry_algo.h"

void vocument_mupdf_free_bytes(unsigned char *ptr);

/*
 * Reads the 5 ISO 32000-1 page boundary boxes and rotation for a page.
 * MediaBox, CropBox, BleedBox, TrimBox, ArtBox (each 4 floats [x0, y0, x1, y1]).
 * Rotation is an integer in degrees (0, 90, 180, 270).
 */
int vocument_mupdf_get_page_boxes(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    float *out_media_box,
    float *out_crop_box,
    float *out_bleed_box,
    float *out_trim_box,
    float *out_art_box,
    int *out_rotation
) {
    if (!bytes || len == 0 || page_index < 0 || !out_media_box || !out_crop_box ||
        !out_bleed_box || !out_trim_box || !out_art_box || !out_rotation) {
        return 1;
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int page_count = pdf_count_pages(ctx, pdf_doc);
            if (page_index >= page_count) {
                status = 1;
            } else {
                pdf_obj *page_ref = pdf_lookup_page_obj(ctx, pdf_doc, page_index);
                if (!page_ref) {
                    status = 3;
                } else {
                    pdf_obj *media_obj = pdf_dict_get_inheritable(ctx, page_ref, PDF_NAME(MediaBox));
                    fz_rect media_r = media_obj ? pdf_to_rect(ctx, media_obj) : fz_make_rect(0, 0, 612, 792);
                    out_media_box[0] = media_r.x0;
                    out_media_box[1] = media_r.y0;
                    out_media_box[2] = media_r.x1;
                    out_media_box[3] = media_r.y1;

                    pdf_obj *crop_obj = pdf_dict_get_inheritable(ctx, page_ref, PDF_NAME(CropBox));
                    fz_rect crop_r = crop_obj ? pdf_to_rect(ctx, crop_obj) : media_r;
                    out_crop_box[0] = crop_r.x0;
                    out_crop_box[1] = crop_r.y0;
                    out_crop_box[2] = crop_r.x1;
                    out_crop_box[3] = crop_r.y1;

                    pdf_obj *bleed_obj = pdf_dict_get(ctx, page_ref, PDF_NAME(BleedBox));
                    fz_rect bleed_r = bleed_obj ? pdf_to_rect(ctx, bleed_obj) : crop_r;
                    out_bleed_box[0] = bleed_r.x0;
                    out_bleed_box[1] = bleed_r.y0;
                    out_bleed_box[2] = bleed_r.x1;
                    out_bleed_box[3] = bleed_r.y1;

                    pdf_obj *trim_obj = pdf_dict_get(ctx, page_ref, PDF_NAME(TrimBox));
                    fz_rect trim_r = trim_obj ? pdf_to_rect(ctx, trim_obj) : crop_r;
                    out_trim_box[0] = trim_r.x0;
                    out_trim_box[1] = trim_r.y0;
                    out_trim_box[2] = trim_r.x1;
                    out_trim_box[3] = trim_r.y1;

                    pdf_obj *art_obj = pdf_dict_get(ctx, page_ref, PDF_NAME(ArtBox));
                    fz_rect art_r = art_obj ? pdf_to_rect(ctx, art_obj) : crop_r;
                    out_art_box[0] = art_r.x0;
                    out_art_box[1] = art_r.y0;
                    out_art_box[2] = art_r.x1;
                    out_art_box[3] = art_r.y1;

                    pdf_obj *rot_obj = pdf_dict_get_inheritable(ctx, page_ref, PDF_NAME(Rotate));
                    *out_rotation = rot_obj ? pdf_to_int(ctx, rot_obj) : 0;
                }
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
        fz_drop_context(ctx);
    }
    fz_catch(ctx) {
        return 2;
    }
    return status;
}

static void helper_put_box(fz_context *ctx, pdf_document *doc, pdf_obj *page_obj, pdf_obj *box_key, const float *box_rect) {
    if (!box_rect) return;
    pdf_obj *arr = pdf_new_array(ctx, doc, 4);
    pdf_array_push_real(ctx, arr, box_rect[0]);
    pdf_array_push_real(ctx, arr, box_rect[1]);
    pdf_array_push_real(ctx, arr, box_rect[2]);
    pdf_array_push_real(ctx, arr, box_rect[3]);
    pdf_dict_put_drop(ctx, page_obj, box_key, arr);
}

/*
 * Sets page boundary boxes (MediaBox, CropBox, BleedBox, TrimBox, ArtBox)
 * for the specified page indices.
 */
int vocument_mupdf_set_page_boxes_ext(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t num_pages,
    const float *media_box,
    const float *crop_box,
    const float *bleed_box,
    const float *trim_box,
    const float *art_box,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || num_pages == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int active_sigs = count_active_signatures(ctx, pdf_doc);
            if (active_sigs > 0) {
                if (!allow_signature_invalidation) {
                    status = 4; // Fail-closed
                } else {
                    mark_signatures_invalidated(ctx, pdf_doc);
                }
            }

            if (status == 0) {
                int total_pages = pdf_count_pages(ctx, pdf_doc);
                for (size_t i = 0; i < num_pages; i++) {
                    int p_idx = page_indices[i];
                    if (p_idx < 0 || p_idx >= total_pages) {
                        status = 1;
                        break;
                    }
                    pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p_idx);
                    if (!page_obj) {
                        status = 3;
                        break;
                    }
                    make_page_self_contained(ctx, page_obj);

                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(MediaBox), media_box);
                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(CropBox), crop_box);
                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(BleedBox), bleed_box);
                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(TrimBox), trim_box);
                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(ArtBox), art_box);
                }

                if (status == 0) {
                    vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                }
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
        fz_drop_context(ctx);
    }
    fz_catch(ctx) {
        return 2;
    }
    return status;
}

/*
 * Applies a destructive hard crop with clipping path injection.
 */
int vocument_mupdf_apply_hard_crop_ext(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t num_pages,
    const float *crop_rect,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || num_pages == 0 || !crop_rect || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int active_sigs = count_active_signatures(ctx, pdf_doc);
            if (active_sigs > 0) {
                if (!allow_signature_invalidation) {
                    status = 4;
                } else {
                    mark_signatures_invalidated(ctx, pdf_doc);
                }
            }

            if (status == 0) {
                float x0 = crop_rect[0] < crop_rect[2] ? crop_rect[0] : crop_rect[2];
                float y0 = crop_rect[1] < crop_rect[3] ? crop_rect[1] : crop_rect[3];
                float x1 = crop_rect[0] > crop_rect[2] ? crop_rect[0] : crop_rect[2];
                float y1 = crop_rect[1] > crop_rect[3] ? crop_rect[1] : crop_rect[3];
                float w = x1 - x0;
                float h = y1 - y0;

                int total_pages = pdf_count_pages(ctx, pdf_doc);
                for (size_t i = 0; i < num_pages; i++) {
                    int p_idx = page_indices[i];
                    if (p_idx < 0 || p_idx >= total_pages) {
                        status = 1;
                        break;
                    }
                    pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p_idx);
                    if (!page_obj) {
                        status = 3;
                        break;
                    }
                    make_page_self_contained(ctx, page_obj);

                    float norm_box[4] = { x0, y0, x1, y1 };
                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(MediaBox), norm_box);
                    helper_put_box(ctx, pdf_doc, page_obj, PDF_NAME(CropBox), norm_box);

                    char prefix_str[128];
                    snprintf(prefix_str, sizeof(prefix_str), "q %.4f %.4f %.4f %.4f re W n\n", x0, y0, w, h);
                    fz_buffer *pre_buf = fz_new_buffer_from_copied_data(ctx, (unsigned char *)prefix_str, strlen(prefix_str));
                    pdf_obj *pre_obj = pdf_add_stream(ctx, pdf_doc, pre_buf, NULL, 0);
                    fz_drop_buffer(ctx, pre_buf);

                    const char *post_str = "\nQ\n";
                    fz_buffer *post_buf = fz_new_buffer_from_copied_data(ctx, (unsigned char *)post_str, strlen(post_str));
                    pdf_obj *post_obj = pdf_add_stream(ctx, pdf_doc, post_buf, NULL, 0);
                    fz_drop_buffer(ctx, post_buf);

                    pdf_obj *contents = pdf_dict_get(ctx, page_obj, PDF_NAME(Contents));
                    if (contents) {
                        pdf_obj *new_contents = pdf_new_array(ctx, pdf_doc, 3);
                        pdf_array_push(ctx, new_contents, pre_obj);
                        if (pdf_is_array(ctx, contents)) {
                            int num_c = pdf_array_len(ctx, contents);
                            for (int ci = 0; ci < num_c; ci++) {
                                pdf_array_push(ctx, new_contents, pdf_array_get(ctx, contents, ci));
                            }
                        } else {
                            pdf_array_push(ctx, new_contents, contents);
                        }
                        pdf_array_push(ctx, new_contents, post_obj);
                        pdf_dict_put_drop(ctx, page_obj, PDF_NAME(Contents), new_contents);
                    }

                    // Annotations filtering
                    pdf_obj *annots = pdf_dict_get(ctx, page_obj, PDF_NAME(Annots));
                    if (annots && pdf_is_array(ctx, annots)) {
                        for (int ai = pdf_array_len(ctx, annots) - 1; ai >= 0; ai--) {
                            pdf_obj *annot = pdf_array_get(ctx, annots, ai);
                            pdf_obj *rect_obj = pdf_dict_get(ctx, annot, PDF_NAME(Rect));
                            if (rect_obj) {
                                fz_rect ar = pdf_to_rect(ctx, rect_obj);
                                if (ar.x1 <= x0 || ar.x0 >= x1 || ar.y1 <= y0 || ar.y0 >= y1) {
                                    pdf_array_delete(ctx, annots, ai);
                                } else {
                                    float clamped[4] = {
                                        ar.x0 < x0 ? x0 : ar.x0,
                                        ar.y0 < y0 ? y0 : ar.y0,
                                        ar.x1 > x1 ? x1 : ar.x1,
                                        ar.y1 > y1 ? y1 : ar.y1
                                    };
                                    helper_put_box(ctx, pdf_doc, annot, PDF_NAME(Rect), clamped);
                                }
                            }
                        }
                    }
                }

                if (status == 0) {
                    vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                }
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
        fz_drop_context(ctx);
    }
    fz_catch(ctx) {
        return 2;
    }
    return status;
}

/*
 * Detects document deskew angle on a given page.
 */
int vocument_mupdf_detect_deskew_angle(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    float *out_angle_degrees
) {
    if (!bytes || len == 0 || page_index < 0 || !out_angle_degrees) {
        return 1;
    }
    *out_angle_degrees = 0.0f;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int page_count = pdf_count_pages(ctx, pdf_doc);
            if (page_index >= page_count) {
                status = 1;
            } else {
                status = vocument_algo_detect_deskew_angle(ctx, pdf_doc, page_index, out_angle_degrees);
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
        fz_drop_context(ctx);
    }
    fz_catch(ctx) {
        return 2;
    }
    return status;
}

/*
 * Applies fine deskew rotation to the specified pages using a vector transformation matrix.
 */
int vocument_mupdf_apply_deskew_ext(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t num_pages,
    float angle_degrees,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || num_pages == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    if (fabs(angle_degrees) < 0.05f) {
        unsigned char *copy = (unsigned char *)malloc(len);
        if (!copy) return 3;
        memcpy(copy, bytes, len);
        *out_pdf_bytes = copy;
        *out_pdf_len = len;
        return 0;
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int active_sigs = count_active_signatures(ctx, pdf_doc);
            if (active_sigs > 0) {
                if (!allow_signature_invalidation) {
                    status = 4;
                } else {
                    mark_signatures_invalidated(ctx, pdf_doc);
                }
            }

            if (status == 0) {
                double rad = -(double)angle_degrees * M_PI / 180.0;
                double c = cos(rad);
                double s = sin(rad);

                int total_pages = pdf_count_pages(ctx, pdf_doc);
                for (size_t i = 0; i < num_pages; i++) {
                    int p_idx = page_indices[i];
                    if (p_idx < 0 || p_idx >= total_pages) {
                        status = 1;
                        break;
                    }
                    pdf_obj *page_obj = pdf_lookup_page_obj(ctx, pdf_doc, p_idx);
                    if (!page_obj) {
                        status = 3;
                        break;
                    }
                    make_page_self_contained(ctx, page_obj);

                    pdf_obj *media_obj = pdf_dict_get_inheritable(ctx, page_obj, PDF_NAME(MediaBox));
                    fz_rect media_r = media_obj ? pdf_to_rect(ctx, media_obj) : fz_make_rect(0, 0, 612, 792);
                    double cx = (media_r.x0 + media_r.x1) * 0.5;
                    double cy = (media_r.y0 + media_r.y1) * 0.5;

                    double tx = cx - cx * c + cy * s;
                    double ty = cy - cx * s - cy * c;

                    char cm_str[128];
                    snprintf(cm_str, sizeof(cm_str), "q %.6f %.6f %.6f %.6f %.4f %.4f cm\n", c, s, -s, c, tx, ty);
                    fz_buffer *pre_buf = fz_new_buffer_from_copied_data(ctx, (unsigned char *)cm_str, strlen(cm_str));
                    pdf_obj *pre_obj = pdf_add_stream(ctx, pdf_doc, pre_buf, NULL, 0);
                    fz_drop_buffer(ctx, pre_buf);

                    const char *post_str = "\nQ\n";
                    fz_buffer *post_buf = fz_new_buffer_from_copied_data(ctx, (unsigned char *)post_str, strlen(post_str));
                    pdf_obj *post_obj = pdf_add_stream(ctx, pdf_doc, post_buf, NULL, 0);
                    fz_drop_buffer(ctx, post_buf);

                    pdf_obj *contents = pdf_dict_get(ctx, page_obj, PDF_NAME(Contents));
                    if (contents) {
                        pdf_obj *new_contents = pdf_new_array(ctx, pdf_doc, 3);
                        pdf_array_push(ctx, new_contents, pre_obj);
                        if (pdf_is_array(ctx, contents)) {
                            int num_c = pdf_array_len(ctx, contents);
                            for (int ci = 0; ci < num_c; ci++) {
                                pdf_array_push(ctx, new_contents, pdf_array_get(ctx, contents, ci));
                            }
                        } else {
                            pdf_array_push(ctx, new_contents, contents);
                        }
                        pdf_array_push(ctx, new_contents, post_obj);
                        pdf_dict_put_drop(ctx, page_obj, PDF_NAME(Contents), new_contents);
                    }
                }

                if (status == 0) {
                    vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                }
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
        fz_drop_context(ctx);
    }
    fz_catch(ctx) {
        return 2;
    }
    return status;
}

/*
 * Detects content bounding box on a page and returns a trimmed crop rectangle.
 */
int vocument_mupdf_detect_content_margins(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    float safety_margin_pt,
    float *out_crop_rect
) {
    if (!bytes || len == 0 || page_index < 0 || !out_crop_rect) {
        return 1;
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *pdf_doc = pdf_specifics(ctx, doc);
        if (!pdf_doc) {
            status = 2;
        } else {
            int page_count = pdf_count_pages(ctx, pdf_doc);
            if (page_index >= page_count) {
                status = 1;
            } else {
                status = vocument_algo_detect_content_margins(ctx, pdf_doc, page_index, safety_margin_pt, out_crop_rect);
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
        fz_drop_context(ctx);
    }
    fz_catch(ctx) {
        return 2;
    }
    return status;
}
