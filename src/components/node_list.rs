use std::cell::RefCell;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect, style::{Modifier, Style}, text::Text, widgets::{Block, List, ListItem, ListState}, Frame
};

use crate::{lessons::{GraphNode, Id}, style_from_status};

pub trait GraphNodeDisplayer: Default {
    fn render<'a>(&'a self, node: &'a GraphNode) -> ListItem<'_>;
}

#[derive(Default)]
pub struct BasicNodeDisplayer;

impl GraphNodeDisplayer for BasicNodeDisplayer {
    fn render<'a>(&'a self, node: &'a GraphNode) -> ListItem<'_> {
        let text = Text::from(node.lesson.name.as_str());
        ListItem::new(text).style(style_from_status(&node.status))
    }
}

pub struct NodeListStyle<'a> {
    display_selected: bool,
    block: Option<Block<'a>>,
}

impl<'a> Default for NodeListStyle<'a> {
    fn default() -> Self {
        Self {
            display_selected: true,
            block: None,
        }
    }
}

impl<'a> NodeListStyle<'a> {
    pub fn dont_display_selected(mut self) -> Self {
        self.display_selected = false;
        self
    }

    pub fn block(mut self, new_val: Block<'a>) -> Self {
        self.block = Some(new_val);
        self
    }
}

#[derive(Debug)]
pub struct NodeListDisplay<NodeDisplayer: GraphNodeDisplayer + Default> {
    nodes: Vec<GraphNode>,
    state: RefCell<ListState>,
    displayer: NodeDisplayer,
}

impl<NodeDisplayer: GraphNodeDisplayer> NodeListDisplay<NodeDisplayer> {
    pub fn new(nodes: Vec<GraphNode>) -> Self {
        Self {
            nodes,
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

    pub fn add_new_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
    }

    fn get_widget<'a>(&'a self, block: Option<Block<'a>>) -> List<'a> {
        let items = self.nodes.iter().map(|node| self.displayer.render(node));
        match block {
            Some(block) => {
                List::new(items)
                    .block(block)
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            }
            None => {
                List::new(items)
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            }
        }
    }


    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        self.render_with_style(area, frame, NodeListStyle::default());
    }

    pub fn render_with_style(&self, area: Rect, frame: &mut Frame<'_>, style: NodeListStyle) {
        let list = self.get_widget(style.block);

        if style.display_selected {
            frame.render_stateful_widget(list, area, &mut self.state.borrow_mut());
        } else {
            frame.render_widget(list, area);
        }
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

    pub fn get_all_ids(&self) -> Vec<Id> {
        self.nodes.iter().map(|node| node.lesson.get_id()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
