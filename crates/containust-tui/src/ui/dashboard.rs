//! Main dashboard layout showing container overview.
//!
//! Displays a table of all containers with their status,
//! resource usage, and uptime.

use ratatui::Frame;

use crate::app::App;

/// Renders the main dashboard view.
pub fn render_dashboard(_frame: &mut Frame, _app: &App) {
    // Layout: header bar, container table, footer with keybindings
}
