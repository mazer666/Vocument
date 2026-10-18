//! Multi-language detection heuristics and legal variant awareness (`ARCHITECTURE.md` §20, `REQUIREMENTS.md` §9).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Result of automatic language detection and classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageDetectionResult {
    pub primary_language: String,
    pub confidence: f32,
    pub is_multilingual: bool,
    pub german_score: f32,
    pub english_score: f32,
    pub variant_hint: Option<String>,
}

/// Detects primary document language (German `deu` vs English `eng`), checks for multilingual
/// mixed content, and identifies regional legal dialect markers (`de-AT` vs `de-DE`).
pub fn detect_language(text: &str) -> LanguageDetectionResult {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| {
            !c.is_alphanumeric() && c != '§' && c != 'ä' && c != 'ö' && c != 'ü' && c != 'ß'
        })
        .filter(|w| !w.is_empty())
        .collect();

    if words.is_empty() {
        return LanguageDetectionResult {
            primary_language: "eng".into(),
            confidence: 0.5,
            is_multilingual: false,
            german_score: 0.0,
            english_score: 0.0,
            variant_hint: None,
        };
    }

    let mut german_score = 0.0f32;
    let mut english_score = 0.0f32;

    // Character-level indicators:
    for c in text.chars() {
        match c {
            'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü' => german_score += 3.0,
            'ß' => german_score += 4.0,
            '§' => german_score += 2.0,
            _ => {}
        }
    }

    let german_stopwords: HashSet<&str> = [
        "der", "die", "das", "und", "für", "in", "mit", "nicht", "eine", "einer", "eines", "einem",
        "auf", "nach", "über", "gemäß", "vom", "zum", "den", "dem", "des", "zu", "ist", "sind",
        "alle", "aller", "sowie", "durch", "oder", "bei", "dieser", "dieses",
    ]
    .iter()
    .copied()
    .collect();

    let german_legal_terms: HashSet<&str> = [
        "vereinbarung",
        "vertrag",
        "vertragsparteien",
        "geltungsbereich",
        "geheimhaltung",
        "geschäftsgeheimnisse",
        "bestimmungen",
        "pflichten",
        "erfüllungsort",
        "gerichtsstand",
        "schadenersatz",
        "haftung",
        "abgb",
        "uwg",
        "bgb",
        "hgb",
        "ustg",
        "rechnung",
        "gesamtbetrag",
        "österreich",
        "wien",
        "deutschland",
        "berlin",
        "schriftform",
        "informationsempfängers",
        "vertraulichkeit",
        "vertrauliche",
        "unterlagen",
    ]
    .iter()
    .copied()
    .collect();

    let english_stopwords: HashSet<&str> = [
        "the", "and", "of", "to", "in", "for", "with", "that", "this", "by", "from", "as", "or",
        "between", "each", "shall", "which", "not", "is", "are", "have", "has", "be", "been",
        "such", "any", "all", "its", "their", "under",
    ]
    .iter()
    .copied()
    .collect();

    let english_legal_terms: HashSet<&str> = [
        "agreement",
        "mutual",
        "nondisclosure",
        "confidentiality",
        "confidential",
        "section",
        "purpose",
        "information",
        "disclose",
        "proprietary",
        "receiving",
        "party",
        "parties",
        "obligations",
        "standard",
        "care",
        "governing",
        "law",
        "jurisdiction",
        "statutory",
        "invoice",
        "subtotal",
        "amount",
        "commercial",
        "transaction",
        "unauthorized",
        "disclosure",
        "dissemination",
        "corporate",
    ]
    .iter()
    .copied()
    .collect();

    let austrian_variant_markers: HashSet<&str> = [
        "abgb",
        "österreich",
        "wien",
        "bgbl",
        "ogh",
        "vfgh",
        "vwg",
        "notariat",
    ]
    .iter()
    .copied()
    .collect();

    let mut austrian_hits = 0;

    for w in &words {
        if german_stopwords.contains(w) {
            german_score += 2.0;
        }
        if german_legal_terms.contains(w) {
            german_score += 4.0;
        }
        if english_stopwords.contains(w) {
            english_score += 2.0;
        }
        if english_legal_terms.contains(w) {
            english_score += 4.0;
        }
        if austrian_variant_markers.contains(w) {
            austrian_hits += 1;
        }
    }

    let total_score = german_score + english_score;
    let (primary_language, confidence) = if total_score > 0.0 {
        if german_score >= english_score {
            let conf = ((german_score - english_score) / total_score).clamp(0.5, 0.99);
            ("deu".to_string(), conf)
        } else {
            let conf = ((english_score - german_score) / total_score).clamp(0.5, 0.99);
            ("eng".to_string(), conf)
        }
    } else {
        ("eng".to_string(), 0.5)
    };

    let is_multilingual = if total_score > 8.0 {
        let min_ratio = (german_score.min(english_score)) / total_score;
        min_ratio >= 0.22
    } else {
        false
    };

    let variant_hint = if primary_language == "deu" {
        if austrian_hits >= 1 {
            Some("de-AT".to_string())
        } else {
            Some("de-DE".to_string())
        }
    } else {
        Some("en-US".to_string())
    };

    LanguageDetectionResult {
        primary_language,
        confidence,
        is_multilingual,
        german_score,
        english_score,
        variant_hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_german_legal_contract_language_detection() {
        let text = "VEREINBARUNG ÜBER VERTRAULICHKEIT UND GEHEIMHALTUNG\n\
                    § 1 Geltungsbereich und vertrauliche Informationen\n\
                    Die Vertragsparteien vereinbaren die strikte Geheimhaltung gemäß den Bestimmungen des ABGB.\n\
                    Erfüllungsort ist Wien, Österreich.";
        let res = detect_language(text);
        assert_eq!(res.primary_language, "deu");
        assert!(
            res.confidence >= 0.75,
            "Expected high German confidence, got {}",
            res.confidence
        );
        assert_eq!(res.variant_hint.as_deref(), Some("de-AT"));
    }

    #[test]
    fn test_english_mutual_nda_language_detection() {
        let text = "MUTUAL NON-DISCLOSURE AND CONFIDENTIALITY AGREEMENT\n\
                    Section 1. Purpose and Confidential Information\n\
                    The parties intend to disclose proprietary technical and financial information strictly for evaluating a commercial transaction.\n\
                    This Agreement shall be governed by governing law and exclusive jurisdiction.";
        let res = detect_language(text);
        assert_eq!(res.primary_language, "eng");
        assert!(
            res.confidence >= 0.75,
            "Expected high English confidence, got {}",
            res.confidence
        );
    }

    #[test]
    fn test_bilingual_invoice_language_detection() {
        let text = "RECHNUNG / COMMERCIAL INVOICE\n\
                    Rechnungsnummer / Invoice Number: RE-2026-7712\n\
                    1. Software Engineering Services / Softwareentwicklung: 2.000,00 EUR\n\
                    Zwischensumme / Subtotal: 3.500,00 EUR\n\
                    20% Umsatzsteuer / 20% VAT: 700,00 EUR\n\
                    Gesamtbetrag / Total Invoice Amount: 4.200,00 EUR";
        let res = detect_language(text);
        assert!(
            res.is_multilingual,
            "Expected bilingual flag to be set for mixed invoice"
        );
    }
}
