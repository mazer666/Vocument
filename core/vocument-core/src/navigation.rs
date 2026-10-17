//! Logical page labels and page navigation (`PAGELABEL-001`..`PAGELABEL-004`, `ROADMAP.md` M0.3).
//!
//! Requirements:
//! - `PAGELABEL-001`: Support PDF logical page labels including Roman numerals and prefixes.
//! - `PAGELABEL-002`: Physical PDF page index remains visible/available.
//! - `PAGELABEL-003`: Navigate by physical index or logical label.
//! - `PAGELABEL-004`: Create/edit page-label ranges.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Numbering style for a logical page-label range (`PAGELABEL-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageLabelStyle {
    /// Decimal Arabic numerals (`1, 2, 3, ...`).
    DecimalArabic,
    /// Lowercase Roman numerals (`i, ii, iii, iv, ...`).
    RomanLower,
    /// Uppercase Roman numerals (`I, II, III, IV, ...`).
    RomanUpper,
    /// Lowercase Latin letters (`a, b, ..., z, aa, bb, ...`).
    AlphabeticLower,
    /// Uppercase Latin letters (`A, B, ..., Z, AA, BB, ...`).
    AlphabeticUpper,
    /// Only the prefix string is used (no numeric suffix).
    PrefixOnly,
}

/// A rule defining how logical page labels are formatted starting at `start_page_index` (`PAGELABEL-004`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageLabelRange {
    /// Zero-based physical PDF page index where this range begins.
    pub start_page_index: u32,
    pub style: PageLabelStyle,
    /// Optional prefix prepended to the formatted number (e.g. `"A-"` or `"Cover"`).
    pub prefix: String,
    /// Starting ordinal number (`>= 1`) for the first page in this range.
    pub start_number: u32,
}

/// Combined physical and logical identity of a page (`PAGELABEL-002`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageDescriptor {
    /// Zero-based physical page index (`0..page_count`).
    pub page_index: u32,
    /// One-based physical page number (`1..=page_count`).
    pub physical_page_number: u32,
    /// Resolved logical page label (e.g. `"i"`, `"ii"`, `"A-1"`, or `"1"`).
    pub logical_label: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PageNavigationError {
    #[error(
        "page navigation target '{target}' did not match any logical label or physical page (1..={page_count})"
    )]
    TargetNotFound { target: String, page_count: u32 },
    #[error(
        "invalid page-label range: start_page_index {start_page_index} is out of bounds (page count: {page_count})"
    )]
    RangeOutOfBounds {
        start_page_index: u32,
        page_count: u32,
    },
    #[error("invalid page-label range: start_number must be >= 1")]
    InvalidStartNumber,
}

/// Manages embedded PDF page labels and user-defined `PageLabelRange` overrides (`PAGELABEL-001`..`PAGELABEL-004`).
#[derive(Debug, Clone)]
pub struct PageLabelCatalog {
    page_count: u32,
    embedded_labels: Vec<Option<String>>,
    custom_ranges: Vec<PageLabelRange>,
}

impl PageLabelCatalog {
    pub fn new(page_count: u32, embedded_labels: Vec<Option<String>>) -> Self {
        Self {
            page_count,
            embedded_labels,
            custom_ranges: Vec::new(),
        }
    }

    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    pub fn custom_ranges(&self) -> &[PageLabelRange] {
        &self.custom_ranges
    }

    /// Replaces the custom page-label ranges (`PAGELABEL-004`).
    /// Ranges are sorted by `start_page_index`.
    pub fn set_custom_ranges(
        &mut self,
        mut ranges: Vec<PageLabelRange>,
    ) -> Result<(), PageNavigationError> {
        for r in &ranges {
            if r.start_page_index >= self.page_count && self.page_count > 0 {
                return Err(PageNavigationError::RangeOutOfBounds {
                    start_page_index: r.start_page_index,
                    page_count: self.page_count,
                });
            }
            if r.style != PageLabelStyle::PrefixOnly && r.start_number == 0 {
                return Err(PageNavigationError::InvalidStartNumber);
            }
        }
        ranges.sort_by_key(|r| r.start_page_index);
        ranges.dedup_by_key(|r| r.start_page_index);
        self.custom_ranges = ranges;
        Ok(())
    }

    /// Returns the `PageDescriptor` (`page_index`, `physical_page_number`, `logical_label`) for `page_index`.
    pub fn descriptor_for_page(&self, page_index: u32) -> Option<PageDescriptor> {
        if page_index >= self.page_count {
            return None;
        }
        let logical_label = self.label_for_page(page_index);
        Some(PageDescriptor {
            page_index,
            physical_page_number: page_index + 1,
            logical_label,
        })
    }

    /// Returns descriptors for all pages (`0..page_count`).
    pub fn all_descriptors(&self) -> Vec<PageDescriptor> {
        (0..self.page_count)
            .filter_map(|idx| self.descriptor_for_page(idx))
            .collect()
    }

    /// Computes the logical label for `page_index` (`PAGELABEL-001`, `PAGELABEL-002`).
    pub fn label_for_page(&self, page_index: u32) -> String {
        if let Some(active_range) = self
            .custom_ranges
            .iter()
            .rev()
            .find(|r| r.start_page_index <= page_index)
        {
            let offset = page_index.saturating_sub(active_range.start_page_index);
            let number = active_range.start_number.saturating_add(offset);
            return format_range_label(active_range.style, &active_range.prefix, number);
        }

        if let Some(Some(embedded)) = self.embedded_labels.get(page_index as usize)
            && !embedded.is_empty()
        {
            return embedded.clone();
        }

        (page_index + 1).to_string()
    }

    /// Resolves a user navigation input (`query`) by logical page label first, or by 1-based physical
    /// page number (`PAGELABEL-003`). Supports explicit `"#3"` syntax to force physical page `3`.
    pub fn resolve_navigation_target(
        &self,
        query: &str,
    ) -> Result<PageDescriptor, PageNavigationError> {
        let trimmed = query.trim();
        if trimmed.is_empty() || self.page_count == 0 {
            return Err(PageNavigationError::TargetNotFound {
                target: trimmed.to_owned(),
                page_count: self.page_count,
            });
        }

        // Explicit "#N" syntax always targets 1-based physical page N:
        if let Some(phys_str) = trimmed.strip_prefix('#')
            && let Ok(phys_num) = phys_str.trim().parse::<u32>()
            && phys_num >= 1
            && phys_num <= self.page_count
            && let Some(desc) = self.descriptor_for_page(phys_num - 1)
        {
            return Ok(desc);
        }

        // 1. Exact case-sensitive or case-insensitive logical label match:
        for idx in 0..self.page_count {
            let label = self.label_for_page(idx);
            if (label == trimmed || label.eq_ignore_ascii_case(trimmed))
                && let Some(desc) = self.descriptor_for_page(idx)
            {
                return Ok(desc);
            }
        }

        // 2. Fallback to 1-based physical page number:
        if let Ok(phys_num) = trimmed.parse::<u32>()
            && phys_num >= 1
            && phys_num <= self.page_count
            && let Some(desc) = self.descriptor_for_page(phys_num - 1)
        {
            return Ok(desc);
        }

        Err(PageNavigationError::TargetNotFound {
            target: trimmed.to_owned(),
            page_count: self.page_count,
        })
    }
}

fn format_range_label(style: PageLabelStyle, prefix: &str, number: u32) -> String {
    let suffix = match style {
        PageLabelStyle::DecimalArabic => number.to_string(),
        PageLabelStyle::RomanLower => to_roman(number).to_ascii_lowercase(),
        PageLabelStyle::RomanUpper => to_roman(number),
        PageLabelStyle::AlphabeticLower => to_alphabetic(number).to_ascii_lowercase(),
        PageLabelStyle::AlphabeticUpper => to_alphabetic(number),
        PageLabelStyle::PrefixOnly => String::new(),
    };
    format!("{prefix}{suffix}")
}

fn to_roman(mut n: u32) -> String {
    if n == 0 {
        return "0".to_owned();
    }
    let table: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for &(val, sym) in table {
        while n >= val {
            out.push_str(sym);
            n -= val;
        }
    }
    out
}

fn to_alphabetic(n: u32) -> String {
    if n == 0 {
        return String::new();
    }
    let zero_based = n - 1;
    let letter = (b'A' + (zero_based % 26) as u8) as char;
    let repeat = (zero_based / 26) + 1;
    std::iter::repeat_n(letter, repeat as usize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_roman_prefix_and_decimal_labels_and_resolves_targets() {
        let mut catalog = PageLabelCatalog::new(
            6,
            vec![
                Some("i".to_owned()),
                Some("ii".to_owned()),
                Some("1".to_owned()),
                Some("2".to_owned()),
                Some("3".to_owned()),
                Some("4".to_owned()),
            ],
        );

        assert_eq!(catalog.label_for_page(0), "i");
        assert_eq!(catalog.label_for_page(1), "ii");
        assert_eq!(catalog.label_for_page(2), "1");

        // Navigate by logical label "ii" -> page_index 1 (physical page 2)
        let nav_ii = catalog.resolve_navigation_target("ii").expect("resolve ii");
        assert_eq!(nav_ii.page_index, 1);
        assert_eq!(nav_ii.physical_page_number, 2);

        // Navigate by logical label "1" -> page_index 2 (physical page 3)
        let nav_1 = catalog.resolve_navigation_target("1").expect("resolve 1");
        assert_eq!(nav_1.page_index, 2);
        assert_eq!(nav_1.physical_page_number, 3);

        // Navigate by explicit physical page "#1" -> page_index 0
        let nav_phys_1 = catalog.resolve_navigation_target("#1").expect("resolve #1");
        assert_eq!(nav_phys_1.page_index, 0);

        // Edit page-label ranges (PAGELABEL-004): Cover, Roman I..II, Appendix A-1..A-3
        catalog
            .set_custom_ranges(vec![
                PageLabelRange {
                    start_page_index: 0,
                    style: PageLabelStyle::PrefixOnly,
                    prefix: "Cover".to_owned(),
                    start_number: 1,
                },
                PageLabelRange {
                    start_page_index: 1,
                    style: PageLabelStyle::RomanUpper,
                    prefix: String::new(),
                    start_number: 1,
                },
                PageLabelRange {
                    start_page_index: 3,
                    style: PageLabelStyle::DecimalArabic,
                    prefix: "A-".to_owned(),
                    start_number: 1,
                },
            ])
            .expect("set custom ranges");

        assert_eq!(catalog.label_for_page(0), "Cover");
        assert_eq!(catalog.label_for_page(1), "I");
        assert_eq!(catalog.label_for_page(2), "II");
        assert_eq!(catalog.label_for_page(3), "A-1");
        assert_eq!(catalog.label_for_page(5), "A-3");

        let nav_a2 = catalog
            .resolve_navigation_target("A-2")
            .expect("resolve A-2");
        assert_eq!(nav_a2.page_index, 4);
    }
}
