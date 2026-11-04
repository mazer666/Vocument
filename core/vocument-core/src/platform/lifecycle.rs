//! Platform application lifecycle monitoring (`CR-0003`, `PLATFORM-003`).
//!
//! Reports foreground/background transitions, memory pressure, and termination requests
//! to ensure background worker jobs and transactions suspend or checkpoint safely.

use std::sync::{Arc, Mutex};

/// Operational state of the host platform application shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppLifecycleState {
    #[default]
    Active,
    Inactive,
    Background,
    Suspended,
    Terminating,
}

/// Trait reporting host app lifecycle events across native UI shells.
pub trait PlatformAppLifecycle: Send + Sync {
    /// Returns the current lifecycle state.
    fn current_state(&self) -> AppLifecycleState;

    /// Transitions to a new lifecycle state.
    fn set_state(&self, state: AppLifecycleState);

    /// Returns true if background processing should pause due to system constraints.
    fn should_pause_background_jobs(&self) -> bool {
        matches!(
            self.current_state(),
            AppLifecycleState::Background
                | AppLifecycleState::Suspended
                | AppLifecycleState::Terminating
        )
    }
}

/// Generic / Windows application lifecycle controller.
#[derive(Debug, Default)]
pub struct GenericAppLifecycle {
    state: Arc<Mutex<AppLifecycleState>>,
}

impl GenericAppLifecycle {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PlatformAppLifecycle for GenericAppLifecycle {
    fn current_state(&self) -> AppLifecycleState {
        self.state.lock().map(|s| *s).unwrap_or_default()
    }

    fn set_state(&self, state: AppLifecycleState) {
        if let Ok(mut lock) = self.state.lock() {
            *lock = state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_transitions() {
        let lifecycle = GenericAppLifecycle::new();
        assert_eq!(lifecycle.current_state(), AppLifecycleState::Active);
        assert!(!lifecycle.should_pause_background_jobs());

        lifecycle.set_state(AppLifecycleState::Background);
        assert_eq!(lifecycle.current_state(), AppLifecycleState::Background);
        assert!(lifecycle.should_pause_background_jobs());

        lifecycle.set_state(AppLifecycleState::Active);
        assert!(!lifecycle.should_pause_background_jobs());
    }
}
