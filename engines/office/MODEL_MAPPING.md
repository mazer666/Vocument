# Office ↔ FlowDocument mapping conventions (M4.5)

Binding for **both** adapters (DOCX and ODT). A convention that differs between the formats
makes a DOCX→ODT conversion lose information, so it is written down once.

## Principles

1. **Never silently discard** (`OFFICE-004`). Every property that is not mapped produces an
   entry in the `ConversionReport` (`Loss`, `Approximated`, `PreservedUnrendered`, `Security`).
2. **Prefer preservation over rendering.** Where the model has no field, keep the information in
   the `extra` map of the nearest model object under a key prefixed `office.` so that a later
   export of the *same format* can write it back; report it as `PreservedUnrendered`.
3. **No panics, no `unwrap`.** Hostile input must produce an `OfficeError` or a report entry.
4. **Deterministic output.** Same document, same bytes (no clocks, no random identifiers, sorted
   maps). Export must not call `SystemTime`.
5. **Untrusted input.** Use `xml::parse` and `OfficePackage::from_bytes`; never resolve external
   references; links go through `util::safe_hyperlink`; external images/OLE objects are never
   fetched (report `Security`).

## Identifiers

Generated with `util::IdGenerator`: `sec_N`, `p_N`, `h_N`, `list_N`, `li_N`, `tbl_N`, `cell_N`,
`img_N`, `fn_N` (footnote key), `comment_N`, `reply_N`, `chg_N`. All ids are unique in the
document (`validate_document` enforces it).

## Mapping table

| Office concept | FlowDocument |
|---|---|
| Paragraph style (`w:style type=paragraph` / `style:style family=paragraph`) | `Stylesheet.paragraph_styles[id]`, `id` = Word `styleId` / ODF `style:name`, `name` = display name. Size, font, spacing, alignment, indent, keep-with-next resolved through the inheritance chain. |
| Heading (style with outline level 1–6, or name `Heading N`) | `Block::Heading { level, style_id }`. Levels above 6 become a paragraph + `Approximated`. |
| Plain paragraph | `Block::Paragraph { style_id, alignment?, space_before_pt?, space_after_pt? }` — only direct overrides, the rest stays in the style. |
| Direct character formatting | `TextSpan { style: FontStyle{bold,italic,underline,strikethrough,subscript,superscript}, font_size_pt, font_family, color, background_color }`. Character styles are resolved into direct formatting. |
| Tab / line break | `'\t'` / `'\n'` inside `TextSpan.text`. Soft hyphen `U+00AD`, non-breaking hyphen `U+2011`. |
| Page break before a paragraph | `ParagraphBlock.extra["office.page_break_before"] = true` (`PreservedUnrendered`). |
| Lists (`w:numPr` / `text:list`) | `Block::List`; consecutive items of one list → one `ListBlock`; deeper level → `ListItem.nested_list`. Bullet formats → `ListKind::Unordered`, numbered formats → `Ordered`. Number format string kept in `ListBlock.extra["office.number_format"]`. |
| Table | `Block::Table`; `TableRow.is_header` from `w:tblHeader` / `table:table-header-rows`; `col_span` / `row_span` from `w:gridSpan`+`w:vMerge` / `table:number-columns-spanned`+`table:number-rows-spanned`; `column_widths_pt` from the grid; `background_color` from cell shading. |
| Inline image | `Block::Image { asset_ref, alt_text, display_*_pt }`. The model has block images only: an inline image inside a text paragraph is emitted as its own block after the paragraph (`Approximated`, code `*.image.inline-moved`). Binary data goes to `ImportedDocument.assets` under `util::content_addressed_path`. Linked (external) images are **not** loaded (`Security`). |
| Footnote / endnote | `TextSpan { text: "", footnote_ref: Some(id) }` + `FlowDocument.footnotes[id]` (`number` = order of appearance). Endnotes are mapped to footnotes (`Approximated`). |
| Header / footer | `Section.header_blocks` / `footer_blocks` — **default** variant only; first-page / even-page variants are reported as `Loss`. Page-number fields become the text `{page}` and `{total_pages}`, which `vocument-layout` resolves; export converts them back to fields. |
| Page geometry | `Section.geometry` (size, margins, orientation, columns). One `Section` per Word section / ODF page-layout change. |
| Hyperlink | `TextSpan.link_url` (only if `safe_hyperlink` accepts it, else the text stays and a `Security` entry is written). |
| Other fields (TOC, date, cross-reference …) | The cached result text is kept (`Approximated`, code `*.field.<kind>`). |
| Tracked insertion | `TextSpan { tracked_change_id: Some(id), style.underline: true }` + `TrackedChange { change_type: Insert }`. |
| Tracked deletion | `TextSpan { tracked_change_id: Some(id), style.strikethrough: true }` (the deleted text is kept) + `TrackedChange { change_type: Delete }`. |
| Tracked format change | `TextSpan { tracked_change_id: Some(id), original_style: Some(previous) }` + `TrackedChange { change_type: FormatChange }`. |
| Comment | `FlowReview.comments`: `FlowComment { anchor_block_id, anchor_char_start/end, anchor_text, resolved, replies }`. Offsets count Unicode scalar values over the concatenated `text` of the block's spans (footnote-reference spans count 0). Timestamps are kept as the ISO-8601 string of the source. |
| Document properties | `DocumentMetadata`: title, subtitle, authors, description, keywords, language, `created_at_utc`, `modified_at_utc`. Unknown custom properties → `custom_attributes`. |
| Bookmarks, comments ranges, proofing marks, rsids | Dropped **without** report only when they carry no user content (bookmarks used as hyperlink targets are kept as `#name` anchors where possible). |
| Text boxes, shapes, charts, SmartArt, OLE objects, equations, ActiveX, form controls | Text content is kept as plain paragraphs where it exists (`Approximated`); the object itself is reported as `Loss` with a code such as `docx.drawing.chart`. |
| Macros (`vbaProject.bin`, `Basic/`, scripts) | Never read or executed. `Security` entry. |

## Page-break and unknown-property policy

An unmapped element *with text inside* keeps its text. An unmapped element *without* text is
reported by its stable code and counted; the report is the user's proof that nothing vanished
unnoticed.

