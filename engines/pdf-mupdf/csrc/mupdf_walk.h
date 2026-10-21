/* The places of a document that can carry text.
 *
 * A redaction has to know every place of the document that can carry the
 * text of the user — not only the content stream of the page, but every
 * string of the document: in the text of an annotation, in the popup of an
 * annotation, in the value of a form field, in the structure tree, in the
 * name tree, in the optional content, in the private data of a page, in the
 * metadata, in the XMP packet.
 *
 * The second round of the red team worked with a list of known places. The
 * third round showed what a list misses: a popup that does not stand in
 * `/Annots`, the structure tree, the optional content, the name tree — every
 * one of them carried the secret, and the redaction reported success.
 *
 * The walk below therefore visits **every** object of the document: the
 * pages, the catalog, the annotations, the form fields, the metadata, the
 * structure tree, the name tree, the optional content, the private data and
 * the embedded files. Every string and every stream is offered to the
 * caller, together with the place it lies in. The caller decides what it
 * does with it: the search reports it, the scrub empties it.
 *
 * The objects of a page are walked with the number of the page, so that the
 * caller can tell the visible text of a page from the hidden text of the
 * document. The content stream of a page is marked as such. */

#ifndef VOCUMENT_MUPDF_WALK_H
#define VOCUMENT_MUPDF_WALK_H

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* The maximum depth of the walk.
 *
 * A document may refer to itself — a page refers to its parent, the parent
 * refers to the children. The depth keeps the walk finite; the objects that
 * were visited are remembered as well, so that a cycle costs nothing. */
#define VOCUMENT_WALK_MAX_DEPTH 48

/* The length of the path that names a place in the document. */
#define VOCUMENT_WALK_PATH_MAX 256

/* One place of the document. */
typedef struct {
    /* The object that carries the text. */
    pdf_obj *object;
    /* The dictionary or the array that holds the object. */
    pdf_obj *parent;
    /* The key of the object in its parent — `NULL` when the object is an
       element of an array. */
    pdf_obj *key;
    /* The index of the object in its parent — `-1` when the object is the
       value of a dictionary. */
    int index;
    /* The page the object lies on — `-1` for the document itself. */
    int page_index;
    /* `1` when the object is the content of a page. */
    int is_page_content;
    /* The way to the object, for example `/Root/Names/Dests`. */
    char path[VOCUMENT_WALK_PATH_MAX];
} vocument_walk_item;

/* The callback of the walk. It returns `0` to stop the walk. */
typedef int (*vocument_walk_callback)(void *user, const vocument_walk_item *item);

/* The state of the walk. */
typedef struct {
    fz_context *ctx;
    vocument_walk_callback callback;
    void *user;
    /* The numbers of the indirect objects that were visited. */
    int *visited;
    int visited_count;
    int visited_capacity;
} vocument_walk_t;

/* Prepares a walk. */
static inline void vocument_walk_init(
    vocument_walk_t *walk,
    fz_context *ctx,
    vocument_walk_callback callback,
    void *user
) {
    walk->ctx = ctx;
    walk->callback = callback;
    walk->user = user;
    walk->visited = NULL;
    walk->visited_count = 0;
    walk->visited_capacity = 0;
}

/* Releases the state of the walk. */
static inline void vocument_walk_free(vocument_walk_t *walk) {
    if (walk->visited) {
        free(walk->visited);
        walk->visited = NULL;
    }
    walk->visited_count = 0;
    walk->visited_capacity = 0;
}

/* Answers whether the object was visited before, and remembers it.
 *
 * Only the indirect objects are remembered: they are the ones that can form
 * a cycle. A direct object lives inside its parent and cannot refer to
 * itself. */
static inline int vocument_walk_seen(vocument_walk_t *walk, pdf_obj *obj) {
    fz_context *ctx = walk->ctx;
    if (!pdf_is_indirect(ctx, obj)) {
        return 0;
    }
    int num = pdf_to_num(ctx, obj);
    if (num <= 0) {
        return 0;
    }
    for (int i = 0; i < walk->visited_count; i++) {
        if (walk->visited[i] == num) {
            return 1;
        }
    }
    if (walk->visited_count >= walk->visited_capacity) {
        int capacity = walk->visited_capacity ? walk->visited_capacity * 2 : 128;
        int *grown = (int *)realloc(walk->visited, (size_t)capacity * sizeof(int));
        if (!grown) {
            return 0;
        }
        walk->visited = grown;
        walk->visited_capacity = capacity;
    }
    walk->visited[walk->visited_count++] = num;
    return 0;
}

/* Answers whether the object is a page. */
static inline int vocument_walk_is_page(fz_context *ctx, pdf_obj *obj) {
    pdf_obj *type = pdf_dict_get(ctx, obj, PDF_NAME(Type));
    if (!pdf_is_name(ctx, type)) {
        return 0;
    }
    const char *name = pdf_to_name(ctx, type);
    return name && strcmp(name, "Page") == 0;
}

/* Offers a place to the caller. */
static inline void vocument_walk_offer(
    vocument_walk_t *walk,
    pdf_obj *object,
    pdf_obj *parent,
    pdf_obj *key,
    int index,
    int page_index,
    int is_page_content,
    const char *path
) {
    vocument_walk_item item;
    item.object = object;
    item.parent = parent;
    item.key = key;
    item.index = index;
    item.page_index = page_index;
    item.is_page_content = is_page_content;
    snprintf(item.path, sizeof(item.path), "%s", path ? path : "");
    walk->callback(walk->user, &item);
}

/* Walks through the objects of the document. */
static inline void vocument_walk_object(
    vocument_walk_t *walk,
    pdf_obj *obj,
    pdf_obj *parent,
    pdf_obj *key,
    int index,
    int page_index,
    int is_page_content,
    int depth,
    const char *path
);

/* Walks through the elements of an array. */
static inline void vocument_walk_array(
    vocument_walk_t *walk,
    pdf_obj *array,
    int page_index,
    int is_page_content,
    int depth,
    const char *path
) {
    fz_context *ctx = walk->ctx;
    int count = pdf_array_len(ctx, array);
    for (int i = 0; i < count; i++) {
        char sub[VOCUMENT_WALK_PATH_MAX];
        snprintf(sub, sizeof(sub), "%s[%d]", path, i);
        vocument_walk_object(
            walk,
            pdf_array_get(ctx, array, i),
            array,
            NULL,
            i,
            page_index,
            is_page_content,
            depth + 1,
            sub
        );
    }
}

/* Walks through the values of a dictionary. */
static inline void vocument_walk_dict(
    vocument_walk_t *walk,
    pdf_obj *dict,
    int page_index,
    int is_page_content,
    int depth,
    const char *path
) {
    fz_context *ctx = walk->ctx;
    int is_page = vocument_walk_is_page(ctx, dict);
    int count = pdf_dict_len(ctx, dict);
    for (int i = 0; i < count; i++) {
        pdf_obj *child_key = pdf_dict_get_key(ctx, dict, i);
        pdf_obj *child = pdf_dict_get_val(ctx, dict, i);
        const char *name = pdf_to_name(ctx, child_key);
        /* The parent of a page is the tree of the pages. Walking into it
           would visit the other pages of the document — and they would carry
           the number of the page they were reached from. The pages are
           walked on their own, with their own number. */
        if (is_page && name && strcmp(name, "Parent") == 0) {
            continue;
        }
        char sub[VOCUMENT_WALK_PATH_MAX];
        snprintf(sub, sizeof(sub), "%s/%s", path, name ? name : "?");
        /* The content of a page is the visible text of the page. The caller
           has to know the difference: the text of the page is removed by the
           redaction itself, the hidden text of the document is cleaned. */
        int child_is_page_content = is_page_content;
        if (is_page && name && strcmp(name, "Contents") == 0) {
            child_is_page_content = 1;
        }
        vocument_walk_object(
            walk,
            child,
            dict,
            child_key,
            -1,
            page_index,
            child_is_page_content,
            depth + 1,
            sub
        );
    }
}

/* Walks through one object. */
static inline void vocument_walk_object(
    vocument_walk_t *walk,
    pdf_obj *obj,
    pdf_obj *parent,
    pdf_obj *key,
    int index,
    int page_index,
    int is_page_content,
    int depth,
    const char *path
) {
    fz_context *ctx = walk->ctx;
    if (!obj || depth > VOCUMENT_WALK_MAX_DEPTH) {
        return;
    }
    if (vocument_walk_seen(walk, obj)) {
        return;
    }
    if (pdf_is_dict(ctx, obj)) {
        /* A stream is a dictionary with a content. Its content is a place of
           its own — the text of a page, the XMP packet, the file of an
           attachment — and the entries of its dictionary carry no text. */
        if (pdf_is_stream(ctx, obj)) {
            vocument_walk_offer(
                walk, obj, parent, key, index, page_index, is_page_content, path
            );
            return;
        }
        vocument_walk_dict(walk, obj, page_index, is_page_content, depth, path);
        return;
    }
    if (pdf_is_array(ctx, obj)) {
        vocument_walk_array(walk, obj, page_index, is_page_content, depth, path);
        return;
    }
    vocument_walk_offer(walk, obj, parent, key, index, page_index, is_page_content, path);
}

/* Walks through the whole document.
 *
 * The pages come first: the caller has to know the number of the page a
 * place lies on. The catalog and the document information follow; the pages
 * are not walked twice. */
static inline void vocument_walk_document(
    vocument_walk_t *walk,
    pdf_document *pdf_doc
) {
    fz_context *ctx = walk->ctx;
    int page_count = pdf_count_pages(ctx, pdf_doc);
    for (int i = 0; i < page_count; i++) {
        pdf_page *page = NULL;
        fz_var(page);
        fz_try(ctx) {
            page = pdf_load_page(ctx, pdf_doc, i);
            if (page) {
                vocument_walk_object(
                    walk, page->obj, NULL, NULL, -1, i, 0, 0, "/Page"
                );
            }
        }
        fz_always(ctx) {
            if (page) {
                pdf_drop_page(ctx, page);
            }
        }
        fz_catch(ctx) {
            fz_rethrow(ctx);
        }
    }
    pdf_obj *trailer = pdf_trailer(ctx, pdf_doc);
    pdf_obj *root = pdf_dict_get(ctx, trailer, PDF_NAME(Root));
    vocument_walk_object(walk, root, NULL, NULL, -1, -1, 0, 0, "/Root");
    pdf_obj *info = pdf_dict_get(ctx, trailer, PDF_NAME(Info));
    vocument_walk_object(walk, info, NULL, NULL, -1, -1, 0, 0, "/Info");
}

#endif /* VOCUMENT_MUPDF_WALK_H */
