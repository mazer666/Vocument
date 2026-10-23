#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

#include "mupdf_serialize.h"

void vocument_mupdf_free_bytes(unsigned char *ptr);

#include "mupdf_page_helpers.h"

/*
 * Reorders the pages of a PDF document according to `order` (0-indexed).
 * Performed in-place on the source document to preserve Document Catalog,
 * Metadata (/Info and /Metadata), Bookmarks (/Outlines), PageLabels,
 * Form definitions, Tagged PDF structures (/StructTreeRoot), and language tags.
 *
 * Status:
 *   0: success
 *   1: invalid arguments or invalid permutation
 *   2: cannot open or parse PDF
 *   3: out of memory
 *   4: operation blocked by digital signatures (preflight fail-closed)
 */
int vocument_mupdf_reorder_pages_ext(
    const unsigned char *bytes,
    size_t len,
    const int *order,
    size_t order_len,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !order || order_len == 0 || !out_pdf_bytes || !out_pdf_len) {
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
            int count = pdf_count_pages(ctx, pdf_doc);
            if ((size_t)count != order_len) {
                status = 1;
            } else {
                /* Validate permutation */
                int *seen = (int *)calloc((size_t)count, sizeof(int));
                if (!seen) {
                    fz_throw(ctx, FZ_ERROR_SYSTEM, "calloc failed");
                }
                for (size_t i = 0; i < order_len; i++) {
                    int p = order[i];
                    if (p < 0 || p >= count || seen[p]) {
                        status = 1;
                        break;
                    }
                    seen[p] = 1;
                }
                free(seen);

                if (status == 0) {
                    int sigs = count_active_signatures(ctx, pdf_doc);
                    if (sigs != 0 && !allow_signature_invalidation) {
                        status = 4; /* Blocked by digital signatures or unexaminable structure */
                    } else if (sigs < 0) {
                        status = 1; /* Cannot safely invalidate cyclic/unexaminable structure */
                    } else if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                        status = 1; /* Broken Tagged PDF structure fail-closed */
                    } else {
                        /* Collect page references and make them self-contained */
                        pdf_obj **page_objs = (pdf_obj **)fz_calloc(ctx, (size_t)count, sizeof(pdf_obj *));
                        for (int i = 0; i < count; i++) {
                            pdf_obj *p_ref = pdf_lookup_page_obj(ctx, pdf_doc, i);
                            make_page_self_contained(ctx, p_ref);
                            page_objs[i] = p_ref;
                        }

                        pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
                        pdf_obj *pages_root = pdf_dict_get(ctx, root, PDF_NAME(Pages));

                        /* Reconstruct page tree in new permutation order */
                        pdf_obj *new_kids = pdf_new_array(ctx, pdf_doc, count);
                        for (size_t i = 0; i < order_len; i++) {
                            pdf_obj *p_ref = page_objs[order[i]];
                            pdf_array_push(ctx, new_kids, p_ref);
                            pdf_dict_put(ctx, p_ref, PDF_NAME(Parent), pages_root);
                        }
                        pdf_dict_put_drop(ctx, pages_root, PDF_NAME(Kids), new_kids);
                        pdf_dict_put_int(ctx, pages_root, PDF_NAME(Count), count);

                        fz_free(ctx, page_objs);

                        restructure_pagelabels_for_reorder(ctx, pdf_doc, order, count);

                        if (sigs > 0 && allow_signature_invalidation) {
                            mark_signatures_invalidated(ctx, pdf_doc);
                        }

                        if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                            status = 1;
                        } else {
                            pdf_sync_open_pages(ctx, pdf_doc);
                            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                        }
                    }
                }
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}

int vocument_mupdf_reorder_pages(
    const unsigned char *bytes,
    size_t len,
    const int *order,
    size_t order_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    return vocument_mupdf_reorder_pages_ext(bytes, len, order, order_len, 0, out_pdf_bytes, out_pdf_len);
}

/*
 * Duplicates `page_index` (0-indexed) by inserting a copy immediately following it.
 * Preserves all document catalog metadata, bookmarks, page labels, and tagged structures.
 * Clones page contents, resources, and annotations, and assigns unique names to duplicated form fields.
 *
 * Status:
 *   0: success
 *   1: invalid arguments or page index out of range
 *   2: cannot open or parse PDF
 *   3: out of memory
 *   4: operation blocked by digital signatures (preflight fail-closed)
 */
int vocument_mupdf_duplicate_page_ext(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || page_index < 0 || !out_pdf_bytes || !out_pdf_len) {
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
            int count = pdf_count_pages(ctx, pdf_doc);
            if (page_index >= count) {
                status = 1;
            } else {
                int sigs = count_active_signatures(ctx, pdf_doc);
                if (sigs != 0 && !allow_signature_invalidation) {
                    status = 4;
                } else if (sigs < 0) {
                    status = 1;
                } else if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                    status = 1; /* Inconsistent Tagged PDF structure fail-closed */
                } else {
                    pdf_obj *src_page_ref = pdf_lookup_page_obj(ctx, pdf_doc, page_index);
                    make_page_self_contained(ctx, src_page_ref);

                    pdf_obj *new_page_dict = pdf_copy_dict(ctx, src_page_ref);
                    pdf_dict_del(ctx, new_page_dict, PDF_NAME(Parent));
                    pdf_dict_del(ctx, new_page_dict, PDF_NAME(Annots));

                    pdf_obj *sp_obj = pdf_dict_gets(ctx, src_page_ref, "StructParents");
                    if (sp_obj && pdf_is_int(ctx, sp_obj)) {
                        int src_sp = pdf_to_int(ctx, sp_obj);
                        pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
                        pdf_obj *struct_tree_root = root ? pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "StructTreeRoot")) : NULL;
                        pdf_obj *parent_tree = struct_tree_root ? pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, struct_tree_root, "ParentTree")) : NULL;
                        pdf_obj *nums = parent_tree ? pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, parent_tree, "Nums")) : NULL;
                        if (!nums || !pdf_is_array(ctx, nums)) {
                            status = 1; /* Missing ParentTree Nums for page with StructParents */
                        } else {
                            int nums_len = pdf_array_len(ctx, nums);
                            int max_sp = 0;
                            pdf_obj *found_arr = NULL;
                            for (int n = 0; n < nums_len; n += 2) {
                                int k = pdf_to_int(ctx, pdf_array_get(ctx, nums, n));
                                if (k > max_sp) max_sp = k;
                                if (k == src_sp) {
                                    found_arr = pdf_array_get(ctx, nums, n + 1);
                                }
                            }
                            if (!found_arr) {
                                status = 1; /* StructParents key not in ParentTree */
                            } else {
                                int new_sp = max_sp + 1;
                                pdf_dict_put_int(ctx, new_page_dict, PDF_NAME(StructParents), new_sp);
                                pdf_obj *cloned_arr = pdf_deep_copy_obj(ctx, found_arr);
                                pdf_array_push_int(ctx, nums, new_sp);
                                pdf_array_push_drop(ctx, nums, cloned_arr);
                            }
                        }
                    }

                    if (status == 0) {
                        pdf_obj *new_page_ref = pdf_add_object_drop(ctx, pdf_doc, new_page_dict);

                        /* Clone annotations and form fields if present */
                        pdf_obj *src_annots = pdf_dict_get(ctx, src_page_ref, PDF_NAME(Annots));
                        if (pdf_is_array(ctx, src_annots) && pdf_array_len(ctx, src_annots) > 0) {
                            int n_annots = pdf_array_len(ctx, src_annots);
                            pdf_obj *new_annots = pdf_new_array(ctx, pdf_doc, n_annots);

                            pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
                            pdf_obj *acroform = pdf_dict_get(ctx, root, PDF_NAME(AcroForm));
                            pdf_obj *fields = acroform ? pdf_dict_get(ctx, acroform, PDF_NAME(Fields)) : NULL;

                            for (int a = 0; a < n_annots; a++) {
                                pdf_obj *src_a = pdf_array_get(ctx, src_annots, a);
                                pdf_obj *annot_copy = pdf_deep_copy_obj(ctx, pdf_resolve_indirect(ctx, src_a));
                                pdf_dict_put(ctx, annot_copy, PDF_NAME(P), new_page_ref);

                                /* Check for form widget */
                                pdf_obj *ft = pdf_dict_get_inheritable(ctx, annot_copy, PDF_NAME(FT));
                                pdf_obj *t = pdf_dict_get(ctx, annot_copy, PDF_NAME(T));
                                if (ft || t) {
                                    if (t) {
                                        const char *orig_name = pdf_to_text_string(ctx, t);
                                        char renamed[256];
                                        generate_unique_field_name(ctx, fields, orig_name, renamed, sizeof(renamed));
                                        pdf_dict_put_text_string(ctx, annot_copy, PDF_NAME(T), renamed);
                                    }
                                }

                                pdf_obj *new_a_ref = pdf_add_object_drop(ctx, pdf_doc, annot_copy);
                                if ((ft || t) && fields && pdf_is_array(ctx, fields)) {
                                    pdf_array_push(ctx, fields, new_a_ref);
                                }
                                pdf_array_push(ctx, new_annots, new_a_ref);
                                pdf_drop_obj(ctx, new_a_ref);
                            }
                            pdf_dict_put_drop(ctx, new_page_ref, PDF_NAME(Annots), new_annots);
                        }

                        pdf_insert_page(ctx, pdf_doc, page_index + 1, new_page_ref);
                        pdf_drop_obj(ctx, new_page_ref);

                        if (sigs > 0 && allow_signature_invalidation) {
                            mark_signatures_invalidated(ctx, pdf_doc);
                        }

                        if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                            status = 1;
                        } else {
                            pdf_sync_open_pages(ctx, pdf_doc);
                            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                        }
                    }
                }
            }
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}

int vocument_mupdf_duplicate_page(
    const unsigned char *bytes,
    size_t len,
    int page_index,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    return vocument_mupdf_duplicate_page_ext(bytes, len, page_index, 0, out_pdf_bytes, out_pdf_len);
}

/*
 * Deletes the specified `page_indices` from the PDF document.
 * Performed in-place on the source document to preserve Document Catalog,
 * Metadata, surviving bookmarks, page labels, and tagged structures.
 * Cleans up /AcroForm /Fields references and re-anchors bookmarks targeting deleted pages.
 * Refuses if all pages would be deleted.
 *
 * Status:
 *   0: success
 *   1: invalid arguments, invalid index, or attempt to delete all pages
 *   2: cannot open or parse PDF
 *   3: out of memory
 *   4: operation blocked by digital signatures (preflight fail-closed)
 */
int vocument_mupdf_delete_pages_ext(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
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
            int count = pdf_count_pages(ctx, pdf_doc);
            char *delete_mask = (char *)calloc((size_t)count, sizeof(char));
            if (!delete_mask) {
                fz_throw(ctx, FZ_ERROR_SYSTEM, "calloc failed");
            }

            size_t deleted_count = 0;
            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= count) {
                    status = 1;
                    break;
                }
                if (!delete_mask[p]) {
                    delete_mask[p] = 1;
                    deleted_count++;
                }
            }

            if (status == 0) {
                int sigs = count_active_signatures(ctx, pdf_doc);
                if (sigs != 0 && !allow_signature_invalidation) {
                    status = 4;
                } else if (sigs < 0) {
                    status = 1;
                } else if (deleted_count >= (size_t)count) {
                    status = 1; /* Cannot delete all pages */
                } else if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                    status = 1; /* Inconsistent Tagged PDF structure fail-closed */
                } else {
                        /* Collect deleted page objects and determine a surviving target */
                        pdf_obj **del_page_objs = (pdf_obj **)fz_calloc(ctx, deleted_count, sizeof(pdf_obj *));
                        size_t del_idx = 0;
                        pdf_obj *surviving_target = NULL;

                        for (int i = 0; i < count; i++) {
                            pdf_obj *p_ref = pdf_lookup_page_obj(ctx, pdf_doc, i);
                            if (delete_mask[i]) {
                                del_page_objs[del_idx++] = p_ref;
                            } else if (!surviving_target) {
                                surviving_target = p_ref;
                            }
                        }

                        /* Clean up AcroForm fields referencing deleted pages */
                        cleanup_acroform_fields_for_deleted_pages(ctx, pdf_doc, del_page_objs, (int)deleted_count);

                        /* Re-anchor outline bookmarks targeting deleted pages */
                        pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
                        pdf_obj *outlines = pdf_dict_get(ctx, root, PDF_NAME(Outlines));
                        if (outlines && pdf_is_dict(ctx, outlines)) {
                            pdf_obj *first_item = pdf_dict_get(ctx, outlines, PDF_NAME(First));
                            while (first_item && pdf_is_dict(ctx, first_item)) {
                                reanchor_outline_item(ctx, first_item, del_page_objs, (int)deleted_count, surviving_target);
                                first_item = pdf_dict_get(ctx, first_item, PDF_NAME(Next));
                            }
                        }

                        /* Re-anchor internal links targeting deleted pages */
                        reanchor_internal_links_for_deleted_pages(ctx, pdf_doc, del_page_objs, (int)deleted_count, surviving_target);

                        /* Clean up Tagged PDF ParentTree for deleted pages */
                        cleanup_parent_tree_for_deleted_pages(ctx, pdf_doc, del_page_objs, (int)deleted_count);

                        fz_free(ctx, del_page_objs);

                        /* Perform in-place page deletions in descending order */
                        for (int p = count - 1; p >= 0; p--) {
                            if (delete_mask[p]) {
                                pdf_delete_page(ctx, pdf_doc, p);
                            }
                        }

                        if (sigs > 0 && allow_signature_invalidation) {
                            mark_signatures_invalidated(ctx, pdf_doc);
                        }

                        if (!validate_tagged_pdf_consistency(ctx, pdf_doc)) {
                            status = 1;
                        } else {
                            pdf_sync_open_pages(ctx, pdf_doc);
                            vocument_serialize_pdf_doc_to_malloc(ctx, pdf_doc, out_pdf_bytes, out_pdf_len);
                        }
                }
            }
            free(delete_mask);
        }
    }
    fz_always(ctx) {
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}

int vocument_mupdf_delete_pages(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    return vocument_mupdf_delete_pages_ext(bytes, len, page_indices, page_indices_len, 0, out_pdf_bytes, out_pdf_len);
}

/*
 * Extracts the requested `page_indices` into a new standalone PDF document.
 * Preserves Document /Info metadata, /Metadata XMP stream, document language (/Lang),
 * and page-level annotations and form widgets. Source document is left completely unchanged.
 *
 * Status:
 *   0: success
 *   1: invalid arguments or index out of range
 *   2: cannot open or parse PDF
 *   3: out of memory
 */
int vocument_mupdf_extract_pages(
    const unsigned char *bytes,
    size_t len,
    const int *page_indices,
    size_t page_indices_len,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!bytes || len == 0 || !page_indices || page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    fz_stream *stream = NULL;
    fz_document *doc = NULL;
    pdf_document *dst_doc = NULL;
    int status = 0;

    fz_var(stream);
    fz_var(doc);
    fz_var(dst_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);
        stream = fz_open_memory(ctx, bytes, len);
        doc = fz_open_document_with_stream(ctx, "application/pdf", stream);
        pdf_document *src_doc = pdf_specifics(ctx, doc);
        if (!src_doc) {
            status = 2;
        } else {
            int count = pdf_count_pages(ctx, src_doc);
            for (size_t i = 0; i < page_indices_len; i++) {
                int p = page_indices[i];
                if (p < 0 || p >= count) {
                    status = 1;
                    break;
                }
            }

            if (status == 0) {
                dst_doc = pdf_create_document(ctx);

                /* Graft Document /Info dictionary (Title, Author, Producer, CreationDate, etc.) */
                pdf_obj *src_info = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Info));
                if (src_info) {
                    pdf_obj *dst_info = pdf_graft_object(ctx, dst_doc, src_info);
                    pdf_dict_put_drop(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Info), dst_info);
                }

                /* Graft Document /Metadata XMP stream and /Lang */
                pdf_obj *src_root = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Root));
                pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
                if (src_root && dst_root) {
                    pdf_obj *src_meta = pdf_dict_get(ctx, src_root, PDF_NAME(Metadata));
                    if (src_meta) {
                        pdf_obj *dst_meta = pdf_graft_object(ctx, dst_doc, src_meta);
                        pdf_dict_put_drop(ctx, dst_root, PDF_NAME(Metadata), dst_meta);
                    }
                    pdf_obj *src_lang = pdf_dict_get(ctx, src_root, PDF_NAME(Lang));
                    if (src_lang) {
                        pdf_obj *dst_lang = pdf_graft_object(ctx, dst_doc, src_lang);
                        pdf_dict_put_drop(ctx, dst_root, PDF_NAME(Lang), dst_lang);
                    }
                }

                pdf_graft_map *map = pdf_new_graft_map(ctx, dst_doc);
                fz_try(ctx) {
                    for (size_t i = 0; i < page_indices_len; i++) {
                        int src_p = page_indices[i];
                        pdf_graft_mapped_page(ctx, map, -1, src_doc, src_p);
                        int dst_p = (int)i;

                        pdf_obj *src_page_ref = pdf_lookup_page_obj(ctx, src_doc, src_p);
                        pdf_obj *dst_page_ref = pdf_lookup_page_obj(ctx, dst_doc, dst_p);
                        if (dst_page_ref) {
                            pdf_dict_del(ctx, dst_page_ref, PDF_NAME(StructParents));
                            pdf_dict_dels(ctx, dst_page_ref, "Tabs");
                        }

                        /* Graft annotations and form fields */
                        pdf_obj *src_annots = pdf_dict_get(ctx, src_page_ref, PDF_NAME(Annots));
                        if (pdf_is_array(ctx, src_annots) && pdf_array_len(ctx, src_annots) > 0) {
                            int n_annots = pdf_array_len(ctx, src_annots);
                            pdf_obj *dst_annots = pdf_new_array(ctx, dst_doc, n_annots);

                            for (int a = 0; a < n_annots; a++) {
                                pdf_obj *src_a = pdf_array_get(ctx, src_annots, a);
                                pdf_obj *dst_a = pdf_graft_mapped_object(ctx, map, src_a);
                                pdf_dict_put(ctx, dst_a, PDF_NAME(P), dst_page_ref);
                                pdf_dict_dels(ctx, dst_a, "StructParent");
                                pdf_array_push(ctx, dst_annots, dst_a);
                            }
                            pdf_dict_put_drop(ctx, dst_page_ref, PDF_NAME(Annots), dst_annots);
                        }
                    }
                }
                fz_always(ctx) {
                    pdf_drop_graft_map(ctx, map);
                }
                fz_catch(ctx) {
                    fz_rethrow(ctx);
                }

                vocument_serialize_pdf_doc_to_malloc(ctx, dst_doc, out_pdf_bytes, out_pdf_len);
            }
        }
    }
    fz_always(ctx) {
        if (dst_doc) pdf_drop_document(ctx, dst_doc);
        if (doc) fz_drop_document(ctx, doc);
        if (stream) fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}
