use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{layout::Rect, widgets::Paragraph, Frame};

#[derive(Default, Debug)]
pub struct TextInput {
    text: String,
    name: Option<String>,
}

impl TextInput {
    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let text_widget = Paragraph::new(self.text.as_str());

        frame.render_widget(text_widget, area);
        frame.set_cursor(area.x + self.text.len() as u16, area.y);
    }

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            name: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn handle_key(&mut self, key: &KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char(c) => self.text.push(c),
            KeyCode::Backspace => {
                self.text.pop();
            }
            _ => (),
        }
    }

    pub fn to_str(&self) -> &str {
        &self.text
    }
}
