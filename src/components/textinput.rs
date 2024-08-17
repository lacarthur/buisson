use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    widgets::{Block, Paragraph},
    Frame,
};

#[derive(Default)]
pub struct TextInputStyle<'a> {
    /// whether or not to display the cursor.
    display_cursor: bool,
    block: Option<Block<'a>>,
}

impl<'a> TextInputStyle<'a> {
    pub fn display_cursor(mut self) -> Self {
        self.display_cursor = true;
        self
    }

    pub fn dont_display_cursor(mut self) -> Self {
        self.display_cursor = false;
        self
    }

    pub fn block(mut self, new_val: Block<'a>) -> Self {
        self.block = Some(new_val);
        self
    }
}

#[derive(Default, Debug)]
pub struct TextInput {
    text: String,
}

impl TextInput {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        self.render_with_style(area, frame, TextInputStyle::default());
    }

    pub fn render_textinput(&self, area: Rect, frame: &mut Frame<'_>, display_cursor: bool) {
        let text_widget = Paragraph::new(self.text.as_str());

        if display_cursor {
            frame.set_cursor(area.x + self.text_len(), area.y);
        }

        frame.render_widget(text_widget, area);
    }

    pub fn render_with_style(&self, area: Rect, frame: &mut Frame<'_>, style: TextInputStyle) {
        match style.block {
            Some(block) => {
                let inner_area = block.inner(area);

                frame.render_widget(block, area);
                self.render_textinput(inner_area, frame, style.display_cursor);
            }
            None => self.render_textinput(area, frame, style.display_cursor),
        }
    }

    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
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

    /// the length of the displayed text, in actual characters instead of bytes
    pub fn text_len(&self) -> u16 {
        self.text.chars().count() as u16
    }
}
