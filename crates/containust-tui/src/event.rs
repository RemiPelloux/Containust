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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn terminal_event_resize_stores_dimensions() {
        let event = TerminalEvent::Resize(192, 48);
        match event {
            TerminalEvent::Resize(w, h) => {
                assert_eq!(w, 192);
                assert_eq!(h, 48);
            }
            _ => panic!("expected Resize variant"),
        }
    }

    #[test]
    fn terminal_event_tick_variant() {
        let event = TerminalEvent::Tick;
        match event {
            TerminalEvent::Tick => {}
            _ => panic!("expected Tick variant"),
        }
    }

    #[test]
    fn terminal_event_clone_works() {
        let original = TerminalEvent::Tick;
        let cloned = original.clone();
        match (original, cloned) {
            (TerminalEvent::Tick, TerminalEvent::Tick) => {}
            _ => panic!("expected Tick variants"),
        }
    }

    #[test]
    fn terminal_event_resize_clone() {
        let original = TerminalEvent::Resize(80, 24);
        let cloned = original.clone();
        match (original, cloned) {
            (TerminalEvent::Resize(w1, h1), TerminalEvent::Resize(w2, h2)) => {
                assert_eq!(w1, w2);
                assert_eq!(h1, h2);
            }
            _ => panic!("expected Resize variants"),
        }
    }

    #[test]
    fn terminal_event_zero_dimensions_allowed() {
        let event = TerminalEvent::Resize(0, 0);
        match event {
            TerminalEvent::Resize(w, h) => {
                assert_eq!(w, 0);
                assert_eq!(h, 0);
            }
            _ => panic!("expected Resize variant"),
        }
    }

    #[test]
    fn terminal_event_max_dimensions() {
        let event = TerminalEvent::Resize(u16::MAX, u16::MAX);
        match event {
            TerminalEvent::Resize(w, h) => {
                assert_eq!(w, u16::MAX);
                assert_eq!(h, u16::MAX);
            }
            _ => panic!("expected Resize variant"),
        }
    }
}
