//! End-to-end integration tests for UniFFI native editor bridge (`ADR-0020`, `UI-009`, `PERF-001`).

use vocument_ffi::*;

#[test]
fn test_ffi_editor_session_full_lifecycle_and_pdoc_save() {
    // 1. Create new editor session
    let session = document_core_create_editor_session("Court Brief".to_string())
        .expect("create editor session");
    assert_eq!(session.title, "Court Brief");
    assert_eq!(session.total_pages, 1);
    assert!(!session.pages.is_empty());

    let editor_id = session.editor_id;

    // 2. Type text (§ 242 BGB)
    let s2 = document_core_editor_insert_text(
        editor_id.clone(),
        "Gemäß § 242 BGB ist der Schuldner verpflichtet, die Leistung nach Treu und Glauben zu bewirken."
            .to_string(),
    )
    .expect("type legal text");
    assert!(s2.plain_text.contains("§ 242 BGB"));
    assert!(s2.can_undo);
    assert!(!s2.can_redo);

    // 3. Formatting: Toggle bold and type clause
    let _ = document_core_editor_toggle_bold(editor_id.clone()).expect("toggle bold");
    let s3 =
        document_core_editor_insert_text(editor_id.clone(), " Dies gilt ausnahmslos.".to_string())
            .expect("type bold clause");
    assert!(s3.plain_text.contains("Dies gilt ausnahmslos"));

    // 4. Undo and redo
    let s_undo = document_core_editor_undo(editor_id.clone()).expect("undo");
    assert!(!s_undo.plain_text.contains("Dies gilt ausnahmslos"));
    assert!(s_undo.can_redo);

    let s_redo = document_core_editor_redo(editor_id.clone()).expect("redo");
    assert!(s_redo.plain_text.contains("Dies gilt ausnahmslos"));

    // 5. Paragraph break (Enter key)
    let s_break =
        document_core_editor_insert_paragraph_break(editor_id.clone()).expect("insert break");
    assert!(s_break.plain_text.contains("\n"));

    // 6. Heading level 1
    let _ = document_core_editor_insert_text(editor_id.clone(), "II. Sachverhalt".to_string())
        .expect("type heading");
    let s_h1 = document_core_editor_set_heading_level(editor_id.clone(), 1).expect("set H1");
    assert!(s_h1.plain_text.contains("II. Sachverhalt"));

    // 7. Insert bullet list & numbered list
    let s_list = document_core_editor_insert_bullet_list(editor_id.clone()).expect("bullet list");
    assert!(!s_list.plain_text.is_empty());

    let s_num = document_core_editor_insert_numbered_list(editor_id.clone()).expect("num list");
    assert!(!s_num.plain_text.is_empty());

    // 8. Insert table & footnote
    let s_table = document_core_editor_insert_table(editor_id.clone(), 3, 2).expect("insert table");
    assert!(s_table.total_pages >= 1);

    let s_fn =
        document_core_editor_insert_footnote(editor_id.clone(), "BGH, NJW 2021, 1234.".to_string())
            .expect("insert footnote");
    assert!(s_fn.total_pages >= 1);

    // 9. Coordinate hit-testing (simulate click at 100 pt, 100 pt)
    let s_hit =
        document_core_editor_hit_test(editor_id.clone(), 0, 100.0, 100.0).expect("hit test");
    assert!(s_hit.cursor_rect.is_some());

    // 10. Performance budget: typing latency < 16,000 µs (16 ms)
    let latency_micros = document_core_editor_measure_typing_latency_micros(
        editor_id.clone(),
        "Prüfung der materiellen Rechtslage und Beweiswürdigung.".to_string(),
    )
    .expect("measure latency");
    assert!(
        latency_micros < 16_000,
        "Typing latency {latency_micros} µs exceeds 16ms budget"
    );

    // 11. Save as hardened `.pdoc` file
    let tmp_dir = std::env::temp_dir().join(format!("vocument_test_editor_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let pdoc_path = tmp_dir.join("brief_session.pdoc");
    let saved_path = document_core_editor_save_pdoc(
        editor_id.clone(),
        Some(pdoc_path.to_string_lossy().to_string()),
    )
    .expect("save pdoc");
    assert_eq!(saved_path, pdoc_path.to_string_lossy().to_string());
    assert!(pdoc_path.exists());

    // 12. Close and re-open saved `.pdoc` session
    document_core_close_editor_session(editor_id).expect("close session");

    let reopened = document_core_open_pdoc_editor_session(pdoc_path.to_string_lossy().to_string())
        .expect("reopen pdoc");
    assert_eq!(reopened.title, "Court Brief");
    assert!(reopened.plain_text.contains("§ 242 BGB"));
    assert!(reopened.plain_text.contains("II. Sachverhalt"));

    document_core_close_editor_session(reopened.editor_id).expect("close reopened session");
    let _ = std::fs::remove_dir_all(&tmp_dir);
}
