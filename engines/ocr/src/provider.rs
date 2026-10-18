//! `OcrProvider` abstraction interface and errors (`ARCHITECTURE.md` §20).

use thiserror::Error;
use vocument_imaging::ScanFrame;

use crate::models::{OcrOptions, OcrPageResult};

/// Errors that can occur during optical character recognition.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OcrError {
    #[error("OCR engine initialization failed: {details}")]
    EngineInitFailed { details: String },

    #[error("failed to configure image buffer for OCR: {details}")]
    ImageSetupFailed { details: String },

    #[error("OCR recognition process failed: {details}")]
    RecognitionFailed { details: String },

    #[error("requested language '{lang}' is not available in local tessdata")]
    LanguageNotAvailable { lang: String },

    #[error("scan frame contains zero dimensions or empty pixel buffer")]
    EmptyFrame,

    #[error("test corpus fixture not found: {path}")]
    CorpusNotFound { path: String },
}

/// Abstract provider interface for optical character recognition engines (`ARCHITECTURE.md` §20).
pub trait OcrProvider: Send + Sync {
    /// Unique identifier of the OCR provider implementation.
    fn provider_id(&self) -> &'static str;

    /// List of available/supported languages in local installation.
    fn supported_languages(&self) -> Vec<String>;

    /// Executes OCR recognition over a raw or cleaned `ScanFrame`.
    fn recognize_frame(
        &self,
        frame: &ScanFrame,
        options: &OcrOptions,
    ) -> Result<OcrPageResult, OcrError>;
}
