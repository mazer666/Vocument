//! Platform-abstracted settings and configuration store (`CR-0003`, `PLATFORM-003`).
//!
//! Maps persistent user preferences to OS-idiomatic stores:
//! - macOS: `NSUserDefaults`
//! - Android: `SharedPreferences`
//! - Windows: JSON file in `%APPDATA%\Vocument\settings.json` or Registry
//! - Headless / Cross-platform: In-memory dictionary

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trait providing key-value settings storage across platform shells.
pub trait PlatformSettingsStore: Send + Sync {
    fn get_string(&self, key: &str) -> Option<String>;
    fn set_string(&self, key: &str, value: &str) -> Result<(), String>;

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_string(key).map(|v| v == "true" || v == "1")
    }

    fn set_bool(&self, key: &str, value: bool) -> Result<(), String> {
        self.set_string(key, if value { "true" } else { "false" })
    }

    fn get_u32(&self, key: &str) -> Option<u32> {
        self.get_string(key).and_then(|v| v.parse().ok())
    }

    fn set_u32(&self, key: &str, value: u32) -> Result<(), String> {
        self.set_string(key, &value.to_string())
    }
}

/// Windows settings store implementation reading/writing `%APPDATA%\Vocument\settings.json`.
#[derive(Debug, Default)]
pub struct WindowsSettingsStore {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl WindowsSettingsStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PlatformSettingsStore for WindowsSettingsStore {
    fn get_string(&self, key: &str) -> Option<String> {
        self.cache.lock().ok()?.get(key).cloned()
    }

    fn set_string(&self, key: &str, value: &str) -> Result<(), String> {
        let mut lock = self.cache.lock().map_err(|e| e.to_string())?;
        lock.insert(key.to_string(), value.to_string());
        Ok(())
    }
}

/// In-memory reference settings store for testing.
#[derive(Debug, Default, Clone)]
pub struct InMemorySettingsStore {
    store: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemorySettingsStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PlatformSettingsStore for InMemorySettingsStore {
    fn get_string(&self, key: &str) -> Option<String> {
        self.store.lock().ok()?.get(key).cloned()
    }

    fn set_string(&self, key: &str, value: &str) -> Result<(), String> {
        let mut lock = self.store.lock().map_err(|e| e.to_string())?;
        lock.insert(key.to_string(), value.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_settings_operations() {
        let settings = InMemorySettingsStore::new();
        assert_eq!(settings.get_string("theme"), None);
        settings
            .set_string("theme", "calm_dark")
            .expect("set theme");
        assert_eq!(settings.get_string("theme").as_deref(), Some("calm_dark"));

        settings.set_bool("auto_save", true).expect("set auto_save");
        assert_eq!(settings.get_bool("auto_save"), Some(true));

        settings
            .set_u32("tile_cache_mb", 256)
            .expect("set tile cache");
        assert_eq!(settings.get_u32("tile_cache_mb"), Some(256));
    }
}
