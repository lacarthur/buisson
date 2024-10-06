use crossterm::{
    event,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

use buisson::app::{App, AppError};
use cli_log::*;
use ratatui::prelude::{CrosstermBackend, Terminal};
use std::io::stdout;

fn main() -> Result<(), AppError> {
    init_cli_log!();
    stdout()
        .execute(EnterAlternateScreen)
        .map_err(AppError::IOError)?;
    enable_raw_mode().map_err(AppError::IOError)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).map_err(AppError::IOError)?;
    terminal.clear().map_err(AppError::IOError)?;

    let mut app = App::new()?;

    while !app.is_quitting() {
        terminal
            .draw(|frame| {
                app.render(frame.area(), frame);
            })
            .map_err(AppError::IOError)?;

        if event::poll(std::time::Duration::from_millis(16)).map_err(AppError::IOError)? {
            app.handle_event(&event::read().map_err(AppError::IOError)?);
        }
    }

    stdout()
        .execute(LeaveAlternateScreen)
        .map_err(AppError::IOError)?;
    disable_raw_mode().map_err(AppError::IOError)?;
    Ok(())
}
