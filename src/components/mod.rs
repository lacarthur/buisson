use ratatui::{style::Style, widgets::Borders};

pub mod fuzzyfinder;
pub mod lesson_edit_form;
pub mod node_list;
pub mod textinput;

#[derive(Default)]
pub struct BlockInfo {
    borders: Borders,
    title: String,
    style: Style,
}

impl BlockInfo {
    pub fn borders(mut self, new_val: Borders) -> Self {
        self.borders = new_val;
        self
    }

    pub fn title(mut self, new_val: String) -> Self {
        self.title = new_val;
        self
    }
    
    pub fn style(mut self, new_val: Style) -> Self {
        self.style = new_val;
        self
    }
}
