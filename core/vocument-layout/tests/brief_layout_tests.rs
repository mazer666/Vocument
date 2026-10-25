use vocument_flow::metadata::{Author, DocumentMetadata};
use vocument_flow::model::{
    Block, CalloutBlock, CalloutType, FlowDocument, FootnoteDefinition, HeadingBlock, ListBlock,
    ListItem, ListKind, ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow,
    ThematicBreakBlock,
};
use vocument_flow::styles::{ColorRgba, FontStyle, Stylesheet, TextSpan};
use vocument_layout::font::FontRegistry;
use vocument_layout::layout_document;

fn span(text: &str) -> TextSpan {
    TextSpan {
        text: text.to_string(),
        style: FontStyle::PLAIN,
        font_size_pt: Some(11.0),
        font_family: Some("Inter".to_string()),
        font_weight: None,
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: None,
        extra: Default::default(),
    }
}

fn bold_span(text: &str) -> TextSpan {
    TextSpan {
        text: text.to_string(),
        style: FontStyle::bold(),
        font_size_pt: Some(11.0),
        font_family: Some("Inter".to_string()),
        font_weight: Some(700),
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: None,
        extra: Default::default(),
    }
}

fn span_with_footnote(text: &str, fn_id: &str) -> TextSpan {
    TextSpan {
        text: text.to_string(),
        style: FontStyle::PLAIN,
        font_size_pt: Some(11.0),
        font_family: Some("Inter".to_string()),
        font_weight: None,
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: Some(fn_id.to_string()),
        extra: Default::default(),
    }
}

#[test]
fn test_complex_legal_brief_pagination() {
    let registry = FontRegistry::new();

    let mut metadata = DocumentMetadata::new("brief_doc", "Klageerwiderung Az. 14 O 8921/26");
    metadata
        .authors
        .push(Author::new("Rechtsanwälte Dr. Muster & Kollegen"));
    metadata.description = Some("Rechtsstreit Alpha GmbH ./. Beta AG".to_string());

    let mut doc = FlowDocument::new(metadata);
    doc.stylesheet = Stylesheet::standard_legal();

    // Register footnotes
    let fn1_block = Block::Paragraph(ParagraphBlock::new(
        "fn1_p",
        vec![span(
            "BGH, Urteil vom 14. Mai 2024 – VI ZR 182/22, BGHZ 238, 14 ff.",
        )],
    ));
    doc.footnotes.insert(
        "fn1".to_string(),
        FootnoteDefinition::new("fn1", 1, vec![fn1_block]),
    );

    let fn2_block = Block::Paragraph(ParagraphBlock::new(
        "fn2_p",
        vec![span(
            "Vgl. Palandt/Grüneberg, BGB, 83. Aufl. 2024, § 280 Rn. 18 ff.",
        )],
    ));
    doc.footnotes.insert(
        "fn2".to_string(),
        FootnoteDefinition::new("fn2", 2, vec![fn2_block]),
    );

    let fn3_block = Block::Paragraph(ParagraphBlock::new(
        "fn3_p",
        vec![span(
            "EuGH, Urteil vom 21. Dezember 2023 – C-252/21 (Rechtskrafterfordernis).",
        )],
    ));
    doc.footnotes.insert(
        "fn3".to_string(),
        FootnoteDefinition::new("fn3", 3, vec![fn3_block]),
    );

    let mut blocks = Vec::new();

    // 1. Brief Rubrum / Party designation
    let mut h1 = HeadingBlock::new(
        "h_rubrum",
        1,
        vec![bold_span("An das Landgericht München I")],
    );
    h1.numbered = false;
    blocks.push(Block::Heading(h1));

    let court_address = "Zivilkammer 14\nLenbachplatz 7\n80333 München";
    blocks.push(Block::Paragraph(ParagraphBlock::new(
        "p_court_addr",
        vec![span(court_address)],
    )));
    blocks.push(Block::ThematicBreak(ThematicBreakBlock {
        id: "div1".to_string(),
        extra: Default::default(),
    }));

    // Heading: Rechtsstreit
    let h2_rubrum = HeadingBlock::new("h_part", 2, vec![bold_span("In dem Rechtsstreit")]);
    blocks.push(Block::Heading(h2_rubrum));

    // Parties table
    let r1 = TableRow::new(vec![
        TableCell::new(
            "p1",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_plaint",
                vec![
                    bold_span("Alpha Technologies GmbH"),
                    span(", vertreten durch die Geschäftsführer"),
                ],
            ))],
        ),
        TableCell::new(
            "p2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_role1",
                vec![bold_span("– Klägerin –")],
            ))],
        ),
    ]);
    let r2 = TableRow::new(vec![
        TableCell::new(
            "p3",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_def",
                vec![
                    bold_span("Beta Automotive AG"),
                    span(", vertreten durch den Vorstand"),
                ],
            ))],
        ),
        TableCell::new(
            "p4",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_role2",
                vec![bold_span("– Beklagte –")],
            ))],
        ),
    ]);
    let parties_table =
        TableBlock::new("tbl_parties", vec![r1, r2]).with_column_widths(vec![320.0, 130.0]);
    blocks.push(Block::Table(parties_table));

    // Callout: Urgent Notice
    let urgent_notice = CalloutBlock {
        id: "callout_urgent".to_string(),
        callout_type: CalloutType::LegalNotice,
        title: Some("EILBEDÜRFTIGKEIT / FRISTWAHRUNG".to_string()),
        blocks: vec![Block::Paragraph(ParagraphBlock::new(
            "p_urgent",
            vec![span(
                "Es wird vorab per Telefax übermittelt zur Wahrung der Notfrist gemäß § 276 Abs. 1 ZPO.",
            )],
        ))],
        extra: Default::default(),
    };
    blocks.push(Block::Callout(urgent_notice));

    // 2. Motions (Anträge)
    let mut h_motions = HeadingBlock::new("h_motions", 2, vec![bold_span("I. Anträge")]);
    h_motions.numbered = true;
    blocks.push(Block::Heading(h_motions));

    let motion_items = vec![
        ListItem::new("m1", vec![span("Die Klage wird kostenfällig abgewiesen.")]),
        ListItem::new(
            "m2",
            vec![span(
                "Die Klägerin trägt die Kosten des Rechtsstreits einschließlich der außergerichtlichen Kosten der Beklagten.",
            )],
        ),
        ListItem::new(
            "m3",
            vec![span(
                "Das Urteil ist vorläufig vollstreckbar gegen Sicherheitsleistung in Höhe von 110 % des jeweils zu vollstreckenden Betrages.",
            )],
        ),
    ];
    let motions_list = ListBlock::new("list_motions", ListKind::Ordered, motion_items);
    blocks.push(Block::List(motions_list));

    // 3. Statement of Defense (Begründung)
    let mut h_statement = HeadingBlock::new("h_statement", 2, vec![bold_span("II. Begründung")]);
    h_statement.numbered = true;
    blocks.push(Block::Heading(h_statement));

    let h_sub1 = HeadingBlock::new(
        "h_sub1",
        3,
        vec![span("1. Fehlende Aktivlegitimation und Sachvortrag")],
    );
    blocks.push(Block::Heading(h_sub1));

    let para1_text = "Die Klagepartei stützt ihre geltend gemachten Ansprüche auf angebliche Gewährleistungsrechte aus dem Rahmenvertrag vom 12. Januar 2025. Entgegen der unzutreffenden Darstellung der Klägerin liegt jedoch bereits kein schlüssiger Sachvortrag zu einem Sachmangel im Sinne des § 434 BGB vor.";
    blocks.push(Block::Paragraph(ParagraphBlock::new(
        "p_begr_1",
        vec![span_with_footnote(para1_text, "fn1")],
    )));

    let para2_text = "Vielmehr entspricht die gelieferte Softwarekomponente exakt der vereinbarten Spezifikation gemäß Pflichtenheft Version 2.4. Dies ergibt sich unzweideutig aus dem Abnahmeprotokoll vom 15. März 2025, welches von beiden Parteien vorbehaltlos unterzeichnet worden ist.";
    blocks.push(Block::Paragraph(ParagraphBlock::new(
        "p_begr_2",
        vec![span_with_footnote(para2_text, "fn2")],
    )));

    // 4. Large Multi-page Claims Table
    let h_table = HeadingBlock::new(
        "h_table_head",
        3,
        vec![span("2. Schadens- und Rechnungsprüfung")],
    );
    blocks.push(Block::Heading(h_table));

    let mut table_rows = Vec::new();
    let th = TableRow::header(vec![
        TableCell::new(
            "th1",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth1",
                vec![bold_span("Pos.")],
            ))],
        ),
        TableCell::new(
            "th2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth2",
                vec![bold_span("Rechnungs-Nr.")],
            ))],
        ),
        TableCell::new(
            "th3",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth3",
                vec![bold_span("Bezeichnung / Rüge")],
            ))],
        ),
        TableCell::new(
            "th4",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth4",
                vec![bold_span("Forderung (€)")],
            ))],
        ),
        TableCell::new(
            "th5",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth5",
                vec![bold_span("Bewertung der Beklagten")],
            ))],
        ),
    ]);
    table_rows.push(th);

    // 24 detailed rows
    for i in 1..=24 {
        let r = TableRow::new(vec![
            TableCell::new(
                format!("c_pos_{}", i),
                vec![Block::Paragraph(ParagraphBlock::new(
                    format!("ppos_{}", i),
                    vec![span(&format!("{}.", i))],
                ))],
            ),
            TableCell::new(
                format!("c_inv_{}", i),
                vec![Block::Paragraph(ParagraphBlock::new(
                    format!("pinv_{}", i),
                    vec![span(&format!("RE-2025-{:04}", 1000 + i))],
                ))],
            ),
            TableCell::new(
                format!("c_desc_{}", i),
                vec![Block::Paragraph(ParagraphBlock::new(
                    format!("pdesc_{}", i),
                    vec![span(&format!(
                        "Modul-Anpassung Teillieferung Charge #{}",
                        i
                    ))],
                ))],
            ),
            TableCell::new(
                format!("c_amt_{}", i),
                vec![Block::Paragraph(ParagraphBlock::new(
                    format!("pamt_{}", i),
                    vec![span(&format!("{}.50 €", 1250 * i))],
                ))],
            ),
            TableCell::new(
                format!("c_eval_{}", i),
                vec![Block::Paragraph(ParagraphBlock::new(
                    format!("peval_{}", i),
                    vec![span(if i % 3 == 0 {
                        "Bestritten mit Nichtwissen gem. § 138 Abs. 4 ZPO"
                    } else {
                        "Unbegründet mangels Abnahmeverzugs"
                    })],
                ))],
            ),
        ]);
        table_rows.push(r);
    }

    let claim_table = TableBlock::new("tbl_claims", table_rows)
        .with_caption("Anlage B 1: Tabellarische Einzelfallprüfung der Klagpositionen")
        .with_column_widths(vec![35.0, 95.0, 150.0, 80.0, 90.0])
        .with_repeat_header_rows(1);
    blocks.push(Block::Table(claim_table));

    // 5. Final Concluding paragraph with footnote 3
    let concluding_para = "Aus den dargelegten Gründen ist der Klage jeglicher Erfolg zu versagen. Ein Anspruch auf vorgerichtliche Rechtsanwaltskosten besteht mangels Verzugs ebenfalls nicht.";
    blocks.push(Block::Paragraph(ParagraphBlock::new(
        "p_concl",
        vec![span_with_footnote(concluding_para, "fn3")],
    )));

    // Create Section with Header and Footer
    let mut section = Section::new("sec_main", SectionGeometry::a4_portrait(), blocks);

    // Running Header with page numbering placeholder
    let header_span = TextSpan {
        text: "LG München I – Az. 14 O 8921/26 – Seite {page} von {total_pages}".to_string(),
        style: FontStyle::PLAIN,
        font_size_pt: Some(9.0),
        font_family: Some("Inter".to_string()),
        font_weight: None,
        color: Some(ColorRgba::rgb(120, 120, 120)),
        background_color: None,
        link_url: None,
        footnote_ref: None,
        extra: Default::default(),
    };
    section
        .header_blocks
        .push(Block::Paragraph(ParagraphBlock::new(
            "h_page",
            vec![header_span],
        )));

    // Running Footer
    let footer_span = TextSpan {
        text: "Beglaubigte Abschrift / Elektronisch signiertes Dokument".to_string(),
        style: FontStyle::PLAIN,
        font_size_pt: Some(8.5),
        font_family: Some("Inter".to_string()),
        font_weight: None,
        color: Some(ColorRgba::rgb(140, 140, 140)),
        background_color: None,
        link_url: None,
        footnote_ref: None,
        extra: Default::default(),
    };
    section
        .footer_blocks
        .push(Block::Paragraph(ParagraphBlock::new(
            "f_copy",
            vec![footer_span],
        )));

    doc.sections.push(section);

    // Perform Layout
    let layout = layout_document(&doc, &registry);

    // Quality assertions
    assert!(
        layout.total_pages >= 3,
        "Realistic legal brief with 24-row table must paginate onto at least 3 pages, got {}",
        layout.total_pages
    );

    for (p_idx, page) in layout.pages.iter().enumerate() {
        assert_eq!(page.page_number, p_idx + 1);
        assert!(
            !page.content_boxes.is_empty(),
            "Page {} must have content boxes",
            p_idx + 1
        );
        assert!(
            !page.header_boxes.is_empty(),
            "Page {} must have running header",
            p_idx + 1
        );
        assert!(
            !page.footer_boxes.is_empty(),
            "Page {} must have running footer",
            p_idx + 1
        );

        // Verify resolved page number placeholders in headers
        let header_text = page.header_boxes[0].plain_text();
        assert!(
            header_text.contains(&format!("Seite {} von {}", p_idx + 1, layout.total_pages)),
            "Header must resolve page number, got: {}",
            header_text
        );
    }

    // Verify footnotes are placed in footnote_boxes
    let has_footnotes = layout.pages.iter().any(|p| !p.footnote_boxes.is_empty());
    assert!(
        has_footnotes,
        "At least one page must have typeset footnote boxes"
    );
}
