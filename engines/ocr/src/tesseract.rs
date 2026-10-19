//! Tesseract OCR adapter implementing `OcrProvider` via C bridge (`ARCHITECTURE.md` §20).

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};

use vocument_imaging::{PixelFormat, ScanFrame};

use crate::language::detect_language;
use crate::models::{
    BoundingBox, OcrBlock, OcrBlockType, OcrCharacter, OcrLine, OcrOptions, OcrPageResult, OcrWord,
};
use crate::provider::{OcrError, OcrProvider};

type WordCallback = extern "C" fn(
    text: *const c_char,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
    conf: f32,
    block: c_int,
    line: c_int,
    word: c_int,
    user_data: *mut c_void,
);

type SymbolCallback = extern "C" fn(
    text: *const c_char,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
    conf: f32,
    block: c_int,
    line: c_int,
    word: c_int,
    user_data: *mut c_void,
);

unsafe extern "C" {
    fn vocument_ocr_create(datapath: *const c_char, languages: *const c_char) -> *mut c_void;
    fn vocument_ocr_set_image(
        handle: *mut c_void,
        pixels: *const u8,
        width: c_int,
        height: c_int,
        bytes_per_pixel: c_int,
        bytes_per_line: c_int,
    ) -> c_int;
    fn vocument_ocr_recognize(handle: *mut c_void) -> c_int;
    fn vocument_ocr_get_text(handle: *mut c_void) -> *mut c_char;
    fn vocument_ocr_get_mean_confidence(handle: *mut c_void) -> c_int;
    fn vocument_ocr_get_hocr(handle: *mut c_void, page_num: c_int) -> *mut c_char;
    fn vocument_ocr_iterate_results(
        handle: *mut c_void,
        word_cb: WordCallback,
        symbol_cb: SymbolCallback,
        user_data: *mut c_void,
    ) -> c_int;
    fn vocument_ocr_free_string(str: *mut c_char);
    fn vocument_ocr_destroy(handle: *mut c_void);
}

/// Native Tesseract OCR engine adapter.
#[derive(Debug, Default)]
pub struct TesseractEngine;

impl TesseractEngine {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the best available local tessdata directory.
    pub fn resolve_tessdata_path(custom: Option<&Path>) -> PathBuf {
        if let Some(p) = custom.filter(|p| p.exists()) {
            return p.to_path_buf();
        }
        if let Ok(prefix) = std::env::var("TESSDATA_PREFIX") {
            let p = PathBuf::from(prefix);
            if p.exists() {
                return p;
            }
        }
        let candidates = [
            PathBuf::from("/data/user/0/org.vocument.app/files/tessdata"),
            PathBuf::from("/data/data/org.vocument.app/files/tessdata"),
            PathBuf::from("/opt/homebrew/share/tessdata"),
            PathBuf::from("apps/android/app/src/main/assets/tessdata"),
            PathBuf::from("../../apps/android/app/src/main/assets/tessdata"),
            PathBuf::from("/usr/local/share/tessdata"),
            PathBuf::from("/usr/share/tessdata"),
            PathBuf::from("tessdata"),
        ];
        for candidate in candidates {
            if candidate.exists() {
                return candidate;
            }
        }
        #[cfg(target_os = "android")]
        {
            PathBuf::from("/data/user/0/org.vocument.app/files/tessdata")
        }
        #[cfg(not(target_os = "android"))]
        {
            let brew = PathBuf::from("/opt/homebrew/share/tessdata");
            if brew.exists() {
                brew
            } else {
                PathBuf::from("/usr/share/tessdata")
            }
        }
    }
}

struct RawWord {
    text: String,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    conf: f32,
    block_num: i32,
    line_num: i32,
    word_num: i32,
}

struct RawSymbol {
    text: String,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    conf: f32,
    block_num: i32,
    line_num: i32,
    word_num: i32,
}

struct CollectorContext {
    words: Vec<RawWord>,
    symbols: Vec<RawSymbol>,
}

extern "C" fn on_word_trampoline(
    text: *const c_char,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
    conf: f32,
    block: c_int,
    line: c_int,
    word: c_int,
    user_data: *mut c_void,
) {
    if text.is_null() || user_data.is_null() {
        return;
    }
    let ctx = unsafe { &mut *(user_data as *mut CollectorContext) };
    let c_str = unsafe { CStr::from_ptr(text) };
    if let Ok(s) = c_str.to_str() {
        let clean = s.trim();
        if !clean.is_empty() {
            ctx.words.push(RawWord {
                text: clean.to_string(),
                x0,
                y0,
                x1,
                y1,
                conf,
                block_num: block,
                line_num: line,
                word_num: word,
            });
        }
    }
}

extern "C" fn on_symbol_trampoline(
    text: *const c_char,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
    conf: f32,
    block: c_int,
    line: c_int,
    word: c_int,
    user_data: *mut c_void,
) {
    if text.is_null() || user_data.is_null() {
        return;
    }
    let ctx = unsafe { &mut *(user_data as *mut CollectorContext) };
    let c_str = unsafe { CStr::from_ptr(text) };
    if let Ok(s) = c_str.to_str() {
        ctx.symbols.push(RawSymbol {
            text: s.to_string(),
            x0,
            y0,
            x1,
            y1,
            conf,
            block_num: block,
            line_num: line,
            word_num: word,
        });
    }
}

impl OcrProvider for TesseractEngine {
    fn provider_id(&self) -> &'static str {
        "tesseract"
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["deu".into(), "eng".into(), "deu+eng".into()]
    }

    fn recognize_frame(
        &self,
        frame: &ScanFrame,
        options: &OcrOptions,
    ) -> Result<OcrPageResult, OcrError> {
        if frame.width == 0 || frame.height == 0 || frame.pixels.is_empty() {
            return Err(OcrError::EmptyFrame);
        }

        let tessdata_dir = Self::resolve_tessdata_path(options.tessdata_path.as_deref());
        let lang_str = if options.languages.is_empty() {
            "deu+eng".to_string()
        } else {
            options.languages.join("+")
        };

        let c_datapath = CString::new(tessdata_dir.to_string_lossy().as_bytes()).map_err(|e| {
            OcrError::EngineInitFailed {
                details: e.to_string(),
            }
        })?;
        let c_lang = CString::new(lang_str.as_bytes()).map_err(|e| OcrError::EngineInitFailed {
            details: e.to_string(),
        })?;

        let engine = unsafe { vocument_ocr_create(c_datapath.as_ptr(), c_lang.as_ptr()) };
        if engine.is_null() {
            return Err(OcrError::EngineInitFailed {
                details: format!(
                    "TessBaseAPIInit failed for datapath='{}', lang='{}'",
                    tessdata_dir.display(),
                    lang_str
                ),
            });
        }

        // Prepare pixel buffer parameters
        let (pixels_ptr, bpp, bytes_per_line) = match frame.format {
            PixelFormat::Rgba8 => (frame.pixels.as_ptr(), 4, (frame.stride * 4) as i32),
            PixelFormat::Rgb8 => (frame.pixels.as_ptr(), 3, (frame.stride * 3) as i32),
            PixelFormat::Gray8 => (frame.pixels.as_ptr(), 1, frame.stride as i32),
            PixelFormat::Nv21 | PixelFormat::Yuv420 => {
                (frame.pixels.as_ptr(), 1, frame.width as i32)
            }
        };

        let set_res = unsafe {
            vocument_ocr_set_image(
                engine,
                pixels_ptr,
                frame.width as c_int,
                frame.height as c_int,
                bpp,
                bytes_per_line,
            )
        };
        if set_res != 0 {
            unsafe { vocument_ocr_destroy(engine) };
            return Err(OcrError::ImageSetupFailed {
                details: format!("vocument_ocr_set_image failed with code {}", set_res),
            });
        }

        let rec_res = unsafe { vocument_ocr_recognize(engine) };
        if rec_res != 0 {
            unsafe { vocument_ocr_destroy(engine) };
            return Err(OcrError::RecognitionFailed {
                details: format!("vocument_ocr_recognize failed with code {}", rec_res),
            });
        }

        let full_text = unsafe {
            let ptr = vocument_ocr_get_text(engine);
            if ptr.is_null() {
                String::new()
            } else {
                let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                vocument_ocr_free_string(ptr);
                s
            }
        };

        let mean_conf = unsafe { vocument_ocr_get_mean_confidence(engine) } as f32;

        let hocr_output = if options.generate_hocr {
            unsafe {
                let ptr = vocument_ocr_get_hocr(engine, 0);
                if ptr.is_null() {
                    None
                } else {
                    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                    vocument_ocr_free_string(ptr);
                    Some(s)
                }
            }
        } else {
            None
        };

        let mut collector = CollectorContext {
            words: Vec::new(),
            symbols: Vec::new(),
        };

        unsafe {
            vocument_ocr_iterate_results(
                engine,
                on_word_trampoline,
                on_symbol_trampoline,
                &mut collector as *mut CollectorContext as *mut c_void,
            );
            vocument_ocr_destroy(engine);
        }

        // Group symbols by (block_num, line_num, word_num)
        let mut symbols_by_word: HashMap<(i32, i32, i32), Vec<OcrCharacter>> = HashMap::new();
        for s in collector.symbols {
            let key = (s.block_num, s.line_num, s.word_num);
            let bbox = BoundingBox::new(s.x0 as f32, s.y0 as f32, s.x1 as f32, s.y1 as f32);
            symbols_by_word.entry(key).or_default().push(OcrCharacter {
                text: s.text,
                bbox,
                confidence: s.conf,
            });
        }

        // Assemble domain OcrWords
        let mut domain_words: Vec<OcrWord> = Vec::with_capacity(collector.words.len());
        for rw in collector.words {
            let key = (rw.block_num, rw.line_num, rw.word_num);
            let chars = symbols_by_word.remove(&key).unwrap_or_default();
            let bbox = BoundingBox::new(rw.x0 as f32, rw.y0 as f32, rw.x1 as f32, rw.y1 as f32);
            domain_words.push(OcrWord {
                text: rw.text,
                bbox,
                confidence: rw.conf,
                characters: chars,
                block_index: rw.block_num.max(0) as u32,
                line_index: rw.line_num.max(0) as u32,
                word_index: rw.word_num.max(0) as u32,
            });
        }

        // Group words into lines and blocks
        let mut lines_map: HashMap<(u32, u32), Vec<OcrWord>> = HashMap::new();
        for w in &domain_words {
            lines_map
                .entry((w.block_index, w.line_index))
                .or_default()
                .push(w.clone());
        }

        let mut sorted_keys: Vec<(u32, u32)> = lines_map.keys().copied().collect();
        sorted_keys.sort();

        let mut domain_lines: Vec<OcrLine> = Vec::with_capacity(sorted_keys.len());
        for (block_idx, line_idx) in sorted_keys {
            let words = lines_map.remove(&(block_idx, line_idx)).unwrap_or_default();
            if words.is_empty() {
                continue;
            }
            let line_text = words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let avg_conf = words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32;
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = 0.0f32;
            let mut max_y = 0.0f32;
            for w in &words {
                min_x = min_x.min(w.bbox.x0);
                min_y = min_y.min(w.bbox.y0);
                max_x = max_x.max(w.bbox.x1);
                max_y = max_y.max(w.bbox.y1);
            }
            domain_lines.push(OcrLine {
                text: line_text,
                bbox: BoundingBox::new(min_x, min_y, max_x, max_y),
                confidence: avg_conf,
                words,
                line_index: line_idx,
            });
        }

        // Group lines into blocks
        let mut blocks_map: HashMap<u32, Vec<OcrLine>> = HashMap::new();
        for line in &domain_lines {
            let block_id = line.words.first().map(|w| w.block_index).unwrap_or(0);
            blocks_map.entry(block_id).or_default().push(line.clone());
        }
        let mut block_ids: Vec<u32> = blocks_map.keys().copied().collect();
        block_ids.sort();

        let mut domain_blocks: Vec<OcrBlock> = Vec::with_capacity(block_ids.len());
        for bid in block_ids {
            let blines = blocks_map.remove(&bid).unwrap_or_default();
            if blines.is_empty() {
                continue;
            }
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = 0.0f32;
            let mut max_y = 0.0f32;
            for l in &blines {
                min_x = min_x.min(l.bbox.x0);
                min_y = min_y.min(l.bbox.y0);
                max_x = max_x.max(l.bbox.x1);
                max_y = max_y.max(l.bbox.y1);
            }
            domain_blocks.push(OcrBlock {
                block_type: OcrBlockType::Text,
                bbox: BoundingBox::new(min_x, min_y, max_x, max_y),
                lines: blines,
                block_index: bid,
            });
        }

        let detected_language = if options.detect_language {
            detect_language(&full_text).primary_language
        } else {
            options
                .languages
                .first()
                .cloned()
                .unwrap_or_else(|| "deu".into())
        };

        Ok(OcrPageResult {
            page_index: 0,
            full_text,
            blocks: domain_blocks,
            lines: domain_lines,
            words: domain_words,
            mean_confidence: mean_conf,
            detected_language,
            hocr_output,
            frame_width: frame.width,
            frame_height: frame.height,
        })
    }
}
