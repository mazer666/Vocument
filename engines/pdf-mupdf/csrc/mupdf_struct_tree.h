#ifndef VOCUMENT_MUPDF_STRUCT_TREE_H
#define VOCUMENT_MUPDF_STRUCT_TREE_H

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>

/*
 * Detects whether a PDF document contains Tagged PDF / PDF/UA structural markers.
 */
static inline int vocument_is_document_tagged(fz_context *ctx, pdf_document *doc) {
    if (!doc) return 0;
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, doc), PDF_NAME(Root));
    if (!root) return 0;
    pdf_obj *mark_info = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "MarkInfo"));
    pdf_obj *struct_tree_root = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "StructTreeRoot"));
    if (mark_info || struct_tree_root) return 1;

    int page_count = pdf_count_pages(ctx, doc);
    for (int p = 0; p < page_count; p++) {
        pdf_obj *page_ref = pdf_lookup_page_obj(ctx, doc, p);
        if (page_ref && pdf_dict_gets(ctx, page_ref, "StructParents") != NULL) {
            return 1;
        }
    }
    return 0;
}

/*
 * Computes the maximum integer key used in the document's /ParentTree number tree.
 */
static inline int vocument_get_max_struct_parents_key(fz_context *ctx, pdf_document *doc) {
    if (!doc) return -1;
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, doc), PDF_NAME(Root));
    if (!root) return -1;
    pdf_obj *struct_tree_root = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "StructTreeRoot"));
    if (!struct_tree_root) return -1;

    int max_key = -1;
    pdf_obj *pt_next = pdf_dict_gets(ctx, struct_tree_root, "ParentTreeNextKey");
    if (pt_next && pdf_is_int(ctx, pt_next)) {
        max_key = pdf_to_int(ctx, pt_next) - 1;
    }

    pdf_obj *parent_tree = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, struct_tree_root, "ParentTree"));
    if (parent_tree && pdf_is_dict(ctx, parent_tree)) {
        pdf_obj *nums = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, parent_tree, "Nums"));
        if (nums && pdf_is_array(ctx, nums)) {
            int len = pdf_array_len(ctx, nums);
            for (int i = 0; i < len; i += 2) {
                int k = pdf_to_int(ctx, pdf_array_get(ctx, nums, i));
                if (k > max_key) max_key = k;
            }
        }
    }
    return max_key;
}

/*
 * Grafts and merges Tagged PDF structure tree (/StructTreeRoot, /ParentTree, /RoleMap, /K)
 * from src_doc into dst_doc, shifting StructParents on grafted pages by key_offset.
 *
 * Principle:
 * - If both are untagged: returns 0 (consistent untagged result).
 * - If one is tagged and the other is untagged: returns -1 (explicit conflict, fail-closed per ISO 32000-1 / PDF/UA).
 * - If both are tagged: cleanly unites ParentTree and StructTreeRoot, verifying consistency.
 */
static inline int vocument_merge_tagged_structures(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_graft_map *map,
    int dst_page_offset,
    int src_page_count
) {
    int dst_tagged = vocument_is_document_tagged(ctx, dst_doc);
    int src_tagged = vocument_is_document_tagged(ctx, src_doc);

    if (!dst_tagged && !src_tagged) {
        return 0; /* Both untagged: consistent */
    }

    /* Mismatched tagging: fail-closed to avoid silent structural corruption */
    if (dst_tagged != src_tagged) {
        return -1;
    }

    pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    pdf_obj *src_root = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Root));
    if (!dst_root || !src_root) return -1;

    pdf_obj *dst_str = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, dst_root, "StructTreeRoot"));
    pdf_obj *src_str = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, src_root, "StructTreeRoot"));
    if (!dst_str || !src_str) return -1;

    int max_dst_key = vocument_get_max_struct_parents_key(ctx, dst_doc);
    int key_offset = (max_dst_key >= 0) ? (max_dst_key + 1) : 0;

    /* 1. Shift StructParents on newly grafted pages in dst_doc */
    for (int p = 0; p < src_page_count; p++) {
        pdf_obj *page_ref = pdf_lookup_page_obj(ctx, dst_doc, dst_page_offset + p);
        if (!page_ref) continue;
        pdf_obj *sp = pdf_dict_gets(ctx, page_ref, "StructParents");
        if (sp && pdf_is_int(ctx, sp)) {
            int orig_sp = pdf_to_int(ctx, sp);
            pdf_dict_puts_drop(ctx, page_ref, "StructParents", pdf_new_int(ctx, orig_sp + key_offset));
        }

        /* Check annotations on page for StructParent */
        pdf_obj *annots = pdf_dict_get(ctx, page_ref, PDF_NAME(Annots));
        if (annots && pdf_is_array(ctx, annots)) {
            int n_ann = pdf_array_len(ctx, annots);
            for (int a = 0; a < n_ann; a++) {
                pdf_obj *ann = pdf_array_get(ctx, annots, a);
                if (!ann || !pdf_is_dict(ctx, ann)) continue;
                pdf_obj *asp = pdf_dict_gets(ctx, ann, "StructParent");
                if (asp && pdf_is_int(ctx, asp)) {
                    int orig_asp = pdf_to_int(ctx, asp);
                    pdf_dict_puts_drop(ctx, ann, "StructParent", pdf_new_int(ctx, orig_asp + key_offset));
                }
            }
        }
    }

    /* 2. Graft ParentTree Nums from src_doc into dst_doc */
    pdf_obj *src_pt = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, src_str, "ParentTree"));
    if (src_pt) {
        pdf_obj *src_nums = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, src_pt, "Nums"));
        if (src_nums && pdf_is_array(ctx, src_nums)) {
            pdf_obj *dst_pt = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, dst_str, "ParentTree"));
            if (!dst_pt) {
                dst_pt = pdf_new_dict(ctx, dst_doc, 2);
                pdf_obj *nums_arr = pdf_new_array(ctx, dst_doc, 8);
                pdf_dict_puts_drop(ctx, dst_pt, "Nums", nums_arr);
                pdf_dict_puts_drop(ctx, dst_str, "ParentTree", dst_pt);
            }
            pdf_obj *dst_nums = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, dst_pt, "Nums"));
            if (dst_nums && pdf_is_array(ctx, dst_nums)) {
                int src_len = pdf_array_len(ctx, src_nums);
                for (int i = 0; i < src_len; i += 2) {
                    int k = pdf_to_int(ctx, pdf_array_get(ctx, src_nums, i));
                    pdf_obj *val = pdf_array_get(ctx, src_nums, i + 1);
                    pdf_obj *grafted_val = pdf_graft_mapped_object(ctx, map, val);
                    pdf_array_push_int(ctx, dst_nums, k + key_offset);
                    pdf_array_push(ctx, dst_nums, grafted_val);
                }
            }
        }
    }

    /* 3. Graft StructTreeRoot /K from src_doc into dst_doc */
    pdf_obj *src_k = pdf_dict_gets(ctx, src_str, "K");
    if (src_k) {
        pdf_obj *grafted_k = pdf_graft_mapped_object(ctx, map, src_k);
        pdf_obj *dst_k = pdf_dict_gets(ctx, dst_str, "K");
        if (!dst_k) {
            pdf_dict_puts(ctx, dst_str, "K", grafted_k);
        } else if (pdf_is_array(ctx, dst_k)) {
            if (pdf_is_array(ctx, grafted_k)) {
                int g_len = pdf_array_len(ctx, grafted_k);
                for (int g = 0; g < g_len; g++) {
                    pdf_obj *elem = pdf_array_get(ctx, grafted_k, g);
                    if (elem && pdf_is_dict(ctx, elem)) {
                        pdf_dict_puts(ctx, elem, "P", dst_str);
                    }
                    pdf_array_push(ctx, dst_k, elem);
                }
            } else {
                if (pdf_is_dict(ctx, grafted_k)) {
                    pdf_dict_puts(ctx, grafted_k, "P", dst_str);
                }
                pdf_array_push(ctx, dst_k, grafted_k);
            }
        } else if (pdf_is_dict(ctx, dst_k)) {
            pdf_obj *new_k_arr = pdf_new_array(ctx, dst_doc, 4);
            pdf_array_push(ctx, new_k_arr, dst_k);
            if (pdf_is_array(ctx, grafted_k)) {
                int g_len = pdf_array_len(ctx, grafted_k);
                for (int g = 0; g < g_len; g++) {
                    pdf_obj *elem = pdf_array_get(ctx, grafted_k, g);
                    if (elem && pdf_is_dict(ctx, elem)) {
                        pdf_dict_puts(ctx, elem, "P", dst_str);
                    }
                    pdf_array_push(ctx, new_k_arr, elem);
                }
            } else {
                if (pdf_is_dict(ctx, grafted_k)) {
                    pdf_dict_puts(ctx, grafted_k, "P", dst_str);
                }
                pdf_array_push(ctx, new_k_arr, grafted_k);
            }
            pdf_dict_puts_drop(ctx, dst_str, "K", new_k_arr);
        }
    }

    /* 4. Merge RoleMap */
    pdf_obj *src_role = pdf_dict_gets(ctx, src_str, "RoleMap");
    if (src_role && pdf_is_dict(ctx, src_role)) {
        pdf_obj *dst_role = pdf_dict_gets(ctx, dst_str, "RoleMap");
        if (!dst_role) {
            pdf_obj *g_role = pdf_graft_mapped_object(ctx, map, src_role);
            pdf_dict_puts(ctx, dst_str, "RoleMap", g_role);
        } else if (pdf_is_dict(ctx, dst_role)) {
            int n_role = pdf_dict_len(ctx, src_role);
            for (int r = 0; r < n_role; r++) {
                pdf_obj *rk = pdf_dict_get_key(ctx, src_role, r);
                if (!pdf_dict_get(ctx, dst_role, rk)) {
                    pdf_dict_put(ctx, dst_role, rk, pdf_dict_get_val(ctx, src_role, r));
                }
            }
        }
    }

    /* 5. Update ParentTreeNextKey */
    int max_after = vocument_get_max_struct_parents_key(ctx, dst_doc);
    pdf_dict_puts_drop(ctx, dst_str, "ParentTreeNextKey", pdf_new_int(ctx, max_after + 1));

    return 0;
}

/*
 * Preserves Tagged PDF / ParentTree structures when extracting/splitting pages from src_doc into dst_doc.
 */
static inline int vocument_extract_tagged_structures(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_graft_map *map,
    const int *page_indices,
    size_t page_indices_len
) {
    if (!vocument_is_document_tagged(ctx, src_doc)) {
        return 0; /* Source is untagged */
    }

    pdf_obj *src_root = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Root));
    pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    if (!src_root || !dst_root) return -1;

    pdf_obj *src_str = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, src_root, "StructTreeRoot"));
    if (!src_str) return -1;

    /* 1. MarkInfo */
    pdf_obj *src_mark_info = pdf_dict_gets(ctx, src_root, "MarkInfo");
    if (src_mark_info) {
        pdf_obj *dst_mark = pdf_graft_mapped_object(ctx, map, src_mark_info);
        pdf_dict_puts(ctx, dst_root, "MarkInfo", dst_mark);
    }

    /* 2. Destination StructTreeRoot */
    pdf_obj *dst_str = pdf_new_dict(ctx, dst_doc, 4);
    pdf_dict_puts_drop(ctx, dst_str, "Type", pdf_new_name(ctx, "StructTreeRoot"));
    pdf_dict_puts_drop(ctx, dst_root, "StructTreeRoot", dst_str);

    /* 3. Destination ParentTree */
    pdf_obj *dst_pt = pdf_new_dict(ctx, dst_doc, 2);
    pdf_obj *dst_nums = pdf_new_array(ctx, dst_doc, 4);
    pdf_dict_puts_drop(ctx, dst_pt, "Nums", dst_nums);
    pdf_dict_puts_drop(ctx, dst_str, "ParentTree", dst_pt);

    pdf_obj *src_pt = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, src_str, "ParentTree"));
    pdf_obj *src_nums = src_pt ? pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, src_pt, "Nums")) : NULL;

    for (size_t i = 0; i < page_indices_len; i++) {
        int src_p = page_indices[i];
        pdf_obj *src_page_ref = pdf_lookup_page_obj(ctx, src_doc, src_p);
        pdf_obj *dst_page_ref = pdf_lookup_page_obj(ctx, dst_doc, (int)i);
        if (!src_page_ref || !dst_page_ref) continue;

        pdf_obj *sp = pdf_dict_gets(ctx, src_page_ref, "StructParents");
        if (sp && pdf_is_int(ctx, sp)) {
            int orig_sp = pdf_to_int(ctx, sp);
            pdf_dict_puts_drop(ctx, dst_page_ref, "StructParents", pdf_new_int(ctx, (int)i));

            if (src_nums && pdf_is_array(ctx, src_nums)) {
                int s_len = pdf_array_len(ctx, src_nums);
                for (int n = 0; n < s_len; n += 2) {
                    int k = pdf_to_int(ctx, pdf_array_get(ctx, src_nums, n));
                    if (k == orig_sp) {
                        pdf_obj *val = pdf_array_get(ctx, src_nums, n + 1);
                        pdf_obj *grafted_val = pdf_graft_mapped_object(ctx, map, val);
                        pdf_array_push_int(ctx, dst_nums, (int)i);
                        pdf_array_push(ctx, dst_nums, grafted_val);
                        break;
                    }
                }
            }
        }
    }

    /* 4. RoleMap */
    pdf_obj *src_role = pdf_dict_gets(ctx, src_str, "RoleMap");
    if (src_role) {
        pdf_obj *g_role = pdf_graft_mapped_object(ctx, map, src_role);
        pdf_dict_puts(ctx, dst_str, "RoleMap", g_role);
    }

    /* 5. StructTreeRoot /K */
    pdf_obj *src_k = pdf_dict_gets(ctx, src_str, "K");
    if (src_k) {
        pdf_obj *g_k = pdf_graft_mapped_object(ctx, map, src_k);
        pdf_dict_puts(ctx, dst_str, "K", g_k);
    }

    pdf_dict_puts_drop(ctx, dst_str, "ParentTreeNextKey", pdf_new_int(ctx, (int)page_indices_len));

    return 0;
}

#endif /* VOCUMENT_MUPDF_STRUCT_TREE_H */
