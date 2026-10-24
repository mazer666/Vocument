/*
 * MuPDF Hierarchical Bookmark Bridge (`ROADMAP.md` M3.4).
 * Real native execution using MuPDF C API. Conforms to ISO 32000-1 and CODE-001 (<= 600 lines).
 */

#ifndef VOCUMENT_MUPDF_BOOKMARK_BRIDGE_H
#define VOCUMENT_MUPDF_BOOKMARK_BRIDGE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

int vocument_mupdf_get_bookmarks_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    char **out_json_str
);

int vocument_mupdf_set_bookmarks_json(
    const unsigned char *pdf_bytes,
    size_t pdf_len,
    const char *bookmarks_json,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
);

#ifdef __cplusplus
}
#endif

#endif /* VOCUMENT_MUPDF_BOOKMARK_BRIDGE_H */
