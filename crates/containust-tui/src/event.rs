//! Terminal event handling.
//!
//! Captures keyboard, mouse, and resize events from the terminal
//! and dispatches them to the application state machine.

/// Terminal input events.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// A key was pressed.
    Key(crossterm::event::KeyEvent),
    /// The terminal was resized.
    Resize(u16, u16),
    /// A periodic tick for UI refresh.
    Tick,
}
