use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

use crate::tui_ui::app::{App, InputMode, ViewMode};

pub fn handle_events(app: &mut App) -> std::io::Result<()> {
    while event::poll(Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match app.input_mode {
                InputMode::Normal => handle_normal_mode(app, key.code),
                InputMode::Search => handle_search_mode(app, key.code),
            }
        }
    }
    Ok(())
}

fn handle_normal_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.current_view = app.current_view.prev();
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.current_view = app.current_view.next();
        }
        KeyCode::Char('1') => app.current_view = ViewMode::Overview,
        KeyCode::Char('2') => app.current_view = ViewMode::Timeline,
        KeyCode::Char('3') => app.current_view = ViewMode::Contributors,
        KeyCode::Char('4') => app.current_view = ViewMode::Hotspots,
        KeyCode::Char('5') => app.current_view = ViewMode::Branches,
        KeyCode::Char('j') | KeyCode::Down => {
            app.scroll_down();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.scroll_up();
        }
        KeyCode::Char('G') => {
            app.scroll_bottom();
        }
        KeyCode::Char('g') => {
            app.scroll_top();
        }
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.search_query.clear();
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.scroll_down();
            }
        }
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.scroll_up();
            }
        }
        _ => {}
    }
}

fn handle_search_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.search_query.clear();
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
        }
        _ => {}
    }
}
