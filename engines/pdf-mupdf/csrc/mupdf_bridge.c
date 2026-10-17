#include <mupdf/fitz.h>
#include <string.h>

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

static void lookup_optional_meta(fz_context *ctx, fz_document *doc, const char *key, char *out_buf, size_t out_size) {
    out_buf[0] = '\0';
    fz_try(ctx) {
        int len = fz_lookup_metadata(ctx, doc, key, out_buf, out_size);
        if (len < 0) {
            out_buf[0] = '\0';
        }
    }
    fz_catch(ctx) {
        out_buf[0] = '\0';
    }
}

/*
 * Status codes:
 *  0 = OK
 *  1 = Password required
 *  2 = Invalid/corrupted PDF or open failure
 *  3 = Context allocation failure
 */
int vocument_mupdf_inspect_pdf_bytes(
    const unsigned char *bytes,
    size_t len,
    vocument_mupdf_summary_t *out_summary
) {
    if (!out_summary) {
        return 3;
    }
    memset(out_summary, 0, sizeof(vocument_mupdf_summary_t));
    strncpy(out_summary->engine_version, FZ_VERSION, sizeof(out_summary->engine_version) - 1);

    if (!bytes || len < 5) {
        strncpy(out_summary->error_message, "buffer too small to be a valid PDF", sizeof(out_summary->error_message) - 1);
        out_summary->status_code = 2;
        return 2;
    }

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) {
        strncpy(out_summary->error_message, "failed to allocate fz_context", sizeof(out_summary->error_message) - 1);
        out_summary->status_code = 3;
        return 3;
    }

    fz_stream *stream = NULL;
    fz_document *doc = NULL;

    fz_var(stream);
    fz_var(doc);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);

        int needs_pw = fz_needs_password(ctx, doc);
        out_summary->needs_password = needs_pw;
        if (needs_pw) {
            out_summary->status_code = 1;
            strncpy(out_summary->error_message, "PDF requires an open password", sizeof(out_summary->error_message) - 1);
        } else {
            int pages = fz_count_pages(ctx, doc);
            out_summary->page_count = pages >= 0 ? pages : 0;

            lookup_optional_meta(ctx, doc, FZ_META_FORMAT, out_summary->format_version, sizeof(out_summary->format_version));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_TITLE, out_summary->title, sizeof(out_summary->title));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_AUTHOR, out_summary->author, sizeof(out_summary->author));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_SUBJECT, out_summary->subject, sizeof(out_summary->subject));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_KEYWORDS, out_summary->keywords, sizeof(out_summary->keywords));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_CREATOR, out_summary->creator, sizeof(out_summary->creator));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_PRODUCER, out_summary->producer, sizeof(out_summary->producer));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_CREATIONDATE, out_summary->creation_date, sizeof(out_summary->creation_date));
            lookup_optional_meta(ctx, doc, FZ_META_INFO_MODIFICATIONDATE, out_summary->modification_date, sizeof(out_summary->modification_date));

            out_summary->status_code = 0;
        }
    }
    fz_always(ctx) {
        if (doc) {
            fz_drop_document(ctx, doc);
        }
        if (stream) {
            fz_drop_stream(ctx, stream);
        }
    }
    fz_catch(ctx) {
        const char *msg = fz_caught_message(ctx);
        if (msg) {
            strncpy(out_summary->error_message, msg, sizeof(out_summary->error_message) - 1);
        } else {
            strncpy(out_summary->error_message, "MuPDF threw an exception while opening PDF", sizeof(out_summary->error_message) - 1);
        }
        out_summary->status_code = 2;
    }

    fz_drop_context(ctx);
    return out_summary->status_code;
}
