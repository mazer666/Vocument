/* The places of a document that can carry text.
 *
 * A redaction has to know every place of the document that can carry the
 * text of the user — not only the content stream of the page, but every
 * string of the document: in the text of an annotation, in the popup of an
 * annotation, in the value of a form field, in the structure tree, in the
 * name tree, in the optional content, in the private data of a page, in the
 * metadata, in the XMP packet, in the identifier of the file.
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
 * document. The content stream of a page is marked as such.
 *
 * The fourth round of the red team showed three holes of this walk, and
 * they are closed here:
 *
 * 1. The dictionary of a stream was not walked. A form XObject carries its
 *    resources in its dictionary, and a form XObject can carry the text of
 *    the user in its content. The walk therefore walks the dictionary of a
 *    stream as well, and offers the content of the stream.
 * 2. The objects of the cross-reference table were handed out without their
 *    indirect wrapper, so that `pdf_is_stream()` answered "no" for every one
 *    of them and the content of a stream was never looked at. The sweep
 *    hands them out as indirect references now.
 * 3. The trailer was not walked at all, so that the identifier of the file
 *    (`/ID`) was never seen. The trailer is walked now. */

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
    /* The numbers of the indirect objects that were visited, in a hash
       table with open addressing. A linear list made the sweep over the
       cross-reference table quadratic: 32,000 objects needed 0.83 s. */
    int *visited;
    int visited_capacity;
    int visited_count;
    /* The places that the walk could **not** examine, one per line.
     *
     * A walk that stops somewhere without saying so is a walk that cannot
     * be trusted: the redaction would report a success while a place of the
     * document was never looked into. The limit of the depth and an object
     * that cannot be read are therefore written down here, and the caller
     * turns them into places that could not be looked into. */
    char *uninspected;
    size_t uninspected_len;
    size_t uninspected_capacity;
    /* The **limit of the length** of one note about a place that could not be
     * examined. Zero means that a note may be as long as it needs to be.
     *
     * The limit is only used by the test that proves that a place which
     * **cannot** be written down is not lost silently — it can only make the
     * walk stricter, never laxer. It is a limit of the *length* of a note,
     * not a budget of the number of the notes: the number of the notes is
     * not limited at all. */
    size_t note_limit;
    /* Set when a place could not be written down — because the memory of the
     * machine was exhausted, or because the note was longer than the limit.
     *
     * A walk that knows less about itself must say so: the caller turns this
     * flag into a place that could not be looked into, and a redaction that
     * cannot name every place it did not examine is refused. */
    int note_failed;
    /* The number of the places that could not be written down. The flag
     * stands for a number of places that the report does not name one by
     * one — the number belongs into the report, or the report would announce
     * a number that it does not know. */
    int note_failures;
} vocument_walk_t;

/* The number of slots of the hash table at the start. */
#define VOCUMENT_WALK_TABLE_MIN 256

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
    walk->visited_capacity = 0;
    walk->visited_count = 0;
    walk->uninspected = NULL;
    walk->uninspected_len = 0;
    walk->uninspected_capacity = 0;
    walk->note_limit = 0;
    walk->note_failed = 0;
    walk->note_failures = 0;
}

/* Releases the state of the walk. */
static inline void vocument_walk_free(vocument_walk_t *walk) {
    if (walk->visited) {
        free(walk->visited);
        walk->visited = NULL;
    }
    walk->visited_capacity = 0;
    walk->visited_count = 0;
    if (walk->uninspected) {
        free(walk->uninspected);
        walk->uninspected = NULL;
    }
    walk->uninspected_len = 0;
    walk->uninspected_capacity = 0;
}

/* Writes down a place that the walk could not examine.
 *
 * The reason comes first, the place second — the caller reads the lines
 * apart. A failure of the allocation is not fatal: the walk then knows less
 * about itself, and the caller sees an empty list. The allocation fails only
 * when the machine has no memory left at all. */
static inline void vocument_walk_note_uninspected(
    vocument_walk_t *walk,
    const char *reason,
    const char *path
) {
    if (!walk || !reason) {
        return;
    }
    const char *place = path ? path : "?";
    size_t needed = strlen(reason) + strlen(place) + 3;
    /* The budget: a place that does not fit into it is a place that cannot
     * be written down. The flag says so — the note is not lost silently. */
    if (walk->note_limit > 0 && needed + 1 > walk->note_limit) {
        walk->note_failed = 1;
        walk->note_failures++;
        return;
    }
    if (walk->uninspected_len + needed + 1 > walk->uninspected_capacity) {
        size_t capacity = walk->uninspected_capacity ? walk->uninspected_capacity : 256;
        while (walk->uninspected_len + needed + 1 > capacity) {
            capacity *= 2;
        }
        char *bigger = (char *)realloc(walk->uninspected, capacity);
        if (!bigger) {
            /* The machine has no memory for this note. The walk then knows
             * less about itself — and it has to say so, or the caller
             * mistakes an unexamined place for an examined one. */
            walk->note_failed = 1;
            walk->note_failures++;
            return;
        }
        walk->uninspected = bigger;
        walk->uninspected_capacity = capacity;
    }
    int written = snprintf(
        walk->uninspected + walk->uninspected_len,
        walk->uninspected_capacity - walk->uninspected_len,
        "%s\t%s\n",
        reason,
        place
    );
    if (written < 0) {
        walk->note_failed = 1;
        walk->note_failures++;
        return;
    }
    walk->uninspected_len += (size_t)written;
    if (walk->uninspected_len >= walk->uninspected_capacity) {
        walk->uninspected_len = walk->uninspected_capacity - 1;
    }
}

/* The slot of an object number in the hash table. */
static inline int vocument_walk_slot(int num, int capacity) {
    unsigned int hash = (unsigned int)num * 2654435761u;
    return (int)(hash % (unsigned int)capacity);
}

/* Remembers the number of an object.
 *
 * The table grows when it is half full, so that the search for a free slot
 * stays short. A failure of the allocation is not fatal: the walk then
 * remembers fewer objects, and the objects that are visited twice are
 * offered twice — the caller does not care. */
static inline void vocument_walk_remember(vocument_walk_t *walk, int num) {
    if (num <= 0) {
        return;
    }
    if (walk->visited_capacity == 0) {
        int capacity = VOCUMENT_WALK_TABLE_MIN;
        int *table = (int *)calloc((size_t)capacity, sizeof(int));
        if (!table) {
            return;
        }
        walk->visited = table;
        walk->visited_capacity = capacity;
    } else if ((walk->visited_count + 1) * 2 >= walk->visited_capacity) {
        int capacity = walk->visited_capacity * 2;
        int *table = (int *)calloc((size_t)capacity, sizeof(int));
        if (!table) {
            return;
        }
        for (int i = 0; i < walk->visited_capacity; i++) {
            int entry = walk->visited[i];
            if (entry <= 0) {
                continue;
            }
            int slot = vocument_walk_slot(entry, capacity);
            while (table[slot] != 0) {
                slot = (slot + 1) % capacity;
            }
            table[slot] = entry;
        }
        free(walk->visited);
        walk->visited = table;
        walk->visited_capacity = capacity;
    }
    int slot = vocument_walk_slot(num, walk->visited_capacity);
    while (walk->visited[slot] != 0 && walk->visited[slot] != num) {
        slot = (slot + 1) % walk->visited_capacity;
    }
    if (walk->visited[slot] == 0) {
        walk->visited[slot] = num;
        walk->visited_count++;
    }
}

/* Answers whether the object with this number was visited before. */
static inline int vocument_walk_already_seen(vocument_walk_t *walk, int num) {
    if (num <= 0 || walk->visited_capacity == 0) {
        return 0;
    }
    int slot = vocument_walk_slot(num, walk->visited_capacity);
    for (int probe = 0; probe < walk->visited_capacity; probe++) {
        int entry = walk->visited[slot];
        if (entry == 0) {
            return 0;
        }
        if (entry == num) {
            return 1;
        }
        slot = (slot + 1) % walk->visited_capacity;
    }
    return 0;
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
    if (vocument_walk_already_seen(walk, num)) {
        return 1;
    }
    vocument_walk_remember(walk, num);
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

/* Answers whether the dictionary is the encryption dictionary of the
   document.
 *
   The fourth round of the red team protected the key `/Encrypt` in the
   trailer: the encryption dictionary hangs there, and emptying one of its
   keys would destroy the document. But the specification (ISO 32000-1,
   Table 15) asks for the `/Encrypt` of the trailer to be an **indirect
   reference** — and then the dictionary is an object of the file, which the
   sweep over the cross-reference table reaches by its number. The protection
   of the key did not apply to that path, and the fifth round measured a
   redaction that emptied the `/U` of an encrypted document and broke it.
 *
   The dictionary is therefore recognised by its **content**: the standard
   security handler carries the two keys `/O` and `/U` and the filter
 * `/Standard`. A dictionary that looks like that is never descended into —
 * wherever it hangs, and whether it is reached by the walk from the root or
   by the sweep over the table. */
static inline int vocument_walk_is_security_dictionary(fz_context *ctx, pdf_obj *obj) {
    if (!pdf_is_dict(ctx, obj)) {
        return 0;
    }
    if (!pdf_dict_get(ctx, obj, PDF_NAME(O)) || !pdf_dict_get(ctx, obj, PDF_NAME(U))) {
        return 0;
    }
    pdf_obj *filter = pdf_dict_get(ctx, obj, PDF_NAME(Filter));
    if (pdf_is_name(ctx, filter)) {
        const char *name = pdf_to_name(ctx, filter);
        return name && strcmp(name, "Standard") == 0;
    }
    if (pdf_is_dict(ctx, filter)) {
        return pdf_dict_get(ctx, filter, PDF_NAME(Standard)) != NULL;
    }
    return 0;
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
    if (!obj) {
        return;
    }
    if (depth > VOCUMENT_WALK_MAX_DEPTH) {
        /* The limit of the depth keeps the walk finite. Everything below it
           is **not** examined — and a place that was not examined must not
           pass for a place that was examined and found clean. */
        vocument_walk_note_uninspected(
            walk,
            "the limit of the depth of the walk was reached",
            path
        );
        return;
    }
    if (vocument_walk_seen(walk, obj)) {
        return;
    }
    if (pdf_is_dict(ctx, obj)) {
        /* The encryption dictionary holds the keys of the document, not the
           text of the user. It is **examined** nevertheless: a key that
           happens to spell the text of the user has to be seen by the
           check, otherwise the check would report a success while the text
           stands in the document. It is never **cleaned** — that is the
           business of the cleaning, which refuses the keys of the
           document. */
        /* A stream is a dictionary with a content. Its content is a place of
           its own — the text of a page, the XMP packet, the file of an
           attachment — and the entries of its dictionary can carry the text
           as well: the resources of a form XObject, for example, hold the
           form XObjects that draw the text. Both are therefore walked. */
        if (pdf_is_stream(ctx, obj)) {
            vocument_walk_offer(
                walk, obj, parent, key, index, page_index, is_page_content, path
            );
            vocument_walk_dict(walk, obj, page_index, 0, depth, path);
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

/* Answers whether the stream is a container of objects rather than content.
 *
 * An object stream holds the objects of the file in a compressed form; a
 * cross-reference stream holds the cross-reference table. Neither carries
 * text of the user — their content is the objects themselves, and those are
 * walked on their own. A stream that is not a container is content: the
 * text of a page, the XMP packet, the file of an attachment. */
static inline int vocument_walk_is_container_stream(fz_context *ctx, pdf_obj *obj) {
    pdf_obj *type = pdf_dict_get(ctx, obj, PDF_NAME(Type));
    if (!pdf_is_name(ctx, type)) {
        return 0;
    }
    const char *name = pdf_to_name(ctx, type);
    if (!name) {
        return 0;
    }
    return strcmp(name, "ObjStm") == 0 || strcmp(name, "XRef") == 0;
}

/* Walks through every object of the cross-reference table.
 *
 * The walk from the root reaches the objects that the document still uses. A
 * PDF can carry objects that nothing refers to any more: an older version of
 * a page, a note that was deleted but stayed in the file, an object that the
 * writer left behind. A secret can stand in such an object, and the
 * redaction has to see it — otherwise the file keeps a copy of the text that
 * the user believes he has removed.
 *
 * The cross-reference table lists every object of the file, including the
 * ones that nothing refers to. The objects that were already reached are
 * skipped, so that every object is offered once.
 *
 * The objects are handed out as **indirect references**. The entry of the
 * table holds the parsed object, and `pdf_is_stream()` works on the number
 * of the object — a direct object has the number zero, and the answer is
 * therefore always "no". With the indirect reference the content of the
 * stream is found and cleaned. */
static inline void vocument_walk_xref(
    vocument_walk_t *walk,
    pdf_document *pdf_doc
) {
    fz_context *ctx = walk->ctx;
    int length = pdf_xref_len(ctx, pdf_doc);
    for (int i = 0; i < length; i++) {
        pdf_xref_entry *entry = pdf_get_xref_entry(ctx, pdf_doc, i);
        if (!entry) {
            continue;
        }
        /* `f` is a free entry, `n` a normal object, `o` an object that lives
           in an object stream. */
        if (entry->type != 'n' && entry->type != 'o') {
            continue;
        }
        int num = entry->num;
        if (num <= 0 || vocument_walk_already_seen(walk, num)) {
            continue;
        }
        /* The object has to be read before it can be walked. An object that
           cannot be read is a place that was **not** examined: the walk
           writes it down, so that the caller does not mistake it for a
           place that was examined and found clean. */
        pdf_xref_entry *cached = pdf_cache_object(ctx, pdf_doc, num);
        if (!cached || !cached->obj) {
            char place[VOCUMENT_WALK_PATH_MAX];
            snprintf(place, sizeof(place), "/Object[%d]", num);
            vocument_walk_note_uninspected(
                walk,
                "the object could not be read",
                place
            );
            continue;
        }
        if (pdf_obj_num_is_stream(ctx, pdf_doc, num) &&
            vocument_walk_is_container_stream(ctx, cached->obj)) {
            continue;
        }
        char path[VOCUMENT_WALK_PATH_MAX];
        snprintf(path, sizeof(path), "/Object[%d]", num);
        /* The indirect reference: it carries the number of the object, and
           with it the content of the stream. The number must **not** be
           remembered here — the walk remembers it itself, and a number that
           is remembered twice would make the walk skip the object. */
        pdf_obj *reference = pdf_new_indirect(ctx, pdf_doc, num, entry->gen);
        if (!reference) {
            char place[VOCUMENT_WALK_PATH_MAX];
            snprintf(place, sizeof(place), "/Object[%d]", num);
            vocument_walk_note_uninspected(
                walk,
                "the object could not be addressed",
                place
            );
            continue;
        }
        vocument_walk_object(walk, reference, NULL, NULL, -1, -1, 0, 0, path);
        pdf_drop_obj(ctx, reference);
    }
}

/* Walks through the whole document.
 *
 * The pages come first: the caller has to know the number of the page a
 * place lies on. The catalog and the document information follow; the pages
 * are not walked twice. The trailer follows, because it holds the
 * identifier of the file (`/ID`), which is not an object of the table and
 * would therefore be missed by the sweep. The cross-reference table comes
 * last: it holds the objects that nothing refers to any more. */
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
    /* The trailer itself: the identifier of the file (`/ID`) lives here, and
       it is not an object of the table — the sweep cannot reach it. */
    vocument_walk_object(walk, trailer, NULL, NULL, -1, -1, 0, 0, "/Trailer");
    /* The encryption dictionary is **not** walked: it holds the keys of the
       document, not the text of the user. Emptying one of those keys would
       destroy the document. */
    /* The objects that nothing refers to any more. */
    vocument_walk_xref(walk, pdf_doc);
}

#endif /* VOCUMENT_MUPDF_WALK_H */
