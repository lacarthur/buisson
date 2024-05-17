use crossterm::{
    event,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

use buisson::app::App;
use ratatui::prelude::{CrosstermBackend, Terminal};
use std::io::{stdout, Result};

fn main() -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let mut app = App::new()?;

    while !app.is_quitting() {
        terminal.draw(|frame| {
            app.render(frame.size(), frame);
        })?;

        if event::poll(std::time::Duration::from_millis(16))? {
            app.handle_event(&event::read()?)
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
