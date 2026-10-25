//! Tests for native platform FlowDocument editor engine (`ADR-0020`, `AUTHOR-002`, `UI-009`).

use vocument_layout::editor::FlowEditorState;

#[test]
fn test_editor_typing_and_cursor_movement() {
    let mut editor = FlowEditorState::new_empty("Briefing Document");

    // Initial state: empty document, cursor at 0
    assert_eq!(editor.cursor_offset, 0);

    // Type regular text
    editor.insert_text("In accordance with § 242 BGB, ");
    assert_eq!(editor.cursor_offset, 30);

    // Type German and Unicode characters
    editor.insert_text("Treu und Glauben müssen gewahrt bleiben.");
    assert!(editor.cursor_offset > 30);

    let cursor_pos = editor.cursor_rect().expect("cursor rect must exist");
    assert_eq!(cursor_pos.0, 0); // Page index 0
    assert!(cursor_pos.1.x > 0.0);
    assert!(cursor_pos.1.y > 0.0);
    assert!(cursor_pos.1.height > 0.0);

    // Test backspace
    let prev_offset = editor.cursor_offset;
    editor.delete_backward();
    assert_eq!(editor.cursor_offset, prev_offset - 1);
}

#[test]
fn test_editor_cjk_and_unicode_input() {
    let mut editor = FlowEditorState::new_empty("Multilingual Brief");

    // Type Japanese CJK characters (simulating IME input)
    editor.insert_text("契約書第1条：秘密保持義務。");
    assert!(editor.cursor_offset > 0);

    let cursor_pos = editor.cursor_rect().expect("cursor rect for CJK");
    assert!(cursor_pos.1.x > 0.0);

    // Delete character
    let offset_before = editor.cursor_offset;
    editor.delete_backward();
    assert_eq!(editor.cursor_offset, offset_before - 1);
}

fn block_text(block: &vocument_flow::model::Block) -> String {
    match block {
        vocument_flow::model::Block::Paragraph(p) => {
            p.spans.iter().map(|s| s.text.as_str()).collect()
        }
        vocument_flow::model::Block::Heading(h) => {
            h.spans.iter().map(|s| s.text.as_str()).collect()
        }
        _ => String::new(),
    }
}

#[test]
fn test_editor_undo_redo_transactions() {
    let mut editor = FlowEditorState::new_empty("Undo Test");
    assert!(!editor.can_undo());
    assert!(!editor.can_redo());

    editor.insert_text("Initial clause.");
    assert!(editor.can_undo());

    editor.insert_text(" Second clause.");
    let text_before_undo = block_text(&editor.document.sections[0].blocks[0]);
    assert!(text_before_undo.contains("Second clause"));

    // Undo second typing
    assert!(editor.undo());
    let text_after_first_undo = block_text(&editor.document.sections[0].blocks[0]);
    assert!(!text_after_first_undo.contains("Second clause"));
    assert!(text_after_first_undo.contains("Initial clause"));
    assert!(editor.can_redo());

    // Redo second typing
    assert!(editor.redo());
    let text_after_redo = block_text(&editor.document.sections[0].blocks[0]);
    assert!(text_after_redo.contains("Second clause"));
}

#[test]
fn test_editor_paragraph_break_split() {
    let mut editor = FlowEditorState::new_empty("Split Test");
    editor.insert_text("First sentence. Second sentence.");

    // Move cursor between sentences (offset 16)
    editor.cursor_offset = 16;
    editor.insert_paragraph_break();

    // Must now have 2 paragraph blocks
    assert_eq!(editor.document.sections[0].blocks.len(), 2);
    let p1 = block_text(&editor.document.sections[0].blocks[0]);
    let p2 = block_text(&editor.document.sections[0].blocks[1]);

    assert_eq!(p1.trim(), "First sentence.");
    assert_eq!(p2.trim(), "Second sentence.");
    assert_eq!(editor.cursor_offset, 0);
}

#[test]
fn test_editor_formatting_and_headings() {
    let mut editor = FlowEditorState::new_empty("Format Test");
    editor.insert_text("Legal Heading");

    // Convert paragraph to Heading Level 1
    editor.set_heading_level(1);
    match &editor.document.sections[0].blocks[0] {
        vocument_flow::model::Block::Heading(h) => {
            assert_eq!(h.level, 1);
            let text: String = h.spans.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(text, "Legal Heading");
        }
        other => panic!("Expected HeadingBlock, got: {other:?}"),
    }

    // Toggle Bold
    editor.toggle_bold();

    // Convert back to paragraph
    editor.set_heading_level(0);
    match &editor.document.sections[0].blocks[0] {
        vocument_flow::model::Block::Paragraph(p) => {
            let text: String = p.spans.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(text, "Legal Heading");
        }
        other => panic!("Expected ParagraphBlock, got: {other:?}"),
    }
}

#[test]
fn test_editor_lists_tables_and_footnotes() {
    let mut editor = FlowEditorState::new_empty("Complex Structs");

    // Insert bullet list
    editor.insert_bullet_list();
    assert!(
        editor.document.sections[0]
            .blocks
            .iter()
            .any(|b| matches!(b, vocument_flow::model::Block::List(_)))
    );

    // Insert numbered list
    editor.insert_numbered_list();

    // Insert 3x2 Table
    editor.insert_table(3, 2);
    let has_table = editor.document.sections[0].blocks.iter().any(|b| {
        if let vocument_flow::model::Block::Table(tbl) = b {
            tbl.rows.len() == 3 && tbl.rows[0].cells.len() == 2
        } else {
            false
        }
    });
    assert!(has_table);

    // Insert Footnote
    editor.insert_footnote("See Federal Supreme Court ruling BGHZ 123, 45.");
    assert_eq!(editor.document.footnotes.len(), 1);
}

#[test]
fn test_editor_hit_test_and_selection() {
    let mut editor = FlowEditorState::new_empty("Selection Test");
    editor.insert_text("Clickable and selectable legal text run.");

    // Hit test at point within page content
    let hit = editor.hit_test_and_set_cursor(0, 100.0, 85.0);
    assert!(hit.is_some());

    // Selection range: select characters 0 to 15
    let block_id = editor.active_block_id.clone();
    editor.set_selection(&block_id, 0, &block_id, 15);
    let sel_rects = editor.selection_rects();
    assert!(!sel_rects.is_empty());
    assert!(sel_rects[0].1.width > 0.0);
}

#[test]
fn test_editor_sub_16ms_typing_latency() {
    let mut editor = FlowEditorState::new_empty("Performance Test");
    // Pre-populate with 1,000 words
    let mut text = String::new();
    for i in 0..100 {
        text.push_str(&format!(
            "Clause {} stipulates standard confidentiality. ",
            i
        ));
    }
    editor.insert_text(&text);

    // Measure typing latency for 50 keystrokes
    let avg_per_char_micros =
        editor.measure_typing_latency_micros("Quick legal addendum with § symbols and notes.");

    // Sub-16ms budget per keystroke: 16,000 microseconds
    assert!(
        avg_per_char_micros < 16_000,
        "Average typing latency {} µs exceeds 16ms budget!",
        avg_per_char_micros
    );
}

#[test]
fn test_editor_undo_coalescing_efficiency() {
    let mut editor = FlowEditorState::new_empty("Undo Efficiency");

    // Type "Hello " char by char
    for c in "Hello ".chars() {
        editor.insert_text(&c.to_string());
    }
    // Type "World" char by char
    for c in "World".chars() {
        editor.insert_text(&c.to_string());
    }

    let full_text = block_text(&editor.document.sections[0].blocks[0]);
    assert_eq!(full_text, "Hello World");

    // First undo should undo the "World" word batch, leaving "Hello "
    assert!(editor.undo());
    let text_after_undo_1 = block_text(&editor.document.sections[0].blocks[0]);
    assert_eq!(text_after_undo_1, "Hello ");

    // Second undo should undo the space " ", leaving "Hello"
    assert!(editor.undo());
    let text_after_undo_2 = block_text(&editor.document.sections[0].blocks[0]);
    assert_eq!(text_after_undo_2, "Hello");

    // Third undo should undo the "Hello" word batch, leaving ""
    assert!(editor.undo());
    let text_after_undo_3 = block_text(&editor.document.sections[0].blocks[0]);
    assert_eq!(text_after_undo_3, "");

    // Redo all three steps
    assert!(editor.redo());
    assert_eq!(block_text(&editor.document.sections[0].blocks[0]), "Hello");
    assert!(editor.redo());
    assert_eq!(block_text(&editor.document.sections[0].blocks[0]), "Hello ");
    assert!(editor.redo());
    assert_eq!(
        block_text(&editor.document.sections[0].blocks[0]),
        "Hello World"
    );
}

#[test]
fn test_glyph_based_cursor_and_selection() {
    let mut editor = FlowEditorState::new_empty("Glyph Metrics Test");

    // Type text with very narrow and very wide characters
    editor.insert_text("iii WWW");

    let block_id = editor.active_block_id.clone();

    // Offset 0: start of line
    editor.set_cursor(&block_id, 0);
    let r0 = editor.cursor_rect().expect("cursor rect present").1;

    // Offset 1: after 'i'
    editor.set_cursor(&block_id, 1);
    let r1 = editor.cursor_rect().expect("cursor rect present").1;
    let adv_i = r1.x - r0.x;

    // Offset 5: after first 'W' (offset 4 is space)
    editor.set_cursor(&block_id, 4);
    let r4 = editor.cursor_rect().expect("cursor rect present").1;
    editor.set_cursor(&block_id, 5);
    let r5 = editor.cursor_rect().expect("cursor rect present").1;
    let adv_w = r5.x - r4.x;

    // 'W' must have a significantly larger glyph advance than 'i' in variable-width font
    assert!(
        adv_w > adv_i * 1.5,
        "Glyph advance of W ({:.2}) should exceed i ({:.2})",
        adv_w,
        adv_i
    );

    // Test BiDi RTL detection and layout
    let mut rtl_editor = FlowEditorState::new_empty("RTL BiDi Test");
    rtl_editor.insert_text("سلام دنیا"); // Arabic RTL text
    let rtl_cursor = rtl_editor.cursor_rect().expect("cursor rect present").1;
    assert!(rtl_cursor.x > 0.0);
}

#[test]
fn test_tracked_changes_insert_delete_and_review() {
    let mut editor = FlowEditorState::new_empty("Reviewing Document");
    editor.insert_text("Initial agreement terms.");

    // Enable tracking under "Counsel Bob"
    editor.enable_tracking("Counsel Bob");
    assert!(editor.is_tracking());

    // Insert tracked text
    editor.insert_text(" Valid until 2030.");
    assert_eq!(editor.document.review.tracked_changes.len(), 1);
    let change = &editor.document.review.tracked_changes[0];
    assert_eq!(change.author, "Counsel Bob");
    assert_eq!(
        change.change_type,
        vocument_flow::review::ChangeType::Insert
    );
    assert_eq!(change.text_snippet.as_deref(), Some(" Valid until 2030."));

    // Add Margin Comment with anchored range
    let block_id = editor.active_block_id.clone();
    let comment_id = editor.add_comment(Some((0, 7)), "Please confirm term length.");
    assert_eq!(editor.document.review.comments.len(), 1);
    assert_eq!(editor.document.review.comments[0].comment_id, comment_id);
    assert_eq!(
        editor.document.review.comments[0].anchor_text.as_deref(),
        Some("Initial")
    );

    // Add Reply
    let reply_id = editor.reply_to_comment(&comment_id, "Confirmed by client.");
    assert!(reply_id.is_some());
    assert_eq!(editor.document.review.comments[0].replies.len(), 1);

    // Resolve comment
    assert!(editor.resolve_comment(&comment_id, true));
    assert!(editor.document.review.comments[0].resolved);

    // Resilient anchor testing: insert text at start of block (offset 0)
    editor.disable_tracking();
    editor.set_cursor(&block_id, 0);
    editor.insert_text("Pre-clause: ");
    // Comment anchor was at (0, 7), should now be shifted by len("Pre-clause: ") = 12
    let updated_comment = &editor.document.review.comments[0];
    assert_eq!(updated_comment.anchor_char_start, Some(12));
    assert_eq!(updated_comment.anchor_char_end, Some(19));

    // Accept change
    let change_id = editor.document.review.tracked_changes[0].change_id.clone();
    assert!(editor.accept_change(&change_id));
    assert_eq!(editor.document.review.tracked_changes.len(), 0);
}

#[test]
fn test_accept_and_reject_all_tracked_changes() {
    let mut editor = FlowEditorState::new_empty("Bulk Review");
    editor.insert_text("Original text.");

    editor.enable_tracking("Reviewer 1");
    editor.insert_text(" Addendum A.");
    editor.insert_text(" Addendum B.");
    assert_eq!(editor.document.review.tracked_changes.len(), 2);

    // Reject all
    editor.reject_all_changes();
    assert_eq!(editor.document.review.tracked_changes.len(), 0);
    let doc_text = block_text(&editor.document.sections[0].blocks[0]);
    assert_eq!(doc_text, "Original text.");
}
