use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, ListItem},
    Frame,
};

use crate::{
    components::node_list::{GraphNodeDisplayer, NodeListDisplay},
    components::textinput::TextInput,
    lessons::{GraphNode, Id},
    style_from_status,
};

use super::{node_list::NodeListStyle, textinput::TextInputStyle};

#[derive(Default, Debug)]
struct FuzzyFinderNodeDisplayer;

impl GraphNodeDisplayer for FuzzyFinderNodeDisplayer {
    fn render<'a>(&'a self, node: &'a GraphNode) -> ListItem<'_> {
        let text = Text::from(node.lesson.name.as_str());
        ListItem::new(text).style(style_from_status(&node.status))
    }
}

/// A fuzzy finder, useful to search for lessons by name.
#[derive(Debug)]
pub struct FuzzyFinder {
    /// The original list of elements you are searching through
    original_list: Vec<GraphNode>,
    /// The component displaying the list of lesssons matching the current search
    display_list: NodeListDisplay<FuzzyFinderNodeDisplayer>,
    search_bar: TextInput,
    state: FuzzyFinderState,
}

#[derive(Debug)]
pub enum FuzzyFinderState {
    TypingSearch,
    NavigatingResults,
}

/// An action to be returned when the fuzzy finder handles an event.
pub enum FuzzyFinderAction {
    /// Nothing, the fuzzy finder is still running
    Noop,
    /// The fuzzy finder should be terminated, and the user selected either nothing (`None`) or
    /// the lesson whose `Id` is given here.
    Terminate(Option<Id>),
}

impl FuzzyFinder {
    pub fn new(list: Vec<GraphNode>) -> Self {
        let display_list = NodeListDisplay::new(list.clone());
        if !list.is_empty() {
            display_list.select_first();
        }
        Self {
            original_list: list.clone(),
            display_list,
            search_bar: TextInput::default(),
            state: FuzzyFinderState::TypingSearch,
        }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(3)]).split(area);
        let list_area = layout[0];
        let searchbar_area = layout[1];

        self.render_results_list(list_area, frame);
        self.render_searchbar(searchbar_area, frame);
    }

    fn render_searchbar(&self, area: Rect, frame: &mut Frame<'_>) {
        let block = Block::new()
            .title(Line::from("Search").alignment(Alignment::Center))
            .borders(Borders::ALL)
            .style(if let FuzzyFinderState::TypingSearch = self.state {
                Style::default().bold()
            } else {
                Style::default()
            });

        let inner_area = block.inner(area);

        frame.render_widget(block, area);

        match &self.state {
            FuzzyFinderState::NavigatingResults => self.search_bar.render(inner_area, frame),
            FuzzyFinderState::TypingSearch => self.search_bar.render_with_style(
                inner_area,
                frame,
                TextInputStyle::default().display_cursor(),
            ),
        }
    }

    fn render_results_list(&self, area: Rect, frame: &mut Frame<'_>) {
        let style = NodeListStyle::default().block(
            Block::new()
                .borders(Borders::ALL)
                .title(Line::from("Results").alignment(Alignment::Center))
                .style(if let FuzzyFinderState::NavigatingResults = self.state {
                    Style::default().bold()
                } else {
                    Style::default()
                }),
        );
        self.display_list.render_with_style(area, frame, style);
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
                    if !self.display_list.is_empty() {
                        self.display_list.select_first();
                    }
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
