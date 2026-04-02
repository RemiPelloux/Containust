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
    pub const fn new() -> Self {
        Self {
            running: true,
            current_view: View::Dashboard,
            selected_index: 0,
        }
    }

    /// Signals the app to quit.
    pub const fn quit(&mut self) {
        self.running = false;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_new_has_running_state_and_dashboard_view() {
        let app = App::new();
        assert!(app.running);
        assert_eq!(app.current_view, View::Dashboard);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn app_default_equals_new() {
        let a = App::new();
        let b = App::default();
        assert!(a.running == b.running);
        assert_eq!(a.current_view, b.current_view);
        assert_eq!(a.selected_index, b.selected_index);
    }

    #[test]
    fn app_quit_sets_running_false() {
        let mut app = App::new();
        assert!(app.running);
        app.quit();
        assert!(!app.running);
    }

    #[test]
    fn view_variants_exist() {
        let views = [View::Dashboard, View::ContainerDetail, View::TraceLog];
        // Verify all variants exist and are distinct
        assert_ne!(views[0], views[1]);
        assert_ne!(views[1], views[2]);
        assert_ne!(views[0], views[2]);
    }

    #[test]
    fn view_equality() {
        assert_eq!(View::Dashboard, View::Dashboard);
        assert_eq!(View::ContainerDetail, View::ContainerDetail);
        assert_eq!(View::TraceLog, View::TraceLog);
    }

    #[test]
    fn view_is_copy() {
        let view = View::Dashboard;
        let copied = view;
        assert_eq!(view, copied);
    }

    #[test]
    fn app_debug_output() {
        let app = App::new();
        let debug = format!("{app:?}");
        assert!(debug.contains("App"));
    }

    #[test]
    fn app_view_debug_output() {
        let debug = format!("{:?}", View::TraceLog);
        assert!(debug.contains("TraceLog"));
    }

    #[test]
    fn app_selected_index_is_mutable() {
        let mut app = App::new();
        app.selected_index = 3;
        assert_eq!(app.selected_index, 3);
    }

    #[test]
    fn app_can_transition_selected_index() {
        let mut app = App::new();
        assert_eq!(app.selected_index, 0);
        app.selected_index += 1;
        assert_eq!(app.selected_index, 1);
    }
}
