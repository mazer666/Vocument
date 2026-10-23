#ifndef VOCUMENT_MUPDF_PAGE_HELPERS_H
#define VOCUMENT_MUPDF_PAGE_HELPERS_H

#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/*
 * Pushes down inheritable attributes (MediaBox, CropBox, Resources, Rotate)
 * from parent /Pages nodes directly into the page dictionary so that the page
 * is fully self-contained regardless of page tree restructuring.
 */
static void make_page_self_contained(fz_context *ctx, pdf_obj *page_ref) {
    static pdf_obj * const inheritable_keys[] = {
        PDF_NAME(MediaBox),
        PDF_NAME(CropBox),
        PDF_NAME(Resources),
        PDF_NAME(Rotate)
    };
    for (size_t k = 0; k < sizeof(inheritable_keys) / sizeof(inheritable_keys[0]); k++) {
        pdf_obj *key = inheritable_keys[k];
        if (!pdf_dict_get(ctx, page_ref, key)) {
            pdf_obj *inherited = pdf_dict_get_inheritable(ctx, page_ref, key);
            if (inherited) {
                pdf_dict_put(ctx, page_ref, key, inherited);
            }
        }
    }
}

static int is_same_object(fz_context *ctx, pdf_obj *a, pdf_obj *b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    int num_a = pdf_to_num(ctx, a);
    int num_b = pdf_to_num(ctx, b);
    if (num_a != 0 && num_a == num_b) return 1;
    return pdf_objcmp_resolve(ctx, a, b) == 0;
}

typedef struct FieldCycleNode {
    pdf_obj *obj;
    struct FieldCycleNode *parent;
} FieldCycleNode;

static int field_cycle_detect(fz_context *ctx, pdf_obj *obj, const FieldCycleNode *ancestors) {
    if (!obj) return 0;
    int target_num = pdf_to_num(ctx, obj);
    const FieldCycleNode *curr = ancestors;
    while (curr) {
        if (curr->obj == obj) return 1;
        if (target_num != 0 && pdf_to_num(ctx, curr->obj) == target_num) return 1;
        curr = curr->parent;
    }
    return 0;
}

static int count_active_signatures_in_field(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj *field,
    int depth,
    const FieldCycleNode *ancestors
) {
    if (!field) return 0;
    if (depth > 32 || field_cycle_detect(ctx, field, ancestors)) {
        return -1; /* Fail-closed on excessive depth or cyclic structure */
    }
    int active_sigs = 0;
    pdf_obj *ft = pdf_dict_get_inheritable(ctx, field, PDF_NAME(FT));
    if (pdf_name_eq(ctx, ft, PDF_NAME(Sig))) {
        if (pdf_signature_is_signed(ctx, pdf_doc, field)) {
            active_sigs++;
        }
    }
    pdf_obj *kids = pdf_dict_get(ctx, field, PDF_NAME(Kids));
    if (pdf_is_array(ctx, kids)) {
        int n_kids = pdf_array_len(ctx, kids);
        FieldCycleNode current_node = { field, (FieldCycleNode *)ancestors };
        for (int k = 0; k < n_kids; k++) {
            pdf_obj *kid = pdf_array_get(ctx, kids, k);
            int sub = count_active_signatures_in_field(ctx, pdf_doc, kid, depth + 1, &current_node);
            if (sub < 0) return -1;
            active_sigs += sub;
        }
    }
    return active_sigs;
}

/*
 * Returns the number of active, signed digital signatures in the document.
 * Recursively inspects nested form field hierarchies (/Kids) and page annotations.
 * Returns -1 if the field structure is cyclic, overly deep (>32), or unexaminable (fail-closed).
 */
static int count_active_signatures(fz_context *ctx, pdf_document *pdf_doc) {
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
    if (!root) return 0;
    int active_sigs = 0;

    pdf_obj *acroform = pdf_dict_get(ctx, root, PDF_NAME(AcroForm));
    if (acroform) {
        pdf_obj *fields = pdf_dict_get(ctx, acroform, PDF_NAME(Fields));
        if (pdf_is_array(ctx, fields)) {
            int n_fields = pdf_array_len(ctx, fields);
            for (int i = 0; i < n_fields; i++) {
                pdf_obj *f = pdf_array_get(ctx, fields, i);
                int sub = count_active_signatures_in_field(ctx, pdf_doc, f, 0, NULL);
                if (sub < 0) return -1; /* Fail-closed on unexaminable structure */
                active_sigs += sub;
            }
        }
    }

    /* Check page annotations for standalone widget signatures not in /Fields */
    int page_count = pdf_count_pages(ctx, pdf_doc);
    for (int p = 0; p < page_count; p++) {
        pdf_obj *page_ref = pdf_lookup_page_obj(ctx, pdf_doc, p);
        if (!page_ref) continue;
        pdf_obj *annots = pdf_dict_get(ctx, page_ref, PDF_NAME(Annots));
        if (!pdf_is_array(ctx, annots)) continue;
        int n_annots = pdf_array_len(ctx, annots);
        for (int a = 0; a < n_annots; a++) {
            pdf_obj *annot = pdf_array_get(ctx, annots, a);
            if (!pdf_is_dict(ctx, annot)) continue;
            pdf_obj *subtype = pdf_dict_get(ctx, annot, PDF_NAME(Subtype));
            if (pdf_name_eq(ctx, subtype, PDF_NAME(Widget))) {
                pdf_obj *ft = pdf_dict_get_inheritable(ctx, annot, PDF_NAME(FT));
                if (pdf_name_eq(ctx, ft, PDF_NAME(Sig))) {
                    if (pdf_signature_is_signed(ctx, pdf_doc, annot)) {
                        pdf_obj *p_field = acroform ? pdf_dict_get(ctx, acroform, PDF_NAME(Fields)) : NULL;
                        if (!p_field) {
                            active_sigs++;
                        }
                    }
                }
            }
        }
    }
    return active_sigs;
}

static void mark_signatures_invalidated_in_field(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj *field,
    int depth,
    const FieldCycleNode *ancestors
) {
    if (!field || depth > 32 || field_cycle_detect(ctx, field, ancestors)) return;
    pdf_obj *ft = pdf_dict_get_inheritable(ctx, field, PDF_NAME(FT));
    if (pdf_name_eq(ctx, ft, PDF_NAME(Sig))) {
        pdf_dict_del(ctx, field, PDF_NAME(V));
    }
    pdf_obj *kids = pdf_dict_get(ctx, field, PDF_NAME(Kids));
    if (pdf_is_array(ctx, kids)) {
        int n_kids = pdf_array_len(ctx, kids);
        FieldCycleNode current_node = { field, (FieldCycleNode *)ancestors };
        for (int k = 0; k < n_kids; k++) {
            pdf_obj *kid = pdf_array_get(ctx, kids, k);
            mark_signatures_invalidated_in_field(ctx, pdf_doc, kid, depth + 1, &current_node);
        }
    }
}

/*
 * When signature invalidation is explicitly permitted, marks existing signatures
 * as invalidated by stripping /V per ISO 32000-1 §12.8, creating a clean unsigned working copy.
 * Recursively cleans nested signature field hierarchies and page widget annotations.
 */
static void mark_signatures_invalidated(fz_context *ctx, pdf_document *pdf_doc) {
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
    if (!root) return;
    pdf_obj *acroform = pdf_dict_get(ctx, root, PDF_NAME(AcroForm));
    if (acroform) {
        pdf_obj *fields = pdf_dict_get(ctx, acroform, PDF_NAME(Fields));
        if (pdf_is_array(ctx, fields)) {
            int n_fields = pdf_array_len(ctx, fields);
            for (int i = 0; i < n_fields; i++) {
                pdf_obj *f = pdf_array_get(ctx, fields, i);
                mark_signatures_invalidated_in_field(ctx, pdf_doc, f, 0, NULL);
            }
        }
    }

    int page_count = pdf_count_pages(ctx, pdf_doc);
    for (int p = 0; p < page_count; p++) {
        pdf_obj *page_ref = pdf_lookup_page_obj(ctx, pdf_doc, p);
        if (!page_ref) continue;
        pdf_obj *annots = pdf_dict_get(ctx, page_ref, PDF_NAME(Annots));
        if (!pdf_is_array(ctx, annots)) continue;
        int n_annots = pdf_array_len(ctx, annots);
        for (int a = 0; a < n_annots; a++) {
            pdf_obj *annot = pdf_array_get(ctx, annots, a);
            if (!pdf_is_dict(ctx, annot)) continue;
            pdf_obj *subtype = pdf_dict_get(ctx, annot, PDF_NAME(Subtype));
            if (pdf_name_eq(ctx, subtype, PDF_NAME(Widget))) {
                pdf_obj *ft = pdf_dict_get_inheritable(ctx, annot, PDF_NAME(FT));
                if (pdf_name_eq(ctx, ft, PDF_NAME(Sig))) {
                    pdf_dict_del(ctx, annot, PDF_NAME(V));
                }
            }
        }
    }
}

/*
 * Traverses outline bookmarks and re-anchors any bookmark targeting a deleted page
 * to the nearest surviving page target.
 */
static void reanchor_outline_item(
    fz_context *ctx,
    pdf_obj *item,
    pdf_obj **del_pages,
    int del_count,
    pdf_obj *surviving_target
) {
    if (!item || !pdf_is_dict(ctx, item)) return;

    pdf_obj *dest = pdf_dict_get(ctx, item, PDF_NAME(Dest));
    if (!dest) {
        pdf_obj *a = pdf_dict_get(ctx, item, PDF_NAME(A));
        if (pdf_is_dict(ctx, a)) {
            pdf_obj *s = pdf_dict_get(ctx, a, PDF_NAME(S));
            if (pdf_name_eq(ctx, s, PDF_NAME(GoTo))) {
                dest = pdf_dict_get(ctx, a, PDF_NAME(D));
            }
        }
    }

    if (dest && pdf_is_array(ctx, dest) && pdf_array_len(ctx, dest) > 0) {
        pdf_obj *target_page = pdf_array_get(ctx, dest, 0);
        if (target_page) {
            for (int d = 0; d < del_count; d++) {
                if (is_same_object(ctx, target_page, del_pages[d])) {
                    if (surviving_target) {
                        pdf_array_put(ctx, dest, 0, surviving_target);
                    }
                    break;
                }
            }
        }
    }

    pdf_obj *first_child = pdf_dict_get(ctx, item, PDF_NAME(First));
    while (first_child && pdf_is_dict(ctx, first_child)) {
        reanchor_outline_item(ctx, first_child, del_pages, del_count, surviving_target);
        first_child = pdf_dict_get(ctx, first_child, PDF_NAME(Next));
    }
}

/*
 * Cleans up /AcroForm /Fields references to widget annotations located on deleted pages.
 */
static void cleanup_acroform_fields_for_deleted_pages(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj **del_pages,
    int del_count
) {
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
    if (!root) return;
    pdf_obj *acroform = pdf_dict_get(ctx, root, PDF_NAME(AcroForm));
    if (!acroform) return;
    pdf_obj *fields = pdf_dict_get(ctx, acroform, PDF_NAME(Fields));
    if (!pdf_is_array(ctx, fields)) return;

    for (int p = 0; p < del_count; p++) {
        pdf_obj *annots = pdf_dict_get(ctx, del_pages[p], PDF_NAME(Annots));
        if (!pdf_is_array(ctx, annots)) continue;
        int n_annots = pdf_array_len(ctx, annots);
        for (int a = 0; a < n_annots; a++) {
            pdf_obj *annot = pdf_array_get(ctx, annots, a);
            for (int f = pdf_array_len(ctx, fields) - 1; f >= 0; f--) {
                pdf_obj *field = pdf_array_get(ctx, fields, f);
                if (is_same_object(ctx, annot, field)) {
                    pdf_array_delete(ctx, fields, f);
                    break;
                }
            }
        }
    }
}

static int field_name_exists_in_fields(fz_context *ctx, pdf_obj *fields, const char *name, int depth) {
    if (!fields || !name || depth > 32) return 0;
    if (pdf_is_array(ctx, fields)) {
        int len = pdf_array_len(ctx, fields);
        for (int i = 0; i < len; i++) {
            pdf_obj *f = pdf_array_get(ctx, fields, i);
            if (field_name_exists_in_fields(ctx, f, name, depth + 1)) return 1;
        }
        return 0;
    }
    if (pdf_is_dict(ctx, fields)) {
        pdf_obj *t = pdf_dict_get(ctx, fields, PDF_NAME(T));
        if (t) {
            const char *existing = pdf_to_text_string(ctx, t);
            if (existing && strcmp(existing, name) == 0) return 1;
        }
        pdf_obj *kids = pdf_dict_get(ctx, fields, PDF_NAME(Kids));
        if (kids) {
            if (field_name_exists_in_fields(ctx, kids, name, depth + 1)) return 1;
        }
    }
    return 0;
}

static void generate_unique_field_name(fz_context *ctx, pdf_obj *fields, const char *orig_name, char *out_buf, size_t out_buf_len) {
    const char *base = (orig_name && strlen(orig_name) > 0) ? orig_name : "Field";
    char base_clean[256];
    strncpy(base_clean, base, sizeof(base_clean) - 1);
    base_clean[sizeof(base_clean) - 1] = '\0';

    char *copy_pos = strstr(base_clean, "_Copy");
    if (copy_pos) {
        char *p = copy_pos + 5;
        int all_digits = (*p != '\0');
        while (*p) {
            if (*p < '0' || *p > '9') { all_digits = 0; break; }
            p++;
        }
        if (all_digits) {
            *copy_pos = '\0';
        }
    }

    for (int copy_idx = 1; copy_idx < 10000; copy_idx++) {
        snprintf(out_buf, out_buf_len, "%s_Copy%d", base_clean, copy_idx);
        if (!field_name_exists_in_fields(ctx, fields, out_buf, 0)) {
            return;
        }
    }
    snprintf(out_buf, out_buf_len, "%s_Copy_fallback", base_clean);
}

static void reanchor_internal_links_for_deleted_pages(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj **del_pages,
    int del_count,
    pdf_obj *surviving_target
) {
    if (!del_pages || del_count == 0 || !surviving_target) return;
    int page_count = pdf_count_pages(ctx, pdf_doc);
    for (int i = 0; i < page_count; i++) {
        pdf_obj *page_ref = pdf_lookup_page_obj(ctx, pdf_doc, i);
        if (!page_ref) continue;
        int is_del = 0;
        for (int d = 0; d < del_count; d++) {
            if (is_same_object(ctx, page_ref, del_pages[d])) {
                is_del = 1; break;
            }
        }
        if (is_del) continue;

        pdf_obj *annots = pdf_dict_get(ctx, page_ref, PDF_NAME(Annots));
        if (!pdf_is_array(ctx, annots)) continue;
        int n_annots = pdf_array_len(ctx, annots);
        for (int a = 0; a < n_annots; a++) {
            pdf_obj *annot = pdf_array_get(ctx, annots, a);
            if (!pdf_is_dict(ctx, annot)) continue;
            pdf_obj *subtype = pdf_dict_get(ctx, annot, PDF_NAME(Subtype));
            if (!pdf_name_eq(ctx, subtype, PDF_NAME(Link))) continue;

            pdf_obj *dest = pdf_dict_get(ctx, annot, PDF_NAME(Dest));
            if (pdf_is_array(ctx, dest) && pdf_array_len(ctx, dest) > 0) {
                pdf_obj *target = pdf_array_get(ctx, dest, 0);
                for (int d = 0; d < del_count; d++) {
                    if (is_same_object(ctx, target, del_pages[d])) {
                        pdf_array_put(ctx, dest, 0, surviving_target);
                        break;
                    }
                }
            }

            pdf_obj *act = pdf_dict_get(ctx, annot, PDF_NAME(A));
            if (pdf_is_dict(ctx, act)) {
                pdf_obj *s = pdf_dict_get(ctx, act, PDF_NAME(S));
                if (pdf_name_eq(ctx, s, PDF_NAME(GoTo))) {
                    pdf_obj *d_obj = pdf_dict_get(ctx, act, PDF_NAME(D));
                    if (pdf_is_array(ctx, d_obj) && pdf_array_len(ctx, d_obj) > 0) {
                        pdf_obj *target = pdf_array_get(ctx, d_obj, 0);
                        for (int d = 0; d < del_count; d++) {
                            if (is_same_object(ctx, target, del_pages[d])) {
                                pdf_array_put(ctx, d_obj, 0, surviving_target);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

static void restructure_pagelabels_for_reorder(fz_context *ctx, pdf_document *pdf_doc, const int *order, int count) {
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
    if (!root) return;
    pdf_obj *pagelabels = pdf_dict_get(ctx, root, PDF_NAME(PageLabels));
    if (!pagelabels) return;
    pdf_obj *nums = pdf_dict_get(ctx, pagelabels, PDF_NAME(Nums));
    if (!pdf_is_array(ctx, nums)) return;

    pdf_obj **old_labels = (pdf_obj **)fz_calloc(ctx, (size_t)count, sizeof(pdf_obj *));
    int nums_len = pdf_array_len(ctx, nums);
    pdf_obj *current_label = NULL;
    int next_num_idx = 0;

    for (int p = 0; p < count; p++) {
        while (next_num_idx + 1 < nums_len && pdf_to_int(ctx, pdf_array_get(ctx, nums, next_num_idx)) <= p) {
            current_label = pdf_array_get(ctx, nums, next_num_idx + 1);
            next_num_idx += 2;
        }
        old_labels[p] = current_label;
    }

    pdf_obj *new_nums = pdf_new_array(ctx, pdf_doc, count * 2);
    pdf_obj *last_written_label = NULL;
    for (int j = 0; j < count; j++) {
        int orig_p = order[j];
        pdf_obj *lbl = old_labels[orig_p];
        if (lbl && (!last_written_label || is_same_object(ctx, lbl, last_written_label) == 0)) {
            pdf_array_push_int(ctx, new_nums, j);
            pdf_array_push(ctx, new_nums, lbl);
            last_written_label = lbl;
        }
    }
    fz_free(ctx, old_labels);

    if (pdf_array_len(ctx, new_nums) > 0) {
        pdf_dict_put_drop(ctx, pagelabels, PDF_NAME(Nums), new_nums);
    } else {
        pdf_drop_obj(ctx, new_nums);
    }
}

static void cleanup_parent_tree_for_deleted_pages(
    fz_context *ctx,
    pdf_document *pdf_doc,
    pdf_obj **del_pages,
    int del_count
) {
    if (!del_pages || del_count == 0) return;
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
    if (!root) return;
    pdf_obj *struct_tree_root = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "StructTreeRoot"));
    if (!struct_tree_root) return;
    pdf_obj *parent_tree = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, struct_tree_root, "ParentTree"));
    if (!parent_tree) return;
    pdf_obj *nums = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, parent_tree, "Nums"));
    if (!pdf_is_array(ctx, nums)) return;

    for (int d = 0; d < del_count; d++) {
        pdf_obj *sp_obj = pdf_dict_gets(ctx, del_pages[d], "StructParents");
        if (sp_obj) {
            int sp = pdf_to_int(ctx, sp_obj);
            int nums_len = pdf_array_len(ctx, nums);
            for (int n = 0; n < nums_len; n += 2) {
                int k = pdf_to_int(ctx, pdf_array_get(ctx, nums, n));
                if (k == sp) {
                    pdf_array_delete(ctx, nums, n + 1);
                    pdf_array_delete(ctx, nums, n);
                    break;
                }
            }
        }
    }
}

/*
 * Validates Tagged PDF / PDF/UA structural consistency per ISO 32000-1 §14.7 & §14.8.
 * Returns 1 if valid or document is untagged.
 * Returns 0 if Tagged PDF markers exist but /StructTreeRoot, /ParentTree,
 * or page /StructParents mappings are broken, incomplete, or dangling.
 */
static int validate_tagged_pdf_consistency(fz_context *ctx, pdf_document *pdf_doc) {
    if (!pdf_doc) return 0;
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, pdf_doc), PDF_NAME(Root));
    if (!root) return 0;

    pdf_obj *mark_info = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "MarkInfo"));
    pdf_obj *struct_tree_root = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, root, "StructTreeRoot"));

    int page_count = pdf_count_pages(ctx, pdf_doc);
    int any_page_has_sp = 0;
    for (int p = 0; p < page_count; p++) {
        pdf_obj *page_ref = pdf_lookup_page_obj(ctx, pdf_doc, p);
        if (page_ref && pdf_dict_gets(ctx, page_ref, "StructParents") != NULL) {
            any_page_has_sp = 1;
            break;
        }
    }

    if (!mark_info && !struct_tree_root && !any_page_has_sp) {
        return 1; /* Untagged document */
    }

    /* If marked or has struct tree or pages reference StructParents, StructTreeRoot MUST exist */
    if (!struct_tree_root || !pdf_is_dict(ctx, struct_tree_root)) {
        return 0;
    }

    /* Check ParentTree if any pages have StructParents */
    if (any_page_has_sp) {
        pdf_obj *parent_tree = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, struct_tree_root, "ParentTree"));
        if (!parent_tree || !pdf_is_dict(ctx, parent_tree)) {
            return 0;
        }
        pdf_obj *nums = pdf_resolve_indirect(ctx, pdf_dict_gets(ctx, parent_tree, "Nums"));
        if (!nums || !pdf_is_array(ctx, nums)) {
            return 0;
        }
        int nums_len = pdf_array_len(ctx, nums);
        if (nums_len % 2 != 0) {
            return 0;
        }

        /* Verify every page with /StructParents has a corresponding entry in Nums */
        for (int p = 0; p < page_count; p++) {
            pdf_obj *page_ref = pdf_lookup_page_obj(ctx, pdf_doc, p);
            if (!page_ref) continue;
            pdf_obj *sp_obj = pdf_dict_gets(ctx, page_ref, "StructParents");
            if (sp_obj) {
                if (!pdf_is_int(ctx, sp_obj)) return 0;
                int sp_val = pdf_to_int(ctx, sp_obj);
                int found = 0;
                for (int n = 0; n < nums_len; n += 2) {
                    int k = pdf_to_int(ctx, pdf_array_get(ctx, nums, n));
                    if (k == sp_val) {
                        found = 1;
                        break;
                    }
                }
                if (!found) {
                    return 0; /* Dangling /StructParents reference */
                }
            }
        }
    }
    return 1;
}

#endif /* VOCUMENT_MUPDF_PAGE_HELPERS_H */
