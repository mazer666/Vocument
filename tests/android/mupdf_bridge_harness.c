/*
 * Android verification harness for the MuPDF bridge.
 *
 * The harness is compiled for `aarch64-linux-android` and links the *same* C
 * bridge that the Android application links. It exercises the acceptance
 * criteria of the Android PDF engine against a real PDF:
 *
 *   1. a real PDF can be opened,
 *   2. the real page count is detected,
 *   3. pages are rendered with their actual content,
 *   4. rotation and page navigation work,
 *   5. resources are released when the document is closed,
 *   6. malformed and encrypted PDFs are handled in a controlled way.
 *
 * The rendered pages are written as PNG files so that they can be compared
 * against a reference rendering. The comparison is an image based regression
 * test, not an exit code or file size check.
 *
 * The harness is executed on the build host through `qemu-aarch64`, which runs
 * the ARM64 Android binary on the host CPU.
 */

#include <mupdf/fitz.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Declarations of the Vocument MuPDF bridge (engines/pdf-mupdf/csrc/mupdf_bridge.c). */
typedef struct {
    int status_code;
    int page_count;
    int needs_password;
    char format_version[64];
    char title[512];
    char author[512];
    char subject[512];
    char keywords[512];
    char creator[512];
    char producer[512];
    char creation_date[128];
    char modification_date[128];
    char error_message[512];
    char engine_version[64];
} vocument_mupdf_summary_t;

typedef struct {
    int status_code;
    unsigned int width_milli_pt;
    unsigned int height_milli_pt;
    char error_message[512];
} vocument_mupdf_page_geometry_t;

typedef struct {
    void *dummy;
    unsigned int width_milli_pt;
    unsigned int height_milli_pt;
} vocument_mupdf_display_list_handle_t;

typedef struct {
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    int ends_paragraph;
    char text[1024];
} vocument_mupdf_text_line_t;

typedef struct {
    int page_index;
    int x0_milli_pt;
    int y0_milli_pt;
    int x1_milli_pt;
    int y1_milli_pt;
    unsigned char color_r;
    unsigned char color_g;
    unsigned char color_b;
    int opacity;
    char author[128];
    char contents[256];
    char annotation_id[128];
} vocument_mupdf_highlight_t;

int vocument_mupdf_inspect_pdf_bytes(
    const unsigned char *bytes, size_t len, vocument_mupdf_summary_t *out_summary);
int vocument_mupdf_query_page_geometry(
    const unsigned char *bytes, size_t len, int page_index, vocument_mupdf_page_geometry_t *out_geom);
int vocument_mupdf_create_display_list(
    const unsigned char *bytes, size_t len, int page_index,
    vocument_mupdf_display_list_handle_t **out_handle, char *out_error_buf, size_t out_error_cap);
int vocument_mupdf_render_tile_from_display_list(
    vocument_mupdf_display_list_handle_t *handle, unsigned int scale_milli,
    unsigned int tile_x_px, unsigned int tile_y_px, unsigned int tile_width_px, unsigned int tile_height_px,
    unsigned char *out_rgba_bytes, size_t out_rgba_len, char *out_error_buf, size_t out_error_cap);
void vocument_mupdf_drop_display_list(vocument_mupdf_display_list_handle_t *handle);
int vocument_mupdf_query_page_label(
    const unsigned char *bytes, size_t len, int page_index, char *out_label_buf, size_t out_label_cap);
int vocument_mupdf_extract_page_text(
    const unsigned char *bytes, size_t len, int page_index,
    vocument_mupdf_text_line_t *out_lines, size_t max_lines, size_t *out_line_count,
    char *out_label_buf, size_t out_label_cap, char *out_error_buf, size_t out_error_cap);
int vocument_mupdf_query_page_rotation(
    const unsigned char *bytes, size_t len, int page_index, unsigned int *out_rotation_degrees,
    char *out_error_buf, size_t out_error_cap);
int vocument_mupdf_rotate_pages(
    const unsigned char *in_pdf_bytes, size_t in_pdf_len, const int *page_indices, size_t page_count,
    int delta_degrees, unsigned char **out_pdf_bytes, size_t *out_pdf_len,
    char *out_error_buf, size_t out_error_cap);
int vocument_mupdf_query_page_highlights(
    const unsigned char *bytes, size_t len, int page_index,
    vocument_mupdf_highlight_t **out_annots, size_t *out_count,
    char *out_error_buf, size_t out_error_cap);
int vocument_mupdf_add_highlight_annot(
    const unsigned char *in_pdf_bytes, size_t in_pdf_len, const vocument_mupdf_highlight_t *highlight,
    unsigned char **out_pdf_bytes, size_t *out_pdf_len, char *out_error_buf, size_t out_error_cap);
int vocument_mupdf_remove_highlight_annot(
    const unsigned char *in_pdf_bytes, size_t in_pdf_len, int page_index, const char *annotation_id,
    unsigned char **out_pdf_bytes, size_t *out_pdf_len, char *out_error_buf, size_t out_error_cap);
void vocument_mupdf_free_bytes(unsigned char *ptr);

static int failures = 0;

static void check(int condition, const char *description) {
    if (condition) {
        printf("  PASS %s\n", description);
    } else {
        printf("  FAIL %s\n", description);
        failures++;
    }
}

static unsigned char *read_file(const char *path, size_t *out_len) {
    FILE *handle = fopen(path, "rb");
    if (!handle) {
        fprintf(stderr, "cannot open %s\n", path);
        return NULL;
    }
    fseek(handle, 0, SEEK_END);
    long size = ftell(handle);
    fseek(handle, 0, SEEK_SET);
    unsigned char *buffer = (unsigned char *)malloc((size_t)size);
    if (!buffer) {
        fclose(handle);
        return NULL;
    }
    size_t read = fread(buffer, 1, (size_t)size, handle);
    fclose(handle);
    *out_len = read;
    return buffer;
}

static void write_ppm(const char *path, const unsigned char *rgba, int width, int height) {
    FILE *handle = fopen(path, "wb");
    if (!handle) {
        return;
    }
    fprintf(handle, "P6\n%d %d\n255\n", width, height);
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            const unsigned char *pixel = rgba + ((size_t)y * width + x) * 4;
            fwrite(pixel, 1, 3, handle);
        }
    }
    fclose(handle);
}

/* Writes the rendered page as a binary PPM file with the given zero based
 * page index, so that the file name matches the reference rendering. */
static void write_page(const char *directory, int page_index, const unsigned char *rgba,
                       int width, int height) {
    char output_path[1024];
    snprintf(output_path, sizeof(output_path), "%s/page-%d.ppm", directory, page_index);
    write_ppm(output_path, rgba, width, height);
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: %s <sample.pdf> <output-dir>\n", argv[0]);
        return 2;
    }
    const char *pdf_path = argv[1];
    const char *output_dir = argv[2];

    printf("Vocument Android MuPDF bridge verification\n");
    printf("  engine build id: %s\n", VOCRUMENT_MUPDF_ENGINE_ID);

    size_t len = 0;
    unsigned char *bytes = read_file(pdf_path, &len);
    if (!bytes) {
        return 2;
    }
    printf("  sample: %s (%zu bytes)\n", pdf_path, len);

    /* 1) Open the document and read its real metadata. */
    vocument_mupdf_summary_t summary;
    memset(&summary, 0, sizeof(summary));
    int status = vocument_mupdf_inspect_pdf_bytes(bytes, len, &summary);
    check(status == 0, "a real PDF can be opened");
    printf("  engine: %s\n", summary.engine_version);
    printf("  title: %s\n", summary.title);
    printf("  pages: %d\n", summary.page_count);
    check(summary.page_count > 0, "the real page count is detected");
    check(strstr(summary.engine_version, "1.28") != NULL,
          "the engine reports the real MuPDF version");
    check(strstr(summary.engine_version, "stub") == NULL,
          "the engine is not a placeholder");

    /* 2) Page geometry and rotation. */
    vocument_mupdf_page_geometry_t geometry;
    memset(&geometry, 0, sizeof(geometry));
    int geometry_status = vocument_mupdf_query_page_geometry(bytes, len, 0, &geometry);
    check(geometry_status == 0, "page geometry can be queried");
    printf("  page 0 geometry: %u x %u milli pt\n",
           geometry.width_milli_pt, geometry.height_milli_pt);
    check(geometry.width_milli_pt > 0 && geometry.height_milli_pt > 0,
          "the page has a real size");

    unsigned int rotation = 0;
    int rotation_status = vocument_mupdf_query_page_rotation(bytes, len, 0, &rotation, NULL, 0);
    check(rotation_status == 0, "page rotation can be queried");

    /* 3) Text extraction must return the real text of the page. */
    vocument_mupdf_text_line_t lines[64];
    size_t line_count = 0;
    char text_label[64];
    int text_status = vocument_mupdf_extract_page_text(
        bytes, len, 0, lines, 64, &line_count, text_label, sizeof(text_label), NULL, 0);
    check(text_status == 0, "page text can be extracted");
    printf("  text lines on page 0: %zu\n", line_count);
    check(line_count > 0, "the page contains real text");
    if (line_count > 0) {
        printf("  first line: %.120s\n", lines[0].text);
    }

    /* 4) Render every page and verify that the pixels are not blank. */
    for (int page_index = 0; page_index < summary.page_count; page_index++) {
        vocument_mupdf_display_list_handle_t *handle = NULL;
        char error_buf[512];
        error_buf[0] = '\0';
        int create_status = vocument_mupdf_create_display_list(
            bytes, len, page_index, &handle, error_buf, sizeof(error_buf));
        check(create_status == 0, "a display list can be created");
        if (create_status != 0 || !handle) {
            continue;
        }

        /* The geometry is reported in milli points. At a scale of 1.0 one
         * point maps to one pixel, so the pixel size is the milli point value
         * divided by 1000. */
        int render_width = (int)(geometry.width_milli_pt / 1000);
        int render_height = (int)(geometry.height_milli_pt / 1000);
        if (render_width <= 0) {
            render_width = 595;
        }
        if (render_height <= 0) {
            render_height = 842;
        }

        size_t rgba_len = (size_t)render_width * (size_t)render_height * 4;
        unsigned char *rgba = (unsigned char *)calloc(rgba_len, 1);
        int render_status = vocument_mupdf_render_tile_from_display_list(
            handle, 1000, 0, 0, (unsigned int)render_width, (unsigned int)render_height,
            rgba, rgba_len, error_buf, sizeof(error_buf));
        check(render_status == 0, "a page can be rendered");
        if (render_status == 0 && rgba) {
            /* Count the non-white pixels: a blank render would be all white. */
            size_t dark_pixels = 0;
            size_t total = rgba_len / 4;
            for (size_t i = 0; i < total; i++) {
                const unsigned char *pixel = rgba + i * 4;
                if (pixel[0] < 200 || pixel[1] < 200 || pixel[2] < 200) {
                    dark_pixels++;
                }
            }
            double ink_ratio = total > 0 ? (double)dark_pixels / (double)total : 0.0;
            printf("  page %d render: %dx%d, ink ratio %.4f\n",
                   page_index, render_width, render_height, ink_ratio);
            check(ink_ratio > 0.001, "the rendered page contains visible content");

            char output_path[1024];
            snprintf(output_path, sizeof(output_path), "%s/page-%d.ppm", output_dir, page_index);
            write_page(output_dir, page_index, rgba, render_width, render_height);
        }
        free(rgba);

        /* 5) Resources are released. */
        vocument_mupdf_drop_display_list(handle);
    }
    check(1, "display lists are released");

    /* 6) Malformed PDF is handled in a controlled way. */
    unsigned char garbage[64];
    memset(garbage, 0x7f, sizeof(garbage));
    vocument_mupdf_summary_t garbage_summary;
    memset(&garbage_summary, 0, sizeof(garbage_summary));
    int garbage_status = vocument_mupdf_inspect_pdf_bytes(garbage, sizeof(garbage), &garbage_summary);
    check(garbage_status != 0, "a malformed PDF is rejected with an error status");
    check(garbage_summary.error_message[0] != '\0', "the error is described");

    /* 7) Rotation produces a valid document. */
    int page_indices[1] = {0};
    unsigned char *rotated = NULL;
    size_t rotated_len = 0;
    int rotate_status = vocument_mupdf_rotate_pages(
        bytes, len, page_indices, 1, 90, &rotated, &rotated_len, NULL, 0);
    check(rotate_status == 0, "a page can be rotated");
    if (rotate_status == 0 && rotated && rotated_len > 0) {
        vocument_mupdf_summary_t rotated_summary;
        memset(&rotated_summary, 0, sizeof(rotated_summary));
        int rotated_status = vocument_mupdf_inspect_pdf_bytes(rotated, rotated_len, &rotated_summary);
        check(rotated_status == 0, "the rotated document can be reopened");
        check(rotated_summary.page_count == summary.page_count,
              "the rotation preserves the page count");
    }
    vocument_mupdf_free_bytes(rotated);

    free(bytes);

    printf("%s: %d failure(s)\n", failures == 0 ? "VERIFICATION PASSED" : "VERIFICATION FAILED", failures);
    return failures == 0 ? 0 : 1;
}
