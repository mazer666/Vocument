//! Hardened, namespace-aware XML reader and escaping helpers for Office parts.
//!
//! Office files are untrusted input. This parser therefore **refuses** every construct that
//! has ever been used to attack XML consumers:
//!
//! - `<!DOCTYPE …>` and entity declarations (XXE, "billion laughs", external subsets) are an
//!   error, so only the five predefined entities and numeric character references exist;
//! - nesting depth, node count and attribute count are bounded by [`OfficeLimits`];
//! - namespace prefixes must be declared; nothing is ever fetched from a URI.
//!
//! The parser is iterative (no recursion), so a deeply nested file cannot overflow the stack.

use crate::error::OfficeError;
use crate::limits::OfficeLimits;

/// A namespace-qualified attribute. Unprefixed attributes have an empty namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub namespace: String,
    pub prefix: String,
    pub local: String,
    pub value: String,
}

/// An XML node: element or character data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

/// An XML element with resolved namespace.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    pub namespace: String,
    /// Prefix as written in the source; kept so a fragment can be written back faithfully.
    pub prefix: String,
    pub local: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Node>,
}

impl Element {
    /// True when the element has the given namespace URI and local name.
    pub fn is(&self, namespace: &str, local: &str) -> bool {
        self.local == local && self.namespace == namespace
    }

    /// Attribute value by namespace URI and local name.
    pub fn attribute(&self, namespace: &str, local: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.local == local && attribute.namespace == namespace)
            .map(|attribute| attribute.value.as_str())
    }

    /// Attribute value by local name, whatever its namespace. Used where producers disagree on
    /// whether an attribute is namespaced (for example OOXML `w:val` against strict `val`).
    pub fn attribute_any_namespace(&self, local: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.local == local)
            .map(|attribute| attribute.value.as_str())
    }

    /// Child elements in document order.
    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    /// First child element with the given name.
    pub fn child(&self, namespace: &str, local: &str) -> Option<&Element> {
        self.child_elements()
            .find(|element| element.is(namespace, local))
    }

    /// Concatenated character data of the element and all of its descendants.
    pub fn text_content(&self) -> String {
        let mut text = String::new();
        let mut stack: Vec<&Node> = self.children.iter().rev().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Text(value) => text.push_str(value),
                Node::Element(element) => stack.extend(element.children.iter().rev()),
            }
        }
        text
    }
}

struct OpenElement {
    element: Element,
    raw_name: String,
    namespace_bindings: usize,
}

/// Parses an XML document into a tree. `part` names the package part for error messages.
pub fn parse(bytes: &[u8], part: &str, limits: &OfficeLimits) -> Result<Element, OfficeError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if bytes.starts_with(b"\xFF\xFE") || bytes.starts_with(b"\xFE\xFF") {
        return Err(malformed(part, "UTF-16 encoded XML is not supported"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| malformed(part, "not valid UTF-8"))?;
    Parser {
        input: text,
        position: 0,
        part,
        limits,
        node_count: 0,
        bindings: vec![(
            "xml".to_string(),
            "http://www.w3.org/XML/1998/namespace".to_string(),
        )],
    }
    .run()
}

fn malformed(part: &str, details: impl Into<String>) -> OfficeError {
    OfficeError::MalformedXml {
        part: part.to_string(),
        details: details.into(),
    }
}

struct Parser<'a> {
    input: &'a str,
    position: usize,
    part: &'a str,
    limits: &'a OfficeLimits,
    node_count: usize,
    /// In-scope `(prefix, uri)` bindings; the default namespace uses the empty prefix.
    bindings: Vec<(String, String)>,
}

impl Parser<'_> {
    fn error(&self, details: impl Into<String>) -> OfficeError {
        malformed(
            self.part,
            format!("{} (at byte {})", details.into(), self.position),
        )
    }

    fn rest(&self) -> &str {
        &self.input[self.position..]
    }

    fn count_node(&mut self) -> Result<(), OfficeError> {
        self.node_count += 1;
        if self.node_count > self.limits.max_xml_nodes {
            return Err(OfficeError::ResourceLimitExceeded {
                what: format!("XML nodes in '{}'", self.part),
                limit: self.limits.max_xml_nodes as u64,
            });
        }
        Ok(())
    }

    fn run(mut self) -> Result<Element, OfficeError> {
        let mut stack: Vec<OpenElement> = Vec::new();
        let mut root: Option<Element> = None;

        while self.position < self.input.len() {
            if self.rest().starts_with('<') {
                if self.rest().starts_with("<?") {
                    self.skip_until("?>", "processing instruction")?;
                } else if self.rest().starts_with("<!--") {
                    self.skip_until("-->", "comment")?;
                } else if self.rest().starts_with("<![CDATA[") {
                    self.position += "<![CDATA[".len();
                    let end = self
                        .rest()
                        .find("]]>")
                        .ok_or_else(|| self.error("unterminated CDATA section"))?;
                    let text = self.rest()[..end].to_string();
                    self.position += end + 3;
                    self.append_text(&mut stack, text)?;
                } else if self.rest().starts_with("<!") {
                    return Err(self.error(
                        "DTD, DOCTYPE and entity declarations are prohibited in Office documents",
                    ));
                } else if self.rest().starts_with("</") {
                    self.close_element(&mut stack, &mut root)?;
                } else {
                    self.open_element(&mut stack, &mut root)?;
                }
            } else {
                let end = self.rest().find('<').unwrap_or(self.rest().len());
                let raw = &self.input[self.position..self.position + end];
                let decoded = self.decode_entities(raw)?;
                self.position += end;
                self.append_text(&mut stack, decoded)?;
            }
        }

        if !stack.is_empty() {
            return Err(self.error("unexpected end of document: unclosed element"));
        }
        root.ok_or_else(|| self.error("document has no root element"))
    }

    fn skip_until(&mut self, terminator: &str, what: &str) -> Result<(), OfficeError> {
        let end = self
            .rest()
            .find(terminator)
            .ok_or_else(|| self.error(format!("unterminated {what}")))?;
        self.position += end + terminator.len();
        Ok(())
    }

    fn append_text(&mut self, stack: &mut [OpenElement], text: String) -> Result<(), OfficeError> {
        if text.is_empty() {
            return Ok(());
        }
        match stack.last_mut() {
            Some(open) => {
                if let Some(Node::Text(existing)) = open.element.children.last_mut() {
                    existing.push_str(&text);
                } else {
                    self.count_node()?;
                    open.element.children.push(Node::Text(text));
                }
                Ok(())
            }
            None if text.trim().is_empty() => Ok(()),
            None => Err(self.error("character data outside of the root element")),
        }
    }

    fn open_element(
        &mut self,
        stack: &mut Vec<OpenElement>,
        root: &mut Option<Element>,
    ) -> Result<(), OfficeError> {
        if root.is_some() && stack.is_empty() {
            return Err(self.error("more than one root element"));
        }
        if stack.len() >= self.limits.max_xml_depth {
            return Err(OfficeError::ResourceLimitExceeded {
                what: format!("XML nesting depth in '{}'", self.part),
                limit: self.limits.max_xml_depth as u64,
            });
        }
        self.count_node()?;
        self.position += 1; // '<'
        let raw_name = self.read_name()?;

        let mut raw_attributes: Vec<(String, String)> = Vec::new();
        let self_closing;
        loop {
            self.skip_whitespace();
            if self.rest().starts_with("/>") {
                self.position += 2;
                self_closing = true;
                break;
            }
            if self.rest().starts_with('>') {
                self.position += 1;
                self_closing = false;
                break;
            }
            if self.position >= self.input.len() {
                return Err(self.error("unterminated start tag"));
            }
            let name = self.read_name()?;
            self.skip_whitespace();
            if !self.rest().starts_with('=') {
                return Err(self.error(format!("attribute '{name}' has no value")));
            }
            self.position += 1;
            self.skip_whitespace();
            let quote = self
                .rest()
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or_else(|| self.error("attribute value is not quoted"))?;
            self.position += 1;
            let end = self
                .rest()
                .find(quote)
                .ok_or_else(|| self.error("unterminated attribute value"))?;
            let raw_value = &self.input[self.position..self.position + end];
            if raw_value.contains('<') {
                return Err(self.error("'<' is not allowed in an attribute value"));
            }
            let value = self.decode_entities(raw_value)?;
            self.position += end + 1;
            if raw_attributes.iter().any(|(existing, _)| *existing == name) {
                return Err(self.error(format!("duplicate attribute '{name}'")));
            }
            if raw_attributes.len() >= self.limits.max_xml_attributes {
                return Err(OfficeError::ResourceLimitExceeded {
                    what: format!("XML attributes on one element in '{}'", self.part),
                    limit: self.limits.max_xml_attributes as u64,
                });
            }
            raw_attributes.push((name, value));
        }

        // Namespace declarations come into scope for this element and its descendants.
        let binding_mark = self.bindings.len();
        for (name, value) in &raw_attributes {
            if name == "xmlns" {
                self.bindings.push((String::new(), value.clone()));
            } else if let Some(prefix) = name.strip_prefix("xmlns:") {
                self.bindings.push((prefix.to_string(), value.clone()));
            }
        }

        let (namespace, local) = self.resolve_name(&raw_name, true)?;
        let prefix = split_prefix(&raw_name);
        let mut attributes = Vec::with_capacity(raw_attributes.len());
        for (name, value) in raw_attributes {
            if name == "xmlns" || name.starts_with("xmlns:") {
                continue;
            }
            let (attribute_namespace, attribute_local) = self.resolve_name(&name, false)?;
            attributes.push(Attribute {
                namespace: attribute_namespace,
                prefix: split_prefix(&name),
                local: attribute_local,
                value,
            });
        }

        let element = Element {
            namespace,
            prefix,
            local,
            attributes,
            children: Vec::new(),
        };
        let bindings_added = self.bindings.len() - binding_mark;
        if self_closing {
            self.bindings.truncate(binding_mark);
            self.attach(stack, root, element);
        } else {
            stack.push(OpenElement {
                element,
                raw_name,
                namespace_bindings: bindings_added,
            });
        }
        Ok(())
    }

    fn close_element(
        &mut self,
        stack: &mut Vec<OpenElement>,
        root: &mut Option<Element>,
    ) -> Result<(), OfficeError> {
        self.position += 2; // "</"
        let name = self.read_name()?;
        self.skip_whitespace();
        if !self.rest().starts_with('>') {
            return Err(self.error("malformed end tag"));
        }
        self.position += 1;
        let open = stack
            .pop()
            .ok_or_else(|| self.error("end tag without matching start tag"))?;
        if open.raw_name != name {
            return Err(self.error(format!(
                "end tag '{name}' does not match start tag '{}'",
                open.raw_name
            )));
        }
        let new_length = self.bindings.len() - open.namespace_bindings;
        self.bindings.truncate(new_length);
        self.attach(stack, root, open.element);
        Ok(())
    }

    fn attach(&mut self, stack: &mut [OpenElement], root: &mut Option<Element>, element: Element) {
        match stack.last_mut() {
            Some(parent) => parent.element.children.push(Node::Element(element)),
            None => *root = Some(element),
        }
    }

    fn skip_whitespace(&mut self) {
        let trimmed = self.rest().trim_start_matches([' ', '\t', '\r', '\n']);
        self.position = self.input.len() - trimmed.len();
    }

    fn read_name(&mut self) -> Result<String, OfficeError> {
        let rest = self.rest();
        let end = rest
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '=' | '>' | '/' | '<' | '"' | '\'' | '?' | '!')
            })
            .unwrap_or(rest.len());
        if end == 0 {
            return Err(self.error("expected a name"));
        }
        let name = rest[..end].to_string();
        self.position += end;
        Ok(name)
    }

    fn resolve_name(&self, raw: &str, use_default: bool) -> Result<(String, String), OfficeError> {
        match raw.split_once(':') {
            Some((prefix, local)) => {
                let uri = self
                    .bindings
                    .iter()
                    .rev()
                    .find(|(bound, _)| bound == prefix)
                    .map(|(_, uri)| uri.clone())
                    .ok_or_else(|| self.error(format!("undeclared namespace prefix '{prefix}'")))?;
                Ok((uri, local.to_string()))
            }
            None if use_default => {
                let uri = self
                    .bindings
                    .iter()
                    .rev()
                    .find(|(bound, _)| bound.is_empty())
                    .map(|(_, uri)| uri.clone())
                    .unwrap_or_default();
                Ok((uri, raw.to_string()))
            }
            None => Ok((String::new(), raw.to_string())),
        }
    }

    fn decode_entities(&self, raw: &str) -> Result<String, OfficeError> {
        if !raw.contains('&') {
            return Ok(raw.to_string());
        }
        let mut decoded = String::with_capacity(raw.len());
        let mut rest = raw;
        while let Some(start) = rest.find('&') {
            decoded.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            let end = after
                .find(';')
                .ok_or_else(|| self.error("unterminated entity reference"))?;
            let entity = &after[..end];
            match entity {
                "amp" => decoded.push('&'),
                "lt" => decoded.push('<'),
                "gt" => decoded.push('>'),
                "quot" => decoded.push('"'),
                "apos" => decoded.push('\''),
                _ => {
                    let code = if let Some(hex) = entity
                        .strip_prefix("#x")
                        .or_else(|| entity.strip_prefix("#X"))
                    {
                        u32::from_str_radix(hex, 16).ok()
                    } else if let Some(decimal) = entity.strip_prefix('#') {
                        decimal.parse::<u32>().ok()
                    } else {
                        None
                    };
                    let character = code
                        .and_then(char::from_u32)
                        .filter(|c| *c != '\0')
                        .ok_or_else(|| {
                            self.error(format!("unknown or invalid entity '&{entity};'"))
                        })?;
                    decoded.push(character);
                }
            }
            rest = &after[end + 1..];
        }
        decoded.push_str(rest);
        Ok(decoded)
    }
}

fn split_prefix(raw_name: &str) -> String {
    raw_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

/// Serialises an element subtree back to XML text, declaring every namespace it uses on the
/// root. Used to keep unmapped fragments (for example unsupported paragraph properties) so the
/// same format can write them back (`OFFICE-005`).
pub fn serialize(element: &Element) -> String {
    let mut declarations: Vec<(String, String)> = Vec::new();
    collect_namespaces(element, &mut declarations);
    let mut output = String::new();
    write_element(element, &declarations, true, &mut output);
    output
}

fn collect_namespaces(element: &Element, declarations: &mut Vec<(String, String)>) {
    let mut stack = vec![element];
    while let Some(current) = stack.pop() {
        let mut note = |prefix: &str, namespace: &str| {
            if !namespace.is_empty()
                && prefix != "xml"
                && !declarations.iter().any(|(p, _)| p == prefix)
            {
                declarations.push((prefix.to_string(), namespace.to_string()));
            }
        };
        note(&current.prefix, &current.namespace);
        for attribute in &current.attributes {
            note(&attribute.prefix, &attribute.namespace);
        }
        stack.extend(current.child_elements());
    }
}

fn qualified(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn write_element(
    element: &Element,
    declarations: &[(String, String)],
    is_root: bool,
    output: &mut String,
) {
    output.push('<');
    output.push_str(&qualified(&element.prefix, &element.local));
    if is_root {
        for (prefix, namespace) in declarations {
            if prefix.is_empty() {
                output.push_str(&format!(" xmlns=\"{}\"", escape_attribute(namespace)));
            } else {
                output.push_str(&format!(
                    " xmlns:{prefix}=\"{}\"",
                    escape_attribute(namespace)
                ));
            }
        }
    }
    for attribute in &element.attributes {
        output.push_str(&format!(
            " {}=\"{}\"",
            qualified(&attribute.prefix, &attribute.local),
            escape_attribute(&attribute.value)
        ));
    }
    if element.children.is_empty() {
        output.push_str("/>");
        return;
    }
    output.push('>');
    for child in &element.children {
        match child {
            Node::Text(text) => output.push_str(&escape_text(text)),
            Node::Element(nested) => write_element(nested, declarations, false, output),
        }
    }
    output.push_str("</");
    output.push_str(&qualified(&element.prefix, &element.local));
    output.push('>');
}

/// Escapes text for use as XML character data. Characters XML 1.0 cannot represent are dropped.
pub fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            c if is_xml_char(c) => escaped.push(c),
            _ => {}
        }
    }
    escaped
}

/// Escapes text for use inside a double-quoted attribute value.
pub fn escape_attribute(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\t' => escaped.push_str("&#9;"),
            '\n' => escaped.push_str("&#10;"),
            '\r' => escaped.push_str("&#13;"),
            c if is_xml_char(c) => escaped.push(c),
            _ => {}
        }
    }
    escaped
}

/// True for the characters XML 1.0 allows.
fn is_xml_char(character: char) -> bool {
    matches!(character,
        '\t' | '\n' | '\r'
        | '\u{20}'..='\u{D7FF}'
        | '\u{E000}'..='\u{FFFD}'
        | '\u{10000}'..='\u{10FFFF}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(xml: &str) -> Result<Element, OfficeError> {
        parse(xml.as_bytes(), "test.xml", &OfficeLimits::default())
    }

    #[test]
    fn resolves_namespaces_and_entities() {
        let root = parse_str(
            r#"<?xml version="1.0"?><a:r xmlns:a="urn:a" xmlns="urn:d"><b x="1&amp;2">t&lt;&#x41;</b></a:r>"#,
        )
        .expect("parse");
        assert!(root.is("urn:a", "r"));
        let child = root.child("urn:d", "b").expect("default namespace child");
        assert_eq!(child.attribute("", "x"), Some("1&2"));
        assert_eq!(child.text_content(), "t<A");
    }

    #[test]
    fn rejects_doctype_and_entity_declarations() {
        let xxe = r#"<?xml version="1.0"?><!DOCTYPE r [<!ENTITY x SYSTEM "file:///etc/passwd">]><r>&x;</r>"#;
        let error = parse_str(xxe).expect_err("XXE must be refused");
        assert!(error.to_string().contains("prohibited"), "{error}");

        let bomb = r#"<!DOCTYPE l [<!ENTITY a "aaaa"><!ENTITY b "&a;&a;&a;">]><l>&b;</l>"#;
        assert!(parse_str(bomb).is_err());
    }

    #[test]
    fn rejects_unknown_entities_and_undeclared_prefixes() {
        assert!(parse_str("<r>&nbsp;</r>").is_err());
        assert!(parse_str("<p:r/>").is_err());
        assert!(parse_str("<r>&#0;</r>").is_err());
    }

    #[test]
    fn rejects_malformed_structure() {
        assert!(parse_str("<a><b></a></b>").is_err());
        assert!(parse_str("<a>").is_err());
        assert!(parse_str("<a/><b/>").is_err());
        assert!(parse_str("text").is_err());
        assert!(parse_str("<a x=1/>").is_err());
        assert!(parse_str(r#"<a x="1" x="2"/>"#).is_err());
    }

    #[test]
    fn enforces_depth_and_node_limits_without_recursion() {
        let limits = OfficeLimits {
            max_xml_depth: 50,
            ..OfficeLimits::default()
        };
        let deep = format!("{}{}", "<a>".repeat(60), "</a>".repeat(60));
        let error = parse(deep.as_bytes(), "deep.xml", &limits).expect_err("too deep");
        assert!(matches!(error, OfficeError::ResourceLimitExceeded { .. }));

        let limits = OfficeLimits {
            max_xml_nodes: 10,
            ..OfficeLimits::default()
        };
        let wide = format!("<r>{}</r>", "<c/>".repeat(100));
        assert!(matches!(
            parse(wide.as_bytes(), "wide.xml", &limits),
            Err(OfficeError::ResourceLimitExceeded { .. })
        ));

        // Far beyond any recursion budget; must still be refused cleanly, not crash.
        let abyss = format!("{}{}", "<a>".repeat(200_000), "</a>".repeat(200_000));
        assert!(parse(abyss.as_bytes(), "abyss.xml", &OfficeLimits::default()).is_err());
    }

    #[test]
    fn serialisation_roundtrips_a_fragment() {
        let source = r#"<w:pPr xmlns:w="urn:w" xmlns:x="urn:x"><w:bidi/><x:a x:k="v&amp;1">t&lt;</x:a></w:pPr>"#;
        let element = parse_str(source).expect("parse");
        let written = serialize(&element);
        assert_eq!(parse_str(&written).expect("reparse"), element);
        assert!(written.contains("xmlns:x=\"urn:x\""));
    }

    #[test]
    fn cdata_comments_and_bom() {
        let root = parse(
            b"\xEF\xBB\xBF<r><!-- c --><![CDATA[<x>&]]>tail</r>",
            "bom.xml",
            &OfficeLimits::default(),
        )
        .expect("parse");
        assert_eq!(root.text_content(), "<x>&tail");
    }

    #[test]
    fn escaping_roundtrips_through_the_parser() {
        let nasty = "a<b>&\"'\u{1}\u{0}ä😀";
        let xml = format!(
            r#"<r v="{}">{}</r>"#,
            escape_attribute(nasty),
            escape_text(nasty)
        );
        let root = parse_str(&xml).expect("parse");
        let expected = "a<b>&\"'ä😀";
        assert_eq!(root.text_content(), expected);
        assert_eq!(root.attribute("", "v"), Some(expected));
    }
}
