//! Read-only accessors for integration tests.
//!
//! Compiled out of release builds: they exist only under `cfg(test)` or the
//! `test-util` feature.
//!
//! Moved verbatim out of the single 809-line `impl XionApp` block in `mod.rs`.

use super::XionApp;

impl XionApp {
    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn state_for_test(&self) -> &crate::ui::AppState {
        &self.state
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn dual_pane_for_test(&self) -> bool {
        self.dual_pane.enabled
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn quick_filter_for_test(&self) -> &str {
        &self.quick_filter
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn quick_filter_active_for_test(&self) -> bool {
        self.quick_filter_active
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn tab_count_for_test(&self) -> usize {
        self.tab_manager.count()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn active_tab_for_test(&self) -> usize {
        self.tab_manager.active
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn context_menu_open_for_test(&self) -> bool {
        self.menus.context_open
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn rename_dialog_for_test(&self) -> bool {
        self.rename_dialog.is_some()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn properties_dialog_for_test(&self) -> bool {
        self.properties_dialog.is_some()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn hex_view_for_test(&self) -> bool {
        self.hex_view.is_some()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn diff_view_for_test(&self) -> bool {
        self.diff_view.is_some()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn grep_state_for_test(&self) -> bool {
        self.grep_state.is_some()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn bulk_rename_for_test(&self) -> bool {
        self.bulk_rename.is_some()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn address_input_for_test(&self) -> &str {
        &self.address_input
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn address_editing_for_test(&self) -> bool {
        self.address_editing
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn recents_is_empty_for_test(&self) -> bool {
        self.recents.list().is_empty()
    }

    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn network_needs_scan_for_test(&self) -> bool {
        self.network_discovery.needs_scan()
    }
}
