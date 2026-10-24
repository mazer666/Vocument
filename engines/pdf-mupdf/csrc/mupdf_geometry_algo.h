#ifndef VOCUMENT_MUPDF_GEOMETRY_ALGO_H
#define VOCUMENT_MUPDF_GEOMETRY_ALGO_H

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <math.h>
#include <stdlib.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

/*
 * Detects document deskew angle on a given page using Radon / projection variance.
 * Scans candidate angles in range [-15.0, 15.0] degrees.
 */
static inline int vocument_algo_detect_deskew_angle(
    fz_context *ctx,
    pdf_document *pdf_doc,
    int page_index,
    float *out_angle_degrees
) {
    pdf_page *page = pdf_load_page(ctx, pdf_doc, page_index);
    if (!page) return 3;

    // Render page at 150 DPI for fast and accurate angle detection
    fz_matrix ctm = fz_scale(150.0f / 72.0f, 150.0f / 72.0f);
    fz_pixmap *pix = fz_new_pixmap_from_page(ctx, &page->super, ctm, fz_device_gray(ctx), 0);
    int w = pix->w;
    int h = pix->h;
    unsigned char *samples = pix->samples;

    float best_angle = 0.0f;
    double max_variance = -1.0;

    // Multi-resolution angular search: Coarse pass (-15 to 15 in steps of 0.5)
    for (float a = -15.0f; a <= 15.0f; a += 0.5f) {
        double rad = (double)a * M_PI / 180.0;
        double cos_a = cos(rad);
        double sin_a = sin(rad);

        int num_lines = 32;
        double sum_line_means = 0.0;
        double line_means[32];

        for (int li = 0; li < num_lines; li++) {
            double y_rel = (double)(li + 1) / (double)(num_lines + 1);
            double line_sum = 0.0;
            int samples_per_line = 32;
            for (int si = 0; si < samples_per_line; si++) {
                double x_rel = (double)(si + 1) / (double)(samples_per_line + 1);
                double u = (x_rel - 0.5) * cos_a - (y_rel - 0.5) * sin_a + 0.5;
                double v = (x_rel - 0.5) * sin_a + (y_rel - 0.5) * cos_a + 0.5;

                int px = (int)(u * w);
                int py = (int)(v * h);
                if (px < 0) px = 0;
                if (px >= w) px = w - 1;
                if (py < 0) py = 0;
                if (py >= h) py = h - 1;

                line_sum += samples[py * w + px];
            }
            line_means[li] = line_sum / samples_per_line;
            sum_line_means += line_means[li];
        }

        double mean = sum_line_means / num_lines;
        double variance = 0.0;
        for (int li = 0; li < num_lines; li++) {
            double diff = line_means[li] - mean;
            variance += diff * diff;
        }

        if (variance > max_variance) {
            max_variance = variance;
            best_angle = a;
        }
    }

    // Fine pass (+/- 0.5 deg around peak in steps of 0.1)
    float coarse_best = best_angle;
    for (float a = coarse_best - 0.5f; a <= coarse_best + 0.5f; a += 0.1f) {
        double rad = (double)a * M_PI / 180.0;
        double cos_a = cos(rad);
        double sin_a = sin(rad);
        int num_lines = 32;
        double sum_line_means = 0.0;
        double line_means[32];

        for (int li = 0; li < num_lines; li++) {
            double y_rel = (double)(li + 1) / (double)(num_lines + 1);
            double line_sum = 0.0;
            int samples_per_line = 32;
            for (int si = 0; si < samples_per_line; si++) {
                double x_rel = (double)(si + 1) / (double)(samples_per_line + 1);
                double u = (x_rel - 0.5) * cos_a - (y_rel - 0.5) * sin_a + 0.5;
                double v = (x_rel - 0.5) * sin_a + (y_rel - 0.5) * cos_a + 0.5;
                int px = (int)(u * w);
                int py = (int)(v * h);
                if (px < 0) px = 0;
                if (px >= w) px = w - 1;
                if (py < 0) py = 0;
                if (py >= h) py = h - 1;
                line_sum += samples[py * w + px];
            }
            line_means[li] = line_sum / samples_per_line;
            sum_line_means += line_means[li];
        }
        double mean = sum_line_means / num_lines;
        double variance = 0.0;
        for (int li = 0; li < num_lines; li++) {
            double diff = line_means[li] - mean;
            variance += diff * diff;
        }
        if (variance > max_variance) {
            max_variance = variance;
            best_angle = a;
        }
    }

    // If projection variance is negligible (< 1.0), page lacks text lines; default to 0.0 deg
    if (max_variance < 1.0) {
        best_angle = 0.0f;
    }

    *out_angle_degrees = best_angle;
    fz_drop_pixmap(ctx, pix);
    fz_drop_page(ctx, &page->super);
    return 0;
}

/*
 * Detects content bounding box on a page and returns a trimmed crop rectangle
 * expanded by safety_margin_pt and bounded to MediaBox.
 */
static inline int vocument_algo_detect_content_margins(
    fz_context *ctx,
    pdf_document *pdf_doc,
    int page_index,
    float safety_margin_pt,
    float *out_crop_rect
) {
    pdf_page *page = pdf_load_page(ctx, pdf_doc, page_index);
    if (!page) return 3;

    pdf_obj *page_ref = page->obj;
    pdf_obj *media_obj = pdf_dict_get_inheritable(ctx, page_ref, PDF_NAME(MediaBox));
    fz_rect media_r = media_obj ? pdf_to_rect(ctx, media_obj) : fz_make_rect(0, 0, 612, 792);

    float dpi = 150.0f;
    fz_matrix ctm = fz_scale(dpi / 72.0f, dpi / 72.0f);
    fz_pixmap *pix = fz_new_pixmap_from_page(ctx, &page->super, ctm, fz_device_gray(ctx), 0);
    int w = pix->w;
    int h = pix->h;
    unsigned char *samples = pix->samples;

    // Sample border to find background luminance
    int border_sum = 0;
    int border_count = 0;
    for (int x = 0; x < w; x++) {
        border_sum += samples[x];
        border_sum += samples[(h - 1) * w + x];
        border_count += 2;
    }
    for (int y = 0; y < h; y++) {
        border_sum += samples[y * w];
        border_sum += samples[y * w + (w - 1)];
        border_count += 2;
    }
    int bg_luma = border_count > 0 ? (border_sum / border_count) : 255;

    int min_x = w, max_x = 0;
    int min_y = h, max_y = 0;
    int threshold = 30; // Threshold of luminance difference from background

    for (int y = 0; y < h; y++) {
        for (int x = 0; x < w; x++) {
            int luma = samples[y * w + x];
            if (abs(luma - bg_luma) > threshold) {
                if (x < min_x) min_x = x;
                if (x > max_x) max_x = x;
                if (y < min_y) min_y = y;
                if (y > max_y) max_y = y;
            }
        }
    }

    if (min_x > max_x || min_y > max_y) {
        // Empty/blank content: return MediaBox
        out_crop_rect[0] = media_r.x0;
        out_crop_rect[1] = media_r.y0;
        out_crop_rect[2] = media_r.x1;
        out_crop_rect[3] = media_r.y1;
    } else {
        // Convert pixel bounds back to PDF coordinates
        float scale = 72.0f / dpi;
        float pdf_x0 = media_r.x0 + (float)min_x * scale;
        float pdf_x1 = media_r.x0 + (float)(max_x + 1) * scale;
        float pdf_y1 = media_r.y1 - (float)min_y * scale;
        float pdf_y0 = media_r.y1 - (float)(max_y + 1) * scale;

        // Expand by safety margin
        pdf_x0 -= safety_margin_pt;
        pdf_y0 -= safety_margin_pt;
        pdf_x1 += safety_margin_pt;
        pdf_y1 += safety_margin_pt;

        // Clamp to MediaBox
        if (pdf_x0 < media_r.x0) pdf_x0 = media_r.x0;
        if (pdf_y0 < media_r.y0) pdf_y0 = media_r.y0;
        if (pdf_x1 > media_r.x1) pdf_x1 = media_r.x1;
        if (pdf_y1 > media_r.y1) pdf_y1 = media_r.y1;

        out_crop_rect[0] = pdf_x0;
        out_crop_rect[1] = pdf_y0;
        out_crop_rect[2] = pdf_x1;
        out_crop_rect[3] = pdf_y1;
    }

    fz_drop_pixmap(ctx, pix);
    fz_drop_page(ctx, &page->super);
    return 0;
}

#endif /* VOCUMENT_MUPDF_GEOMETRY_ALGO_H */
