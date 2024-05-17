use std::cell::RefCell;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::lessons::{GraphNode, Id};

pub trait GraphNodeDisplayer: Default {
    fn render<'a>(&'a self, node: &'a GraphNode) -> ListItem<'_>;
}

pub struct NodeListDisplay<NodeDisplayer: GraphNodeDisplayer> {
    nodes: Vec<GraphNode>,
    title: String,
    state: RefCell<ListState>,
    displayer: NodeDisplayer,
}

impl<NodeDisplayer: GraphNodeDisplayer> NodeListDisplay<NodeDisplayer> {
    pub fn new(nodes: Vec<GraphNode>, title: String) -> Self {
        Self {
            nodes,
            title,
            state: RefCell::new(ListState::default()),
            displayer: NodeDisplayer::default(),
        }
    }

    pub fn update_nodes(&mut self, new_nodes: Vec<GraphNode>) {
        self.nodes = new_nodes;

        let state = self.state.borrow().clone();

        if let Some(index) = state.selected() {
            if self.nodes.is_empty() {
                *self.state.borrow_mut() = ListState::default();
            } else if index >= self.nodes.len() {
                *self.state.borrow_mut() = ListState::default().with_selected(Some(index - 1));
            }
        }
    }

    fn get_widget(&self) -> List<'_> {
        let items = self.nodes.iter().map(|node| self.displayer.render(node));
        List::new(items)
            .block(
                Block::default()
                    .title(self.title.as_str())
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    }

    pub fn render_stateful(&self, area: Rect, frame: &mut Frame<'_>) {
        frame.render_stateful_widget(self.get_widget(), area, &mut self.state.borrow_mut());
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        frame.render_widget(self.get_widget(), area);
    }

    pub fn select(&self, id: Id) {
        *self.state.borrow_mut() = ListState::default().with_selected(Some(id as usize));
    }

    pub fn handle_key(&mut self, key: &KeyEvent) {
        let state = self.state.borrow().clone();
        match key.code {
            KeyCode::Char('j') => {
                if let Some(id) = state.selected() {
                    if !self.nodes.is_empty() && id < self.nodes.len() - 1 {
                        *self.state.borrow_mut() = ListState::default().with_selected(Some(id + 1));
                    }
                } else if !self.nodes.is_empty() {
                    *self.state.borrow_mut() = ListState::default().with_selected(Some(0));
                }
            }
            KeyCode::Char('k') => {
                if let Some(id) = state.selected() {
                    if id > 0 {
                        *self.state.borrow_mut() = ListState::default().with_selected(Some(id - 1));
                    }
                }
            }
            _ => (),
        }
    }

    pub fn selected_id(&self) -> Option<Id> {
        self.state
            .borrow()
            .selected()
            .map(|index| self.nodes[index].lesson.get_id())
    }
}
