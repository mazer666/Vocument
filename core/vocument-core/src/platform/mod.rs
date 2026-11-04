//! Platform abstraction layer (`CR-0003`, `PLATFORM-003`).
//!
//! Provides OS-independent traits for file brokering, persistent settings, application lifecycle,
//! and font enumeration across macOS, Android, Windows, and Linux.

pub mod broker;
pub mod fonts;
pub mod lifecycle;
pub mod settings;

pub use broker::{InMemoryFileBroker, PlatformFileBroker, WindowsFileBroker};
pub use fonts::{BundledFontProvider, PlatformFontProvider, WindowsFontProvider};
pub use lifecycle::{AppLifecycleState, GenericAppLifecycle, PlatformAppLifecycle};
pub use settings::{InMemorySettingsStore, PlatformSettingsStore, WindowsSettingsStore};
