//! TUI application state machine.
//!
//! Manages the main event loop, view transitions, and application state.

/// Which view the TUI is currently showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Main dashboard with container list.
    Dashboard,
    /// Detailed view of a single container.
    ContainerDetail,
    /// eBPF trace log view.
    TraceLog,
}

/// Root application state for the TUI.
#[derive(Debug)]
pub struct App {
    /// Whether the app should continue running.
    pub running: bool,
    /// Current active view.
    pub current_view: View,
    /// Index of the selected container in the list.
    pub selected_index: usize,
}

impl App {
    /// Creates a new application state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: true,
            current_view: View::Dashboard,
            selected_index: 0,
        }
    }

    /// Signals the app to quit.
    pub fn quit(&mut self) {
        self.running = false;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
