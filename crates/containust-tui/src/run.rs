//! Interactive dashboard entrypoint for `ctst ps --tui`.

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use crate::app::App;

/// One row shown in the TUI container table.
#[derive(Debug, Clone)]
pub struct ContainerRow {
    /// Container id.
    pub id: String,
    /// Component name.
    pub name: String,
    /// Lifecycle state label.
    pub state: String,
    /// Host PID when running.
    pub pid: String,
    /// Image reference.
    pub image: String,
}

/// Runs the interactive dashboard until the user quits with `q` or Esc.
///
/// # Errors
///
/// Returns an I/O error when the terminal cannot be initialized or restored.
pub fn run_dashboard(rows: &[ContainerRow]) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new();
    let result = event_loop(&mut terminal, &mut app, rows);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    rows: &[ContainerRow],
) -> io::Result<()> {
    while app.running {
        let _frame = terminal.draw(|frame| draw(frame, app, rows))?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key(app, key.code, rows.len());
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, row_count: usize) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit(),
        KeyCode::Down | KeyCode::Char('j') => {
            if row_count > 0 {
                app.selected_index = (app.selected_index + 1).min(row_count - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.selected_index = app.selected_index.saturating_sub(1);
        }
        _ => {}
    }
}

fn draw(frame: &mut ratatui::Frame, app: &App, rows: &[ContainerRow]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(frame.area());

    frame.render_widget(header_widget(), chunks[0]);
    frame.render_widget(containers_table(app, rows), chunks[1]);
    frame.render_widget(footer_widget(app, rows.len()), chunks[2]);
}

fn header_widget() -> Paragraph<'static> {
    Paragraph::new("Containust — press q to quit, ↑/↓ to select").block(
        Block::default()
            .borders(Borders::ALL)
            .title("ctst ps --tui"),
    )
}

fn footer_widget(app: &App, count: usize) -> Paragraph<'static> {
    Paragraph::new(format!(
        "{count} container(s) — selection {}",
        app.selected_index
    ))
}

fn containers_table<'a>(app: &App, rows: &'a [ContainerRow]) -> Table<'a> {
    let table_rows: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let style = if idx == app.selected_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![
                row.id.as_str(),
                row.name.as_str(),
                row.state.as_str(),
                row.pid.as_str(),
                row.image.as_str(),
            ])
            .style(style)
        })
        .collect();

    Table::new(
        table_rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(35),
        ],
    )
    .header(Row::new(vec!["ID", "NAME", "STATE", "PID", "IMAGE"]))
    .block(Block::default().borders(Borders::ALL).title("Containers"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_key_quit() {
        let mut app = App::new();
        handle_key(&mut app, KeyCode::Char('q'), 0);
        assert!(!app.running);
    }

    #[test]
    fn handle_key_moves_selection() {
        let mut app = App::new();
        handle_key(&mut app, KeyCode::Down, 3);
        assert_eq!(app.selected_index, 1);
        handle_key(&mut app, KeyCode::Up, 3);
        assert_eq!(app.selected_index, 0);
    }
}
