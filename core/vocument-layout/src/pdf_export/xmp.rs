//! XMP metadata packet for the PDF/A-2b and PDF/UA-1 export (`ADR-0020`, `ROADMAP.md` M4.4).

use std::fmt::Write as FmtWrite;

use vocument_flow::model::FlowDocument;

use super::PdfExportOptions;

/// Author name used when the document carries no author.
const FALLBACK_AUTHOR_NAME: &str = "Vocument User";

/// Escapes the five XML special characters.
fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// PDF/A extension schema that declares the `pdfuaid` namespace. ISO 19005-2 (6.6.2.3) demands
/// every XMP property outside the predefined schemas to be declared this way.
const PDF_UA_EXTENSION_SCHEMA: &str = r#"<rdf:Description rdf:about="" xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/" xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#" xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#">
  <pdfaExtension:schemas>
    <rdf:Bag>
      <rdf:li rdf:parseType="Resource">
        <pdfaSchema:schema>PDF/UA identification schema</pdfaSchema:schema>
        <pdfaSchema:namespaceURI>http://www.aiim.org/pdfua/ns/id/</pdfaSchema:namespaceURI>
        <pdfaSchema:prefix>pdfuaid</pdfaSchema:prefix>
        <pdfaSchema:property>
          <rdf:Seq>
            <rdf:li rdf:parseType="Resource">
              <pdfaProperty:name>part</pdfaProperty:name>
              <pdfaProperty:valueType>Integer</pdfaProperty:valueType>
              <pdfaProperty:category>internal</pdfaProperty:category>
              <pdfaProperty:description>Indicates, which part of ISO 14289 standard is followed</pdfaProperty:description>
            </rdf:li>
          </rdf:Seq>
        </pdfaSchema:property>
      </rdf:li>
    </rdf:Bag>
  </pdfaExtension:schemas>
</rdf:Description>
"#;

/// Builds the XMP packet for a document. Titles, authors and descriptions are XML-escaped.
pub(super) fn build_xmp_packet(doc: &FlowDocument, options: &PdfExportOptions) -> String {
    let title = escape_xml(&doc.metadata.title);
    let author = escape_xml(
        doc.metadata
            .authors
            .first()
            .map_or(FALLBACK_AUTHOR_NAME, |author| author.name.as_str()),
    );
    let created = escape_xml(&doc.metadata.created_at_utc);
    let modified = escape_xml(&doc.metadata.modified_at_utc);

    let mut xmp = String::new();
    xmp.push_str("<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    xmp.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");

    if options.pdfa_conformance {
        xmp.push_str(
            "<rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n",
        );
        xmp.push_str("  <pdfaid:part>2</pdfaid:part>\n  <pdfaid:conformance>B</pdfaid:conformance>\n</rdf:Description>\n");
    }
    if options.pdfua_conformance {
        if options.pdfa_conformance {
            xmp.push_str(PDF_UA_EXTENSION_SCHEMA);
        }
        xmp.push_str(
            "<rdf:Description rdf:about=\"\" xmlns:pdfuaid=\"http://www.aiim.org/pdfua/ns/id/\">\n",
        );
        xmp.push_str("  <pdfuaid:part>1</pdfuaid:part>\n</rdf:Description>\n");
    }

    xmp.push_str("<rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">\n");
    let _ = writeln!(xmp, "  <xmp:CreatorTool>Vocument</xmp:CreatorTool>");
    let _ = writeln!(xmp, "  <xmp:CreateDate>{created}</xmp:CreateDate>");
    let _ = writeln!(xmp, "  <xmp:ModifyDate>{modified}</xmp:ModifyDate>");
    xmp.push_str("</rdf:Description>\n");

    xmp.push_str(
        "<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n",
    );
    let _ = writeln!(
        xmp,
        "  <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>"
    );
    let _ = writeln!(
        xmp,
        "  <dc:creator><rdf:Seq><rdf:li>{author}</rdf:li></rdf:Seq></dc:creator>"
    );
    if !options.certified_by_external_tooling {
        xmp.push_str("  <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">Experimental Tagged PDF/A-2b and PDF/UA-1 draft profile. Independent external validation certification pending.</rdf:li></rdf:Alt></dc:description>\n");
    }
    xmp.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>");
    xmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use vocument_flow::metadata::DocumentMetadata;

    #[test]
    fn titles_with_markup_characters_stay_well_formed() {
        let doc = FlowDocument::new(DocumentMetadata::new("d", "A & B <draft>"));
        let packet = build_xmp_packet(&doc, &PdfExportOptions::default());
        assert!(packet.contains("A &amp; B &lt;draft&gt;"));
        assert!(!packet.contains("A & B"));
    }

    #[test]
    fn pdf_ua_namespace_is_declared_for_pdf_a() {
        let doc = FlowDocument::new(DocumentMetadata::new("d", "T"));
        let packet = build_xmp_packet(&doc, &PdfExportOptions::default());
        assert!(packet.contains("<pdfaProperty:category>internal</pdfaProperty:category>"));
        assert!(packet.contains("<pdfuaid:part>1</pdfuaid:part>"));
    }
}
