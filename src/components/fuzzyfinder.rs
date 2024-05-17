use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Text,
    widgets::{Block, Borders, ListItem, Paragraph},
    Frame,
};

use crate::{
    components::node_list::{GraphNodeDisplayer, NodeListDisplay},
    components::textinput::TextInput,
    lessons::{GraphNode, Id},
    style_from_status,
};

#[derive(Default)]
struct FuzzyFinderNodeDisplayer;

impl GraphNodeDisplayer for FuzzyFinderNodeDisplayer {
    fn render<'a>(&'a self, node: &'a GraphNode) -> ListItem<'_> {
        let text = Text::from(node.lesson.name.as_str());
        ListItem::new(text).style(style_from_status(&node.status))
    }
}

pub struct FuzzyFinder {
    original_list: Vec<GraphNode>,
    display_list: NodeListDisplay<FuzzyFinderNodeDisplayer>,
    search_bar: TextInput,
    state: FuzzyFinderState,
}

pub enum FuzzyFinderState {
    TypingSearch,
    NavigatingResults,
}

pub enum FuzzyFinderAction {
    Noop,
    Terminate(Option<Id>),
}

impl FuzzyFinder {
    pub fn new(list: Vec<GraphNode>) -> Self {
        Self {
            original_list: list.clone(),
            display_list: NodeListDisplay::new(list, "Results".into()),
            search_bar: TextInput::default(),
            state: FuzzyFinderState::TypingSearch,
        }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(3)]).split(area);
        let list_area = layout[0];
        let searchbar_area = layout[1];
        match &self.state {
            FuzzyFinderState::TypingSearch => {
                let block = Block::new().title("Search").borders(Borders::ALL);
                let inner = block.inner(searchbar_area);
                frame.render_widget(block, searchbar_area);
                self.search_bar.render(inner, frame);
                self.display_list.render(list_area, frame);
            }
            FuzzyFinderState::NavigatingResults => {
                let text_widget = Paragraph::new(self.search_bar.to_str())
                    .block(Block::new().title("Search").borders(Borders::ALL));
                frame.render_widget(text_widget, searchbar_area);
                self.display_list.render_stateful(list_area, frame);
            }
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> FuzzyFinderAction {
        if key.kind != KeyEventKind::Press {
            return FuzzyFinderAction::Noop;
        }
        match &self.state {
            FuzzyFinderState::TypingSearch => match key.code {
                KeyCode::Esc | KeyCode::Enter => self.state = FuzzyFinderState::NavigatingResults,
                _ => {
                    self.search_bar.handle_key(key);
                    self.display_list.update_nodes(
                        self.original_list
                            .iter()
                            .filter(|&node| node.lesson.name.contains(self.search_bar.to_str()))
                            .cloned()
                            .collect(),
                    );
                }
            },
            FuzzyFinderState::NavigatingResults => match key.code {
                KeyCode::Char('a') => self.state = FuzzyFinderState::TypingSearch,
                KeyCode::Esc => return FuzzyFinderAction::Terminate(None),
                KeyCode::Enter => {
                    return FuzzyFinderAction::Terminate(self.display_list.selected_id())
                }
                _ => self.display_list.handle_key(key),
            },
        }
        FuzzyFinderAction::Noop
    }
}
