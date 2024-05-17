use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    widgets::{Block, Borders},
    Frame,
};

use crate::{
    components::textinput::TextInput,
    lessons::{LessonInfo, LessonStatus},
};

#[derive(Default, Debug)]
pub struct LessonEditForm {
    block_name: String,
    name_input: TextInput,
}

impl LessonEditForm {
    pub fn new(block_name: String, name: String) -> Self {
        Self {
            block_name,
            name_input: TextInput::new(name),
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) {
        self.name_input.handle_key(key);
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let main_block = Block::new()
            .title(self.block_name.as_str())
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::new().bold());
        let main_block_inner = main_block.inner(area);
        let layout = Layout::vertical([Constraint::Min(3), Constraint::Percentage(100)])
            .split(main_block_inner);

        let name_input_area = layout[0];
        let block = Block::new().title("Lesson Name").borders(Borders::ALL);

        let inner = block.inner(name_input_area);

        frame.render_widget(main_block, area);
        frame.render_widget(block, name_input_area);
        self.name_input.render(inner, frame);
    }

    pub fn to_lesson_info(&self) -> LessonInfo {
        LessonInfo {
            name: self.name_input.to_str().into(),
            depends_on: vec![],
            status: LessonStatus::NotPracticed,
        }
    }
}
