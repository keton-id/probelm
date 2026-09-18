pub mod app;
pub mod ui;

pub use app::{App, InputMode, ModelItem, Tab};
pub use ui::render;

use app::{InputMode as AppInputMode, Tab as AppTab};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;

pub async fn run_tui(mut app: App) -> Result<(), String> {
    enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| format!("enter alt screen: {e}"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| format!("init terminal: {e}"))?;

    let res = event_loop(&mut terminal, &mut app).await;

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    res
}

async fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), String> {
    while !app.should_quit {
        terminal
            .draw(|f| ui::render(f, app))
            .map_err(|e| format!("render: {e}"))?;

        if event::poll(Duration::from_millis(50)).map_err(|e| format!("poll event: {e}"))? {
            if let Event::Key(key) = event::read().map_err(|e| format!("read event: {e}"))? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key.code).await;
                }
            }
        }
    }
    Ok(())
}

async fn handle_key(app: &mut App, code: KeyCode) {
    match app.input_mode {
        AppInputMode::Normal => match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.select_prev();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.select_next();
            }
            KeyCode::Char(' ') => {
                app.toggle_selected_model();
            }
            KeyCode::Char('a') => {
                app.input_mode = AppInputMode::AddingModel;
                app.input_buffer.clear();
            }
            KeyCode::Char('d') | KeyCode::Backspace => {
                app.delete_selected_model();
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                app.run_probe_selected().await;
            }
            KeyCode::Tab => {
                app.active_tab = match app.active_tab {
                    AppTab::Models => AppTab::Adapters,
                    AppTab::Adapters => AppTab::Help,
                    AppTab::Help => AppTab::Models,
                };
            }
            KeyCode::Char('1') => app.active_tab = AppTab::Models,
            KeyCode::Char('2') => app.active_tab = AppTab::Adapters,
            KeyCode::Char('3') => app.active_tab = AppTab::Help,
            _ => {}
        },
        AppInputMode::AddingModel => match code {
            KeyCode::Esc => {
                app.input_mode = AppInputMode::Normal;
                app.input_buffer.clear();
            }
            KeyCode::Enter => {
                app.submit_add_model();
            }
            KeyCode::Backspace => {
                app.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                app.input_buffer.push(c);
            }
            _ => {}
        },
    }
}
