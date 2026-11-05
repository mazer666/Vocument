//! Arabic contextual shaping, ligatures, and bidirectional visual ordering (`M4.4`, `CR-0005`).
//!
//! Provides genuine OpenType glyph shaping for Arabic cursive script:
//! - Contextual analysis (Isolated, Initial, Medial, Final forms).
//! - Dual-joining, right-joining, and transparent combining character handling.
//! - Lam-Alef ligature synthesis (`لا`, `لآ`, `لأ`, `لإ`).
//! - Right-to-Left visual reordering for horizontal PDF text matrix emission.

/// Position / contextual form of an Arabic letter in a cursive word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArabicForm {
    Isolated,
    Initial,
    Medial,
    Final,
}

/// Joining type classification according to Unicode Standard §9.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoiningType {
    NonJoining,
    RightJoining,
    DualJoining,
    Transparent,
}

fn joining_type(c: char) -> JoiningType {
    match c {
        // Transparent combining marks (harakat / tashkeel)
        '\u{064B}'..='\u{065F}' | '\u{0670}' => JoiningType::Transparent,

        // Right-joining only: can join to the right (preceding character), cannot join to succeeding
        '\u{0622}' // Alef with madda
        | '\u{0623}' // Alef with hamza above
        | '\u{0624}' // Waw with hamza
        | '\u{0625}' // Alef with hamza below
        | '\u{0627}' // Alef
        | '\u{062F}' // Dal
        | '\u{0630}' // Thal
        | '\u{0631}' // Reh
        | '\u{0632}' // Zain
        | '\u{0648}' // Waw
        | '\u{0649}' // Alef Maksura (often right-joining in standard Arabic)
        | '\u{0671}' // Alef Wasla
        | '\u{0672}' | '\u{0673}' | '\u{0675}'
        | '\u{0688}'..='\u{0699}' => JoiningType::RightJoining,

        // Dual-joining: can join both preceding and succeeding characters
        '\u{0626}' // Yeh with hamza
        | '\u{0628}' // Beh
        | '\u{0629}' // Teh Marbuta (acts as dual in some contexts or right-joining)
        | '\u{062A}' // Teh
        | '\u{062B}' // Theh
        | '\u{062C}' // Jeem
        | '\u{062D}' // Hah
        | '\u{062E}' // Kheh
        | '\u{0633}' // Seen
        | '\u{0634}' // Sheen
        | '\u{0635}' // Sad
        | '\u{0636}' // Dad
        | '\u{0637}' // Tah
        | '\u{0638}' // Zah
        | '\u{0639}' // Ain
        | '\u{063A}' // Ghain
        | '\u{0640}' // Tatweel
        | '\u{0641}' // Feh
        | '\u{0642}' // Qaf
        | '\u{0643}' // Kaf
        | '\u{0644}' // Lam
        | '\u{0645}' // Meem
        | '\u{0646}' // Noon
        | '\u{0647}' // Heh
        | '\u{064A}' // Yeh
        | '\u{067E}' | '\u{0686}' | '\u{06A9}' | '\u{06AF}' | '\u{06CC}' => JoiningType::DualJoining,

        _ => JoiningType::NonJoining,
    }
}

/// Maps a nominal Arabic Unicode character and its contextual form to the Presentation Form scalar.
pub fn shape_char(c: char, form: ArabicForm) -> char {
    match (c, form) {
        // Alef with madda
        ('\u{0622}', ArabicForm::Isolated) => '\u{FE81}',
        ('\u{0622}', ArabicForm::Final) => '\u{FE82}',
        ('\u{0622}', _) => '\u{FE81}',

        // Alef with hamza above
        ('\u{0623}', ArabicForm::Isolated) => '\u{FE83}',
        ('\u{0623}', ArabicForm::Final) => '\u{FE84}',
        ('\u{0623}', _) => '\u{FE83}',

        // Waw with hamza
        ('\u{0624}', ArabicForm::Isolated) => '\u{FE85}',
        ('\u{0624}', ArabicForm::Final) => '\u{FE86}',
        ('\u{0624}', _) => '\u{FE85}',

        // Alef with hamza below
        ('\u{0625}', ArabicForm::Isolated) => '\u{FE87}',
        ('\u{0625}', ArabicForm::Final) => '\u{FE88}',
        ('\u{0625}', _) => '\u{FE87}',

        // Yeh with hamza
        ('\u{0626}', ArabicForm::Isolated) => '\u{FE89}',
        ('\u{0626}', ArabicForm::Final) => '\u{FE8A}',
        ('\u{0626}', ArabicForm::Initial) => '\u{FE8B}',
        ('\u{0626}', ArabicForm::Medial) => '\u{FE8C}',

        // Alef
        ('\u{0627}', ArabicForm::Isolated) => '\u{FE8D}',
        ('\u{0627}', ArabicForm::Final) => '\u{FE8E}',
        ('\u{0627}', _) => '\u{FE8D}',

        // Beh
        ('\u{0628}', ArabicForm::Isolated) => '\u{FE8F}',
        ('\u{0628}', ArabicForm::Final) => '\u{FE90}',
        ('\u{0628}', ArabicForm::Initial) => '\u{FE91}',
        ('\u{0628}', ArabicForm::Medial) => '\u{FE92}',

        // Teh Marbuta
        ('\u{0629}', ArabicForm::Isolated) => '\u{FE93}',
        ('\u{0629}', ArabicForm::Final) => '\u{FE94}',
        ('\u{0629}', _) => '\u{FE93}',

        // Teh
        ('\u{062A}', ArabicForm::Isolated) => '\u{FE95}',
        ('\u{062A}', ArabicForm::Final) => '\u{FE96}',
        ('\u{062A}', ArabicForm::Initial) => '\u{FE97}',
        ('\u{062A}', ArabicForm::Medial) => '\u{FE98}',

        // Theh
        ('\u{062B}', ArabicForm::Isolated) => '\u{FE99}',
        ('\u{062B}', ArabicForm::Final) => '\u{FE9A}',
        ('\u{062B}', ArabicForm::Initial) => '\u{FE9B}',
        ('\u{062B}', ArabicForm::Medial) => '\u{FE9C}',

        // Jeem
        ('\u{062C}', ArabicForm::Isolated) => '\u{FE9D}',
        ('\u{062C}', ArabicForm::Final) => '\u{FE9E}',
        ('\u{062C}', ArabicForm::Initial) => '\u{FE9F}',
        ('\u{062C}', ArabicForm::Medial) => '\u{FEA0}',

        // Hah
        ('\u{062D}', ArabicForm::Isolated) => '\u{FEA1}',
        ('\u{062D}', ArabicForm::Final) => '\u{FEA2}',
        ('\u{062D}', ArabicForm::Initial) => '\u{FEA3}',
        ('\u{062D}', ArabicForm::Medial) => '\u{FEA4}',

        // Kheh
        ('\u{062E}', ArabicForm::Isolated) => '\u{FEA5}',
        ('\u{062E}', ArabicForm::Final) => '\u{FEA6}',
        ('\u{062E}', ArabicForm::Initial) => '\u{FEA7}',
        ('\u{062E}', ArabicForm::Medial) => '\u{FEA8}',

        // Dal
        ('\u{062F}', ArabicForm::Isolated) => '\u{FEA9}',
        ('\u{062F}', ArabicForm::Final) => '\u{FEAA}',
        ('\u{062F}', _) => '\u{FEA9}',

        // Thal
        ('\u{0630}', ArabicForm::Isolated) => '\u{FEAB}',
        ('\u{0630}', ArabicForm::Final) => '\u{FEAC}',
        ('\u{0630}', _) => '\u{FEAB}',

        // Reh
        ('\u{0631}', ArabicForm::Isolated) => '\u{FEAD}',
        ('\u{0631}', ArabicForm::Final) => '\u{FEAE}',
        ('\u{0631}', _) => '\u{FEAD}',

        // Zain
        ('\u{0632}', ArabicForm::Isolated) => '\u{FEAF}',
        ('\u{0632}', ArabicForm::Final) => '\u{FEB0}',
        ('\u{0632}', _) => '\u{FEAF}',

        // Seen
        ('\u{0633}', ArabicForm::Isolated) => '\u{FEB1}',
        ('\u{0633}', ArabicForm::Final) => '\u{FEB2}',
        ('\u{0633}', ArabicForm::Initial) => '\u{FEB3}',
        ('\u{0633}', ArabicForm::Medial) => '\u{FEB4}',

        // Sheen
        ('\u{0634}', ArabicForm::Isolated) => '\u{FEB5}',
        ('\u{0634}', ArabicForm::Final) => '\u{FEB6}',
        ('\u{0634}', ArabicForm::Initial) => '\u{FEB7}',
        ('\u{0634}', ArabicForm::Medial) => '\u{FEB8}',

        // Sad
        ('\u{0635}', ArabicForm::Isolated) => '\u{FEB9}',
        ('\u{0635}', ArabicForm::Final) => '\u{FEBA}',
        ('\u{0635}', ArabicForm::Initial) => '\u{FEBB}',
        ('\u{0635}', ArabicForm::Medial) => '\u{FEBC}',

        // Dad
        ('\u{0636}', ArabicForm::Isolated) => '\u{FEBD}',
        ('\u{0636}', ArabicForm::Final) => '\u{FEBE}',
        ('\u{0636}', ArabicForm::Initial) => '\u{FEBF}',
        ('\u{0636}', ArabicForm::Medial) => '\u{FEC0}',

        // Tah
        ('\u{0637}', ArabicForm::Isolated) => '\u{FEC1}',
        ('\u{0637}', ArabicForm::Final) => '\u{FEC2}',
        ('\u{0637}', ArabicForm::Initial) => '\u{FEC3}',
        ('\u{0637}', ArabicForm::Medial) => '\u{FEC4}',

        // Zah
        ('\u{0638}', ArabicForm::Isolated) => '\u{FEC5}',
        ('\u{0638}', ArabicForm::Final) => '\u{FEC6}',
        ('\u{0638}', ArabicForm::Initial) => '\u{FEC7}',
        ('\u{0638}', ArabicForm::Medial) => '\u{FEC8}',

        // Ain
        ('\u{0639}', ArabicForm::Isolated) => '\u{FEC9}',
        ('\u{0639}', ArabicForm::Final) => '\u{FECA}',
        ('\u{0639}', ArabicForm::Initial) => '\u{FECB}',
        ('\u{0639}', ArabicForm::Medial) => '\u{FECC}',

        // Ghain
        ('\u{063A}', ArabicForm::Isolated) => '\u{FECD}',
        ('\u{063A}', ArabicForm::Final) => '\u{FECE}',
        ('\u{063A}', ArabicForm::Initial) => '\u{FECF}',
        ('\u{063A}', ArabicForm::Medial) => '\u{FED0}',

        // Feh
        ('\u{0641}', ArabicForm::Isolated) => '\u{FED1}',
        ('\u{0641}', ArabicForm::Final) => '\u{FED2}',
        ('\u{0641}', ArabicForm::Initial) => '\u{FED3}',
        ('\u{0641}', ArabicForm::Medial) => '\u{FED4}',

        // Qaf
        ('\u{0642}', ArabicForm::Isolated) => '\u{FED5}',
        ('\u{0642}', ArabicForm::Final) => '\u{FED6}',
        ('\u{0642}', ArabicForm::Initial) => '\u{FED7}',
        ('\u{0642}', ArabicForm::Medial) => '\u{FED8}',

        // Kaf
        ('\u{0643}', ArabicForm::Isolated) => '\u{FED9}',
        ('\u{0643}', ArabicForm::Final) => '\u{FEDA}',
        ('\u{0643}', ArabicForm::Initial) => '\u{FEDB}',
        ('\u{0643}', ArabicForm::Medial) => '\u{FEDC}',

        // Lam
        ('\u{0644}', ArabicForm::Isolated) => '\u{FEDD}',
        ('\u{0644}', ArabicForm::Final) => '\u{FEDE}',
        ('\u{0644}', ArabicForm::Initial) => '\u{FEDF}',
        ('\u{0644}', ArabicForm::Medial) => '\u{FEE0}',

        // Meem
        ('\u{0645}', ArabicForm::Isolated) => '\u{FEE1}',
        ('\u{0645}', ArabicForm::Final) => '\u{FEE2}',
        ('\u{0645}', ArabicForm::Initial) => '\u{FEE3}',
        ('\u{0645}', ArabicForm::Medial) => '\u{FEE4}',

        // Noon
        ('\u{0646}', ArabicForm::Isolated) => '\u{FEE5}',
        ('\u{0646}', ArabicForm::Final) => '\u{FEE6}',
        ('\u{0646}', ArabicForm::Initial) => '\u{FEE7}',
        ('\u{0646}', ArabicForm::Medial) => '\u{FEE8}',

        // Heh
        ('\u{0647}', ArabicForm::Isolated) => '\u{FEE9}',
        ('\u{0647}', ArabicForm::Final) => '\u{FEEA}',
        ('\u{0647}', ArabicForm::Initial) => '\u{FEEB}',
        ('\u{0647}', ArabicForm::Medial) => '\u{FEEC}',

        // Waw
        ('\u{0648}', ArabicForm::Isolated) => '\u{FEED}',
        ('\u{0648}', ArabicForm::Final) => '\u{FEEE}',
        ('\u{0648}', _) => '\u{FEED}',

        // Alef Maksura
        ('\u{0649}', ArabicForm::Isolated) => '\u{FEEF}',
        ('\u{0649}', ArabicForm::Final) => '\u{FEF0}',
        ('\u{0649}', _) => '\u{FEEF}',

        // Yeh
        ('\u{064A}', ArabicForm::Isolated) => '\u{FEF1}',
        ('\u{064A}', ArabicForm::Final) => '\u{FEF2}',
        ('\u{064A}', ArabicForm::Initial) => '\u{FEF3}',
        ('\u{064A}', ArabicForm::Medial) => '\u{FEF4}',

        (other, _) => other,
    }
}

/// Shapes an entire Arabic word or segment into contextual presentation forms with Lam-Alef ligatures.
pub fn shape_arabic_text(text: &str) -> Vec<char> {
    let raw_chars: Vec<char> = text.chars().collect();
    if raw_chars.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(raw_chars.len());
    let mut i = 0;

    while i < raw_chars.len() {
        let ch = raw_chars[i];
        let jtype = joining_type(ch);

        if jtype == JoiningType::Transparent || jtype == JoiningType::NonJoining {
            result.push(ch);
            i += 1;
            continue;
        }

        // Check for Lam-Alef ligatures
        if ch == '\u{0644}' {
            // Find next non-transparent character
            let mut next_k = i + 1;
            while next_k < raw_chars.len()
                && joining_type(raw_chars[next_k]) == JoiningType::Transparent
            {
                next_k += 1;
            }
            if next_k < raw_chars.len() {
                let next_ch = raw_chars[next_k];
                let is_prev_joined = find_prev_joining(&raw_chars, i);
                let lig = match next_ch {
                    '\u{0622}' => Some(if is_prev_joined {
                        '\u{FEF6}'
                    } else {
                        '\u{FEF5}'
                    }),
                    '\u{0623}' => Some(if is_prev_joined {
                        '\u{FEF8}'
                    } else {
                        '\u{FEF7}'
                    }),
                    '\u{0625}' => Some(if is_prev_joined {
                        '\u{FEFA}'
                    } else {
                        '\u{FEF9}'
                    }),
                    '\u{0627}' => Some(if is_prev_joined {
                        '\u{FEFC}'
                    } else {
                        '\u{FEFB}'
                    }),
                    _ => None,
                };
                if let Some(lig_char) = lig {
                    result.push(lig_char);
                    // Copy any transparent marks between i and next_k
                    for &tm in &raw_chars[(i + 1)..next_k] {
                        result.push(tm);
                    }
                    i = next_k + 1;
                    continue;
                }
            }
        }

        let is_prev_joined = find_prev_joining(&raw_chars, i);
        let is_next_joined = find_next_joining(&raw_chars, i);

        let form = match (is_prev_joined, is_next_joined) {
            (true, true) if jtype == JoiningType::DualJoining => ArabicForm::Medial,
            (true, _) => ArabicForm::Final,
            (_, true) if jtype == JoiningType::DualJoining => ArabicForm::Initial,
            _ => ArabicForm::Isolated,
        };

        result.push(shape_char(ch, form));
        i += 1;
    }

    result
}

fn find_prev_joining(chars: &[char], idx: usize) -> bool {
    let mut k = idx;
    while k > 0 {
        k -= 1;
        let jt = joining_type(chars[k]);
        if jt == JoiningType::Transparent {
            continue;
        }
        return jt == JoiningType::DualJoining;
    }
    false
}

fn find_next_joining(chars: &[char], idx: usize) -> bool {
    let mut k = idx + 1;
    while k < chars.len() {
        let jt = joining_type(chars[k]);
        if jt == JoiningType::Transparent {
            k += 1;
            continue;
        }
        return jt == JoiningType::DualJoining || jt == JoiningType::RightJoining;
    }
    false
}

/// Reorders shaped Arabic/RTL segments for horizontal Left-to-Right PDF text matrix emission.
pub fn reorder_rtl_for_pdf(shaped_chars: &[char]) -> Vec<char> {
    let mut output = Vec::with_capacity(shaped_chars.len());
    let mut arabic_run = Vec::new();

    for &c in shaped_chars {
        if is_arabic_or_bidi(c) {
            arabic_run.push(c);
        } else {
            if !arabic_run.is_empty() {
                arabic_run.reverse();
                output.append(&mut arabic_run);
            }
            output.push(c);
        }
    }

    if !arabic_run.is_empty() {
        arabic_run.reverse();
        output.append(&mut arabic_run);
    }

    output
}

fn is_arabic_or_bidi(c: char) -> bool {
    matches!(c,
        '\u{0600}'..='\u{06FF}' |
        '\u{0750}'..='\u{077F}' |
        '\u{08A0}'..='\u{08FF}' |
        '\u{FB50}'..='\u{FDFF}' |
        '\u{FE70}'..='\u{FEFF}'
    )
}
