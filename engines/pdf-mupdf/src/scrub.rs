//! Removing the redacted words from the places of a document that do not lie
//! on the page, and finding the places that still carry them
//! (`REDACT-002`).
//!
//! The redaction of MuPDF removes the content of a page. The text can survive
//! in other places: in the text of an annotation, in the value of a form
//! field, in the metadata, in the XMP packet, in the content of an embedded
//! file. A redaction that reports success while the text is still in the
//! document is a lie.
//!
//! The adapter offers two things:
//!
//! 1. `scrub_words` removes the words from the places where that can be done
//!    without destroying the document.
//! 2. `find_word_occurrences` reports every place that still carries a word
//!    — as plain text, as UTF-16 or as the hex string of the UTF-16 bytes.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar};

use vocument_core::pdf_engine::{PdfEngineError, PdfScrubReport, PdfTextOccurrence};

/// The maximum number of reported places.
pub const MAX_OCCURRENCES: usize = 200;

unsafe extern "C" {
    fn vocument_mupdf_scrub_document(
        bytes: *const c_uchar,
        len: usize,
        words: *const *const c_char,
        word_count: usize,
        out_pdf_bytes: *mut *mut c_uchar,
        out_pdf_len: *mut usize,
        out_scrubbed_annotations: *mut c_int,
        out_scrubbed_form_values: *mut c_int,
        out_scrubbed_metadata: *mut c_int,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_find_text_occurrences(
        bytes: *const c_uchar,
        len: usize,
        words: *const *const c_char,
        word_count: usize,
        out_buf: *mut c_char,
        out_cap: usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_free_bytes(ptr: *mut c_uchar);
}

/// Reads a NUL-terminated string of the C bridge.
fn read_string(buffer: &[c_char]) -> String {
    // SAFETY: the C bridge writes NUL-terminated strings into the array.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// Prepares the words for the C bridge.
///
/// The returned vector owns the strings, the pointers into it stay valid as
/// long as the vector is not moved.
fn prepare_words(words: &[String]) -> Result<Vec<CString>, PdfEngineError> {
    words
        .iter()
        .map(|word| {
            CString::new(word.as_str()).map_err(|_| PdfEngineError::EngineFailure {
                details: "a redacted word contains a NUL byte".to_owned(),
            })
        })
        .collect()
}

/// Removes the words from the annotations, the form fields and the metadata
/// of the document and returns the new bytes of the document.
pub fn scrub_words(
    pdf_bytes: &[u8],
    words: &[String],
) -> Result<(Vec<u8>, PdfScrubReport), PdfEngineError> {
    let word_list = prepare_words(words)?;
    let word_pointers: Vec<*const c_char> = word_list.iter().map(|word| word.as_ptr()).collect();

    let mut out_bytes: *mut c_uchar = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut annotations: c_int = 0;
    let mut form_values: c_int = 0;
    let mut metadata: c_int = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: the pointers refer to the strings of `word_list`, the output
    // pointers are valid, the buffer of the document is valid.
    let status = unsafe {
        vocument_mupdf_scrub_document(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            word_pointers.as_ptr(),
            word_pointers.len(),
            &raw mut out_bytes,
            &raw mut out_len,
            &raw mut annotations,
            &raw mut form_values,
            &raw mut metadata,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = read_string(&err_buf);
        return Err(PdfEngineError::EngineFailure {
            details: if details.is_empty() {
                format!("MuPDF failed to scrub the document (status {status})")
            } else {
                details
            },
        });
    }

    if out_bytes.is_null() {
        return Err(PdfEngineError::EngineFailure {
            details: "MuPDF did not return a document".to_owned(),
        });
    }

    // SAFETY: the bridge allocated the buffer with `malloc` and reports its
    // length; the copy takes ownership of the bytes.
    let scrubbed = unsafe { std::slice::from_raw_parts(out_bytes, out_len) }.to_vec();
    // SAFETY: the buffer was allocated by the bridge with `malloc`.
    unsafe { vocument_mupdf_free_bytes(out_bytes) };

    Ok((
        scrubbed,
        PdfScrubReport {
            scrubbed_annotations: annotations.max(0) as u32,
            scrubbed_form_values: form_values.max(0) as u32,
            scrubbed_metadata_fields: metadata.max(0) as u32,
        },
    ))
}

/// Reports the places of the document that still carry one of the words.
pub fn find_word_occurrences(
    pdf_bytes: &[u8],
    words: &[String],
) -> Result<Vec<PdfTextOccurrence>, PdfEngineError> {
    let word_list = prepare_words(words)?;
    let word_pointers: Vec<*const c_char> = word_list.iter().map(|word| word.as_ptr()).collect();

    let mut report = vec![0 as c_char; 16 * 1024];
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: the pointers refer to the strings of `word_list`, the report
    // buffer has room for the lines, the buffer of the document is valid.
    let status = unsafe {
        vocument_mupdf_find_text_occurrences(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            word_pointers.as_ptr(),
            word_pointers.len(),
            report.as_mut_ptr(),
            report.len(),
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = read_string(&err_buf);
        return Err(PdfEngineError::EngineFailure {
            details: if details.is_empty() {
                format!("MuPDF failed to search the document (status {status})")
            } else {
                details
            },
        });
    }

    let text = read_string(&report);
    let mut occurrences = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let kind = parts.next().unwrap_or_default().to_owned();
        let page_index = parts
            .next()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or(-1);
        let location = parts.next().unwrap_or_default().to_owned();
        if kind.is_empty() {
            continue;
        }
        occurrences.push(PdfTextOccurrence {
            kind,
            page_index,
            location,
        });
    }
    Ok(occurrences)
}
