//! OASIS OpenDocument Text 1.2 / 1.3 (`.odt`) import and export.

pub mod export;
pub mod export_parts;
pub mod import;
pub mod inline;
pub mod styles;

pub use export::export;
pub use import::import;
