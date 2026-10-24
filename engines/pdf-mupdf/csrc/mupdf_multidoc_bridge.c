#include <mupdf/fitz.h>
#include <mupdf/pdf.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "mupdf_serialize.h"
#include "mupdf_page_helpers.h"
#include "mupdf_struct_tree.h"

void vocument_mupdf_free_bytes(unsigned char *ptr);

/*
 * Validates that a link destination target resolves to a valid page in doc.
 */
static int is_destination_target_valid_in_doc(fz_context *ctx, pdf_document *doc, pdf_obj *dest) {
    if (!dest) return 0;
    dest = pdf_resolve_indirect(ctx, dest);
    if (pdf_is_array(ctx, dest)) {
        if (pdf_array_len(ctx, dest) > 0) {
            pdf_obj *target = pdf_array_get(ctx, dest, 0);
            if (pdf_is_int(ctx, target)) {
                int pno = pdf_to_int(ctx, target);
                return (pno >= 0 && pno < pdf_count_pages(ctx, doc));
            }
            if (target) {
                if (pdf_lookup_page_number(ctx, doc, target) >= 0) return 1;
                pdf_obj *resolved = pdf_resolve_indirect(ctx, target);
                if (resolved && pdf_lookup_page_number(ctx, doc, resolved) >= 0) return 1;
                int count = pdf_count_pages(ctx, doc);
                for (int i = 0; i < count; i++) {
                    pdf_obj *p = pdf_lookup_page_obj(ctx, doc, i);
                    if (is_same_object(ctx, p, target) || (resolved && is_same_object(ctx, p, resolved))) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    } else if (pdf_is_dict(ctx, dest)) {
        pdf_obj *d = pdf_dict_get(ctx, dest, PDF_NAME(D));
        return is_destination_target_valid_in_doc(ctx, doc, d);
    }
    return 1;
}

/*
 * Re-maps a destination pointing to a page in src_doc to the corresponding grafted page in dst_doc.
 */
static pdf_obj *remap_destination_to_grafted_pages(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_obj *dest,
    int page_offset,
    const int *source_page_indices,
    size_t source_page_indices_len
) {
    if (!dest) return NULL;
    pdf_obj *res_dest = pdf_resolve_indirect(ctx, dest);
    if (!pdf_is_array(ctx, res_dest) || pdf_array_len(ctx, res_dest) == 0) {
        return NULL;
    }
    pdf_obj *target = pdf_array_get(ctx, res_dest, 0);
    int src_pno = -1;
    if (pdf_is_int(ctx, target)) {
        src_pno = pdf_to_int(ctx, target);
    } else {
        src_pno = pdf_lookup_page_number(ctx, src_doc, target);
        if (src_pno < 0) {
            pdf_obj *res_target = pdf_resolve_indirect(ctx, target);
            if (res_target) src_pno = pdf_lookup_page_number(ctx, src_doc, res_target);
        }
    }
    if (src_pno < 0) {
        if (is_destination_target_valid_in_doc(ctx, dst_doc, res_dest)) {
            return dest;
        }
        return NULL;
    }

    int dst_pno = -1;
    if (source_page_indices && source_page_indices_len > 0) {
        for (size_t i = 0; i < source_page_indices_len; i++) {
            if (source_page_indices[i] == src_pno) {
                dst_pno = page_offset + (int)i;
                break;
            }
        }
        if (dst_pno < 0) {
            /* The target page was not among the selected inserted pages */
            return NULL;
        }
    } else {
        dst_pno = page_offset + src_pno;
    }

    if (dst_pno < 0 || dst_pno >= pdf_count_pages(ctx, dst_doc)) {
        return NULL;
    }
    pdf_obj *correct_dst_page = pdf_lookup_page_obj(ctx, dst_doc, dst_pno);
    if (!correct_dst_page) return NULL;

    int len = pdf_array_len(ctx, res_dest);
    pdf_obj *new_dest = pdf_new_array(ctx, dst_doc, len);
    pdf_array_push(ctx, new_dest, correct_dst_page);
    for (int i = 1; i < len; i++) {
        pdf_array_push(ctx, new_dest, pdf_array_get(ctx, res_dest, i));
    }
    return new_dest;
}

/*
 * Sorts key-value pairs in a NameTree /Names array lexicographically by key (ISO 32000-1 §7.9.6).
 */
static void sort_name_tree_names(fz_context *ctx, pdf_obj *arr) {
    if (!arr || !pdf_is_array(ctx, arr)) return;
    int len = pdf_array_len(ctx, arr);
    if (len < 4) return;
    int n_pairs = len / 2;
    for (int i = 0; i < n_pairs - 1; i++) {
        for (int j = i + 1; j < n_pairs; j++) {
            pdf_obj *k1 = pdf_array_get(ctx, arr, i * 2);
            pdf_obj *k2 = pdf_array_get(ctx, arr, j * 2);
            const char *s1 = pdf_to_text_string(ctx, k1);
            const char *s2 = pdf_to_text_string(ctx, k2);
            if (s1 && s2 && strcmp(s1, s2) > 0) {
                pdf_obj *v1 = pdf_array_get(ctx, arr, i * 2 + 1);
                pdf_obj *v2 = pdf_array_get(ctx, arr, j * 2 + 1);
                pdf_array_put(ctx, arr, i * 2, k2);
                pdf_array_put(ctx, arr, i * 2 + 1, v2);
                pdf_array_put(ctx, arr, j * 2, k1);
                pdf_array_put(ctx, arr, j * 2 + 1, v1);
            }
        }
    }
}

/*
 * Neutralizes or re-maps internal link annotations on a grafted page.
 */
static void sanitize_links_for_grafted_page(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_obj *dst_page_ref,
    int page_offset,
    const int *source_page_indices,
    size_t source_page_indices_len
) {
    pdf_obj *annots = pdf_dict_get(ctx, dst_page_ref, PDF_NAME(Annots));
    if (!pdf_is_array(ctx, annots)) return;

    for (int a = pdf_array_len(ctx, annots) - 1; a >= 0; a--) {
        pdf_obj *annot = pdf_array_get(ctx, annots, a);
        if (!pdf_is_dict(ctx, annot)) continue;

        pdf_obj *subtype = pdf_dict_get(ctx, annot, PDF_NAME(Subtype));
        if (!pdf_name_eq(ctx, subtype, PDF_NAME(Link))) continue;

        pdf_obj *dest = pdf_dict_get(ctx, annot, PDF_NAME(Dest));
        if (dest) {
            pdf_obj *remapped = remap_destination_to_grafted_pages(ctx, dst_doc, src_doc, dest, page_offset, source_page_indices, source_page_indices_len);
            if (remapped) {
                pdf_dict_put(ctx, annot, PDF_NAME(Dest), remapped);
            } else if (!is_destination_target_valid_in_doc(ctx, dst_doc, dest)) {
                pdf_array_delete(ctx, annots, a);
                continue;
            }
        }

        pdf_obj *act = pdf_dict_get(ctx, annot, PDF_NAME(A));
        if (pdf_is_dict(ctx, act)) {
            pdf_obj *s = pdf_dict_get(ctx, act, PDF_NAME(S));
            if (pdf_name_eq(ctx, s, PDF_NAME(GoTo))) {
                pdf_obj *d_obj = pdf_dict_get(ctx, act, PDF_NAME(D));
                if (d_obj) {
                    pdf_obj *remapped = remap_destination_to_grafted_pages(ctx, dst_doc, src_doc, d_obj, page_offset, source_page_indices, source_page_indices_len);
                    if (remapped) {
                        pdf_dict_put(ctx, act, PDF_NAME(D), remapped);
                    } else if (!is_destination_target_valid_in_doc(ctx, dst_doc, d_obj)) {
                        pdf_array_delete(ctx, annots, a);
                    }
                }
            }
        }
    }
}

/*
 * Grafts Named Destinations (/Dests and /Names /Dests) from src_doc into dst_doc,
 * re-mapping page targets to the correct newly grafted pages and resolving name collisions.
 */
static void append_grafted_named_dests(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_graft_map *map,
    int page_offset,
    const char *collision_suffix
) {
    pdf_obj *src_root = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Root));
    if (!src_root) return;
    pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    if (!dst_root) return;

    /* 1. Root /Dests dictionary */
    pdf_obj *src_dests = pdf_dict_get(ctx, src_root, PDF_NAME(Dests));
    if (src_dests && pdf_is_dict(ctx, src_dests)) {
        pdf_obj *dst_dests = pdf_dict_get(ctx, dst_root, PDF_NAME(Dests));
        if (!dst_dests) {
            dst_dests = pdf_new_dict(ctx, dst_doc, 8);
            pdf_dict_put_drop(ctx, dst_root, PDF_NAME(Dests), dst_dests);
        }
        int n = pdf_dict_len(ctx, src_dests);
        for (int i = 0; i < n; i++) {
            pdf_obj *k = pdf_dict_get_key(ctx, src_dests, i);
            pdf_obj *v = pdf_dict_get_val(ctx, src_dests, i);
            pdf_obj *remapped = remap_destination_to_grafted_pages(ctx, dst_doc, src_doc, v, page_offset, NULL, 0);
            pdf_obj *target_v = remapped;
            if (!target_v) {
                pdf_obj *grafted_v = pdf_graft_mapped_object(ctx, map, v);
                if (is_destination_target_valid_in_doc(ctx, dst_doc, grafted_v)) {
                    target_v = grafted_v;
                }
            }
            if (target_v) {
                pdf_obj *target_k = k;
                if (pdf_dict_get(ctx, dst_dests, k)) {
                    /* Disambiguate colliding destination name */
                    const char *orig_key = pdf_to_name(ctx, k);
                    if (!orig_key || !orig_key[0]) orig_key = pdf_to_text_string(ctx, k);
                    char disambiguated[256];
                    snprintf(disambiguated, sizeof(disambiguated), "%s_%s", orig_key, collision_suffix ? collision_suffix : "Grafted");
                    target_k = pdf_new_name(ctx, disambiguated);
                }
                pdf_dict_put(ctx, dst_dests, target_k, target_v);
            }
        }
    }

    /* 2. Root /Names /Dests NameTree */
    pdf_obj *src_names = pdf_dict_get(ctx, src_root, PDF_NAME(Names));
    if (src_names && pdf_is_dict(ctx, src_names)) {
        pdf_obj *src_nd = pdf_dict_get(ctx, src_names, PDF_NAME(Dests));
        if (src_nd && pdf_is_dict(ctx, src_nd)) {
            pdf_obj *src_arr = pdf_dict_get(ctx, src_nd, PDF_NAME(Names));
            if (pdf_is_array(ctx, src_arr)) {
                pdf_obj *dst_names = pdf_dict_get(ctx, dst_root, PDF_NAME(Names));
                if (!dst_names) {
                    dst_names = pdf_new_dict(ctx, dst_doc, 4);
                    pdf_dict_put_drop(ctx, dst_root, PDF_NAME(Names), dst_names);
                }
                pdf_obj *dst_nd = pdf_dict_get(ctx, dst_names, PDF_NAME(Dests));
                if (!dst_nd) {
                    dst_nd = pdf_new_dict(ctx, dst_doc, 4);
                    pdf_obj *names_arr = pdf_new_array(ctx, dst_doc, 8);
                    pdf_dict_put_drop(ctx, dst_nd, PDF_NAME(Names), names_arr);
                    pdf_dict_put_drop(ctx, dst_names, PDF_NAME(Dests), dst_nd);
                }
                pdf_obj *dst_arr = pdf_dict_get(ctx, dst_nd, PDF_NAME(Names));
                if (pdf_is_array(ctx, dst_arr)) {
                    int len = pdf_array_len(ctx, src_arr);
                    for (int i = 0; i < len; i += 2) {
                        pdf_obj *k = pdf_array_get(ctx, src_arr, i);
                        pdf_obj *v = pdf_array_get(ctx, src_arr, i + 1);
                        pdf_obj *remapped = remap_destination_to_grafted_pages(ctx, dst_doc, src_doc, v, page_offset, NULL, 0);
                        pdf_obj *target_v = remapped;
                        if (!target_v) {
                            pdf_obj *grafted_v = pdf_graft_mapped_object(ctx, map, v);
                            if (is_destination_target_valid_in_doc(ctx, dst_doc, grafted_v)) {
                                target_v = grafted_v;
                            }
                        }
                        if (target_v) {
                            pdf_obj *target_k = k;
                            const char *orig_key = pdf_to_text_string(ctx, k);
                            if (orig_key) {
                                int existing_len = pdf_array_len(ctx, dst_arr);
                                for (int e = 0; e < existing_len; e += 2) {
                                    pdf_obj *ex_k = pdf_array_get(ctx, dst_arr, e);
                                    const char *ex_str = pdf_to_text_string(ctx, ex_k);
                                    if (ex_str && strcmp(ex_str, orig_key) == 0) {
                                        char disambiguated[256];
                                        snprintf(disambiguated, sizeof(disambiguated), "%s_%s", orig_key, collision_suffix ? collision_suffix : "Grafted");
                                        target_k = pdf_new_text_string(ctx, disambiguated);
                                        break;
                                    }
                                }
                            }
                            pdf_array_push(ctx, dst_arr, target_k);
                            pdf_array_push(ctx, dst_arr, target_v);
                        }
                    }
                    /* Maintain sorted NameTree property (ISO 32000-1 §7.9.6) */
                    sort_name_tree_names(ctx, dst_arr);
                }
            }
        }
    }
}

/*
 * Appends grafted outline items from a source document into the destination document's outlines tree.
 */
static void append_grafted_outlines(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_graft_map *map
) {
    pdf_obj *src_root = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Root));
    if (!src_root) return;
    pdf_obj *src_outlines = pdf_dict_get(ctx, src_root, PDF_NAME(Outlines));
    if (!src_outlines || !pdf_is_dict(ctx, src_outlines)) return;

    pdf_obj *src_first = pdf_dict_get(ctx, src_outlines, PDF_NAME(First));
    if (!src_first) return;

    pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    if (!dst_root) return;

    pdf_obj *dst_outlines = pdf_dict_get(ctx, dst_root, PDF_NAME(Outlines));
    if (!dst_outlines) {
        /* Graft entire outline tree if destination has none */
        pdf_obj *grafted_outlines = pdf_graft_mapped_object(ctx, map, src_outlines);
        pdf_dict_put(ctx, dst_root, PDF_NAME(Outlines), grafted_outlines);
        return;
    }

    /* Graft top-level chain of source outline items and append after dst_outlines Last */
    pdf_obj *curr_src = src_first;
    pdf_obj *dst_last = pdf_dict_get(ctx, dst_outlines, PDF_NAME(Last));
    pdf_obj *prev_dst = dst_last;

    while (curr_src && pdf_is_dict(ctx, curr_src)) {
        pdf_obj *grafted_item = pdf_graft_mapped_object(ctx, map, curr_src);
        if (grafted_item) {
            pdf_dict_put(ctx, grafted_item, PDF_NAME(Parent), dst_outlines);
            if (prev_dst) {
                pdf_dict_put(ctx, prev_dst, PDF_NAME(Next), grafted_item);
                pdf_dict_put(ctx, grafted_item, PDF_NAME(Prev), prev_dst);
            } else {
                pdf_dict_put(ctx, dst_outlines, PDF_NAME(First), grafted_item);
            }
            prev_dst = grafted_item;
            pdf_dict_put(ctx, dst_outlines, PDF_NAME(Last), grafted_item);
        }
        curr_src = pdf_dict_get(ctx, curr_src, PDF_NAME(Next));
    }
}

/*
 * Grafts PageLabels from src_doc into dst_doc, shifting label start keys by dst_page_offset.
 */
static void append_grafted_pagelabels(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_document *src_doc,
    pdf_graft_map *map,
    int dst_page_offset
) {
    pdf_obj *src_root = pdf_dict_get(ctx, pdf_trailer(ctx, src_doc), PDF_NAME(Root));
    if (!src_root) return;
    pdf_obj *src_pagelabels = pdf_dict_get(ctx, src_root, PDF_NAME(PageLabels));
    if (!src_pagelabels) return;
    pdf_obj *src_nums = pdf_dict_get(ctx, src_pagelabels, PDF_NAME(Nums));
    if (!pdf_is_array(ctx, src_nums)) return;

    pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    if (!dst_root) return;
    pdf_obj *dst_pagelabels = pdf_dict_get(ctx, dst_root, PDF_NAME(PageLabels));

    if (!dst_pagelabels) {
        dst_pagelabels = pdf_new_dict(ctx, dst_doc, 2);
        pdf_obj *dst_nums = pdf_new_array(ctx, dst_doc, 4);
        pdf_dict_put_drop(ctx, dst_pagelabels, PDF_NAME(Nums), dst_nums);
        pdf_dict_put_drop(ctx, dst_root, PDF_NAME(PageLabels), dst_pagelabels);
    }

    pdf_obj *dst_nums = pdf_dict_get(ctx, dst_pagelabels, PDF_NAME(Nums));
    if (!pdf_is_array(ctx, dst_nums)) return;

    int src_len = pdf_array_len(ctx, src_nums);
    for (int i = 0; i < src_len; i += 2) {
        pdf_obj *key_obj = pdf_array_get(ctx, src_nums, i);
        pdf_obj *val_obj = pdf_array_get(ctx, src_nums, i + 1);
        if (key_obj && val_obj) {
            int orig_key = pdf_to_int(ctx, key_obj);
            int new_key = orig_key + dst_page_offset;
            pdf_obj *grafted_val = pdf_graft_mapped_object(ctx, map, val_obj);
            pdf_array_push_int(ctx, dst_nums, new_key);
            pdf_array_push(ctx, dst_nums, grafted_val);
        }
    }
}

/*
 * Shifts existing PageLabels in dst_doc with start keys >= insert_at by count_inserted.
 */
static void shift_existing_pagelabels(
    fz_context *ctx,
    pdf_document *dst_doc,
    int insert_at,
    int count_inserted
) {
    pdf_obj *root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    if (!root) return;
    pdf_obj *pagelabels = pdf_dict_get(ctx, root, PDF_NAME(PageLabels));
    if (!pagelabels) return;
    pdf_obj *nums = pdf_dict_get(ctx, pagelabels, PDF_NAME(Nums));
    if (!pdf_is_array(ctx, nums)) return;

    int len = pdf_array_len(ctx, nums);
    for (int i = 0; i < len; i += 2) {
        pdf_obj *key_obj = pdf_array_get(ctx, nums, i);
        int key = pdf_to_int(ctx, key_obj);
        if (key >= insert_at) {
            pdf_array_put_int(ctx, nums, i, (int64_t)(key + count_inserted));
        }
    }
}

/*
 * Merges form fields (/AcroForm) from grafted pages, ensuring name uniqueness
 * against existing destination fields and updating the /AcroForm /Fields catalog.
 */
static void merge_acroform_fields_for_grafted_page(
    fz_context *ctx,
    pdf_document *dst_doc,
    pdf_obj *dst_page_ref,
    const char *collision_suffix
) {
    pdf_obj *annots = pdf_dict_get(ctx, dst_page_ref, PDF_NAME(Annots));
    if (!pdf_is_array(ctx, annots)) return;

    pdf_obj *dst_root = pdf_dict_get(ctx, pdf_trailer(ctx, dst_doc), PDF_NAME(Root));
    if (!dst_root) return;

    pdf_obj *acroform = pdf_dict_get(ctx, dst_root, PDF_NAME(AcroForm));
    if (!acroform) {
        acroform = pdf_new_dict(ctx, dst_doc, 2);
        pdf_obj *fields = pdf_new_array(ctx, dst_doc, 8);
        pdf_dict_put_drop(ctx, acroform, PDF_NAME(Fields), fields);
        pdf_dict_put_drop(ctx, dst_root, PDF_NAME(AcroForm), acroform);
    }

    pdf_obj *fields = pdf_dict_get(ctx, acroform, PDF_NAME(Fields));
    if (!pdf_is_array(ctx, fields)) return;

    int n_annots = pdf_array_len(ctx, annots);
    for (int a = 0; a < n_annots; a++) {
        pdf_obj *annot = pdf_array_get(ctx, annots, a);
        if (!pdf_is_dict(ctx, annot)) continue;

        pdf_obj *subtype = pdf_dict_get(ctx, annot, PDF_NAME(Subtype));
        if (!pdf_name_eq(ctx, subtype, PDF_NAME(Widget))) continue;

        pdf_obj *t_obj = pdf_dict_get(ctx, annot, PDF_NAME(T));
        if (t_obj) {
            const char *orig_name = pdf_to_text_string(ctx, t_obj);
            if (orig_name && field_name_exists_in_fields(ctx, fields, orig_name, 0)) {
                char new_name[256];
                char base_name[200];
                snprintf(base_name, sizeof(base_name), "%s_%s", orig_name, collision_suffix ? collision_suffix : "Merge");
                generate_unique_field_name(ctx, fields, base_name, new_name, sizeof(new_name));
                pdf_dict_put_text_string(ctx, annot, PDF_NAME(T), new_name);
            }
        }

        /* Add to destination /AcroForm /Fields if not already present */
        int already_present = 0;
        int n_fields = pdf_array_len(ctx, fields);
        for (int f = 0; f < n_fields; f++) {
            if (is_same_object(ctx, annot, pdf_array_get(ctx, fields, f))) {
                already_present = 1;
                break;
            }
        }
        if (!already_present) {
            pdf_array_push(ctx, fields, annot);
        }
    }
}

/*
 * Merges multiple PDF documents into a single PDF document (`M3.2`).
 *
 * Status:
 *   0: success
 *   1: invalid arguments or corrupted document structure
 *   2: cannot open or parse one of the PDFs
 *   3: out of memory
 *   4: operation blocked by digital signatures (fail-closed preflight)
 */
int vocument_mupdf_merge_documents_ext(
    const unsigned char * const *documents,
    const size_t *document_lens,
    size_t document_count,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!documents || !document_lens || document_count == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    int status = 0;
    fz_stream *dst_stream = NULL;
    fz_document *dst_fz_doc = NULL;
    pdf_document *dst_pdf_doc = NULL;

    fz_var(dst_stream);
    fz_var(dst_fz_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);

        /* Open primary document (documents[0]) */
        dst_stream = fz_open_memory(ctx, documents[0], document_lens[0]);
        dst_fz_doc = fz_open_document_with_stream(ctx, "application/pdf", dst_stream);
        dst_pdf_doc = pdf_specifics(ctx, dst_fz_doc);
        if (!dst_pdf_doc) {
            status = 2;
        } else {
            /* Check signatures on primary document */
            int sigs0 = count_active_signatures(ctx, dst_pdf_doc);
            if (sigs0 != 0 && !allow_signature_invalidation) {
                status = 4;
            } else if (sigs0 < 0 || !validate_tagged_pdf_consistency(ctx, dst_pdf_doc)) {
                status = 1;
            }
        }

        if (status == 0) {
            int had_signatures = (count_active_signatures(ctx, dst_pdf_doc) > 0);

            /* Graft pages from all subsequent documents */
            for (size_t d = 1; d < document_count && status == 0; d++) {
                fz_stream *src_stream = NULL;
                fz_document *src_fz_doc = NULL;
                pdf_document *src_pdf_doc = NULL;

                fz_var(src_stream);
                fz_var(src_fz_doc);

                fz_try(ctx) {
                    src_stream = fz_open_memory(ctx, documents[d], document_lens[d]);
                    src_fz_doc = fz_open_document_with_stream(ctx, "application/pdf", src_stream);
                    src_pdf_doc = pdf_specifics(ctx, src_fz_doc);

                    if (!src_pdf_doc) {
                        status = 2;
                    } else {
                        int src_sigs = count_active_signatures(ctx, src_pdf_doc);
                        if (src_sigs != 0 && !allow_signature_invalidation) {
                            status = 4;
                        } else if (src_sigs < 0 || !validate_tagged_pdf_consistency(ctx, src_pdf_doc)) {
                            status = 1;
                        } else {
                            if (src_sigs > 0) had_signatures = 1;

                            int dst_count_before = pdf_count_pages(ctx, dst_pdf_doc);
                            int src_count = pdf_count_pages(ctx, src_pdf_doc);
                            pdf_graft_map *map = pdf_new_graft_map(ctx, dst_pdf_doc);

                            fz_try(ctx) {
                                char suffix[32];
                                snprintf(suffix, sizeof(suffix), "Doc%zu", d + 1);

                                for (int p = 0; p < src_count; p++) {
                                    pdf_graft_mapped_page(ctx, map, -1, src_pdf_doc, p);
                                    int newly_added_idx = dst_count_before + p;
                                    pdf_obj *grafted_page = pdf_lookup_page_obj(ctx, dst_pdf_doc, newly_added_idx);
                                    if (grafted_page) {
                                        make_page_self_contained(ctx, grafted_page);
                                        merge_acroform_fields_for_grafted_page(ctx, dst_pdf_doc, grafted_page, suffix);
                                    }
                                }

                                for (int p = 0; p < src_count; p++) {
                                    int newly_added_idx = dst_count_before + p;
                                    pdf_obj *grafted_page = pdf_lookup_page_obj(ctx, dst_pdf_doc, newly_added_idx);
                                    if (grafted_page) {
                                        sanitize_links_for_grafted_page(ctx, dst_pdf_doc, src_pdf_doc, grafted_page, dst_count_before, NULL, 0);
                                    }
                                }

                                append_grafted_outlines(ctx, dst_pdf_doc, src_pdf_doc, map);
                                append_grafted_pagelabels(ctx, dst_pdf_doc, src_pdf_doc, map, dst_count_before);
                                append_grafted_named_dests(ctx, dst_pdf_doc, src_pdf_doc, map, dst_count_before, suffix);

                                if (vocument_merge_tagged_structures(ctx, dst_pdf_doc, src_pdf_doc, map, dst_count_before, src_count) != 0) {
                                    status = 5;
                                }
                            }
                            fz_always(ctx) {
                                pdf_drop_graft_map(ctx, map);
                            }
                            fz_catch(ctx) {
                                fz_rethrow(ctx);
                            }
                        }
                    }
                }
                fz_always(ctx) {
                    if (src_fz_doc) fz_drop_document(ctx, src_fz_doc);
                    if (src_stream) fz_drop_stream(ctx, src_stream);
                }
                fz_catch(ctx) {
                    status = 2;
                }
            }

            if (status == 0) {
                if (!validate_tagged_pdf_consistency(ctx, dst_pdf_doc)) {
                    status = 5;
                }
            }

            if (status == 0) {
                if (had_signatures && allow_signature_invalidation) {
                    mark_signatures_invalidated(ctx, dst_pdf_doc);
                }
                vocument_serialize_pdf_doc_to_malloc(ctx, dst_pdf_doc, out_pdf_bytes, out_pdf_len);
            }
        }
    }
    fz_always(ctx) {
        if (dst_fz_doc) fz_drop_document(ctx, dst_fz_doc);
        if (dst_stream) fz_drop_stream(ctx, dst_stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}

/*
 * Inserts pages from a source PDF document into a target PDF document at insert_at (0-based) (`M3.2`).
 *
 * Status:
 *   0: success
 *   1: invalid arguments, indices out of bounds, or corrupted Tagged PDF
 *   2: cannot open or parse one of the PDFs
 *   3: out of memory
 *   4: operation blocked by digital signatures (fail-closed preflight)
 */
int vocument_mupdf_insert_pages_ext(
    const unsigned char *target_bytes,
    size_t target_len,
    const unsigned char *source_bytes,
    size_t source_len,
    const int *source_page_indices,
    size_t source_page_indices_len,
    int insert_at,
    int allow_signature_invalidation,
    unsigned char **out_pdf_bytes,
    size_t *out_pdf_len
) {
    if (!target_bytes || target_len == 0 || !source_bytes || source_len == 0 ||
        !source_page_indices || source_page_indices_len == 0 || !out_pdf_bytes || !out_pdf_len) {
        return 1;
    }
    *out_pdf_bytes = NULL;
    *out_pdf_len = 0;

    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_DEFAULT);
    if (!ctx) return 3;

    int status = 0;
    fz_stream *tgt_stream = NULL;
    fz_document *tgt_fz_doc = NULL;
    pdf_document *tgt_pdf_doc = NULL;

    fz_stream *src_stream = NULL;
    fz_document *src_fz_doc = NULL;
    pdf_document *src_pdf_doc = NULL;

    fz_var(tgt_stream);
    fz_var(tgt_fz_doc);
    fz_var(src_stream);
    fz_var(src_fz_doc);
    fz_var(status);

    fz_try(ctx) {
        fz_register_document_handlers(ctx);

        tgt_stream = fz_open_memory(ctx, target_bytes, target_len);
        tgt_fz_doc = fz_open_document_with_stream(ctx, "application/pdf", tgt_stream);
        tgt_pdf_doc = pdf_specifics(ctx, tgt_fz_doc);

        src_stream = fz_open_memory(ctx, source_bytes, source_len);
        src_fz_doc = fz_open_document_with_stream(ctx, "application/pdf", src_stream);
        src_pdf_doc = pdf_specifics(ctx, src_fz_doc);

        if (!tgt_pdf_doc || !src_pdf_doc) {
            status = 2;
        } else {
            int tgt_count = pdf_count_pages(ctx, tgt_pdf_doc);
            int src_count = pdf_count_pages(ctx, src_pdf_doc);

            if (insert_at < 0 || insert_at > tgt_count) {
                status = 1;
            } else {
                for (size_t i = 0; i < source_page_indices_len; i++) {
                    int p = source_page_indices[i];
                    if (p < 0 || p >= src_count) {
                        status = 1;
                        break;
                    }
                }
            }

            if (status == 0) {
                int tgt_sigs = count_active_signatures(ctx, tgt_pdf_doc);
                int src_sigs = count_active_signatures(ctx, src_pdf_doc);

                if ((tgt_sigs != 0 || src_sigs != 0) && !allow_signature_invalidation) {
                    status = 4;
                } else if (tgt_sigs < 0 || src_sigs < 0 ||
                           !validate_tagged_pdf_consistency(ctx, tgt_pdf_doc) ||
                           !validate_tagged_pdf_consistency(ctx, src_pdf_doc)) {
                    status = 1;
                } else {
                    int had_sigs = (tgt_sigs > 0 || src_sigs > 0);
                    pdf_graft_map *map = pdf_new_graft_map(ctx, tgt_pdf_doc);

                    fz_try(ctx) {
                        shift_existing_pagelabels(ctx, tgt_pdf_doc, insert_at, (int)source_page_indices_len);

                        for (size_t i = 0; i < source_page_indices_len; i++) {
                            int src_idx = source_page_indices[i];
                            int pos_in_tgt = insert_at + (int)i;
                            pdf_graft_mapped_page(ctx, map, pos_in_tgt, src_pdf_doc, src_idx);

                            pdf_obj *grafted_page = pdf_lookup_page_obj(ctx, tgt_pdf_doc, pos_in_tgt);
                            if (grafted_page) {
                                make_page_self_contained(ctx, grafted_page);
                                merge_acroform_fields_for_grafted_page(ctx, tgt_pdf_doc, grafted_page, "Insert");
                            }
                        }

                        for (size_t i = 0; i < source_page_indices_len; i++) {
                            int pos_in_tgt = insert_at + (int)i;
                            pdf_obj *grafted_page = pdf_lookup_page_obj(ctx, tgt_pdf_doc, pos_in_tgt);
                            if (grafted_page) {
                                sanitize_links_for_grafted_page(ctx, tgt_pdf_doc, src_pdf_doc, grafted_page, insert_at, source_page_indices, source_page_indices_len);
                            }
                        }

                        append_grafted_named_dests(ctx, tgt_pdf_doc, src_pdf_doc, map, insert_at, "Insert");

                        if (vocument_merge_tagged_structures(ctx, tgt_pdf_doc, src_pdf_doc, map, insert_at, (int)source_page_indices_len) != 0) {
                            status = 5;
                        }
                    }
                    fz_always(ctx) {
                        pdf_drop_graft_map(ctx, map);
                    }
                    fz_catch(ctx) {
                        fz_rethrow(ctx);
                    }

                    if (status == 0) {
                        if (!validate_tagged_pdf_consistency(ctx, tgt_pdf_doc)) {
                            status = 5;
                        }
                    }

                    if (status == 0) {
                        if (had_sigs && allow_signature_invalidation) {
                            mark_signatures_invalidated(ctx, tgt_pdf_doc);
                        }
                        vocument_serialize_pdf_doc_to_malloc(ctx, tgt_pdf_doc, out_pdf_bytes, out_pdf_len);
                    }
                }
            }
        }
    }
    fz_always(ctx) {
        if (src_fz_doc) fz_drop_document(ctx, src_fz_doc);
        if (src_stream) fz_drop_stream(ctx, src_stream);
        if (tgt_fz_doc) fz_drop_document(ctx, tgt_fz_doc);
        if (tgt_stream) fz_drop_stream(ctx, tgt_stream);
    }
    fz_catch(ctx) {
        status = 2;
    }

    fz_drop_context(ctx);
    return status;
}
