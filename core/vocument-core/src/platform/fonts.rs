//! Platform font enumeration and discovery (`CR-0003`, `PLATFORM-003`).
//!
//! Enumerates system and bundled OpenType/TrueType fonts for deterministic pagination
//! and layout consistency across macOS, Windows, Android, and Linux.

use std::sync::{Arc, Mutex};

/// Trait providing access to platform font catalogs and raw OpenType font bytes.
pub trait PlatformFontProvider: Send + Sync {
    /// Enumerates names of available typeface families on the platform.
    fn available_families(&self) -> Vec<String>;

    /// Returns the raw OpenType font bytes for the specified typeface family, if available.
    fn load_font_data(&self, family: &str) -> Option<Vec<u8>>;
}

/// Windows platform font provider searching `C:\Windows\Fonts` or DirectWrite caches.
#[derive(Debug, Default)]
pub struct WindowsFontProvider {
    custom_fonts: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
}

impl WindowsFontProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_font(&self, family: impl Into<String>, data: Vec<u8>) {
        if let Ok(mut lock) = self.custom_fonts.lock() {
            lock.insert(family.into(), data);
        }
    }
}

impl PlatformFontProvider for WindowsFontProvider {
    fn available_families(&self) -> Vec<String> {
        let mut families = vec![
            "Segoe UI".to_string(),
            "Calibri".to_string(),
            "Arial".to_string(),
            "Times New Roman".to_string(),
            "Consolas".to_string(),
        ];
        if let Ok(lock) = self.custom_fonts.lock() {
            for k in lock.keys() {
                if !families.contains(k) {
                    families.push(k.clone());
                }
            }
        }
        families
    }

    fn load_font_data(&self, family: &str) -> Option<Vec<u8>> {
        if let Ok(lock) = self.custom_fonts.lock()
            && let Some(data) = lock.get(family)
        {
            return Some(data.clone());
        }
        // Check Windows Fonts folder when running on Windows host
        #[cfg(target_os = "windows")]
        {
            let win_dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
            let font_dir = std::path::Path::new(&win_dir).join("Fonts");
            let candidates = [
                format!("{family}.ttf"),
                format!("{family}.otf"),
                format!("{family} Regular.ttf"),
            ];
            for candidate in candidates {
                let path = font_dir.join(candidate);
                if path.exists() {
                    if let Ok(bytes) = std::fs::read(path) {
                        return Some(bytes);
                    }
                }
            }
        }
        None
    }
}

/// Fallback font provider supplying bundled fonts (e.g. Lato).
#[derive(Debug, Default, Clone)]
pub struct BundledFontProvider;

impl PlatformFontProvider for BundledFontProvider {
    fn available_families(&self) -> Vec<String> {
        vec!["Lato".to_string(), "Droid Sans Fallback".to_string()]
    }

    fn load_font_data(&self, family: &str) -> Option<Vec<u8>> {
        if family.eq_ignore_ascii_case("lato") {
            Some(
                include_bytes!("../../../../core/vocument-layout/assets/lato_regular.ttf").to_vec(),
            )
        } else if family.eq_ignore_ascii_case("droid sans fallback") {
            Some(
                include_bytes!("../../../../core/vocument-layout/assets/droid_sans_fallback.ttf")
                    .to_vec(),
            )
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_provider_enumeration() {
        let provider = BundledFontProvider;
        let families = provider.available_families();
        assert!(families.contains(&"Lato".to_string()));

        let lato_data = provider.load_font_data("Lato").expect("load lato bytes");
        assert!(!lato_data.is_empty());

        let win_provider = WindowsFontProvider::new();
        win_provider.register_font("CustomFont", vec![1, 2, 3]);
        assert!(
            win_provider
                .available_families()
                .contains(&"CustomFont".to_string())
        );
        assert_eq!(
            win_provider.load_font_data("CustomFont"),
            Some(vec![1, 2, 3])
        );
    }
}
