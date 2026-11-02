//! ECMA-376 / ISO-29500 WordprocessingML (`.docx`) import and export.

pub mod export;
pub mod export_parts;
pub mod import;
pub mod parts;
pub mod props;
pub mod runs;

pub use export::export;
pub use import::import;
