use std::cell::RefCell;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;

use crate::lessons::Id;

#[derive(Debug)]
pub struct NodeList {
    ids: Vec<Id>,
    list_state: RefCell<ListState>
}

impl NodeList {
    pub fn new(ids: Vec<Id>) -> Self {
        let mut list_state = ListState::default();
        list_state.select_first();
        Self {
            ids,
            list_state: RefCell::new(list_state),
        }
    }

    pub fn currently_selected_id(&self) -> Option<Id> {
        self.list_state.borrow().selected().map(|list_index| self.ids[list_index])
    }

    pub fn ids(&self) -> &[Id] {
        &self.ids
    }

    pub fn list_state_refcell(&self) -> &RefCell<ListState> {
        &self.list_state
    }

    pub fn select(&mut self, id: Id) {
        let index = self.ids.iter().position(|&m_id| id == m_id);

        self.list_state.get_mut().select(index);
    }

    pub fn remove_node(&mut self, id: Id) {
        self.ids.retain(|&x| x != id)
    }

    pub fn push(&mut self, id: Id) {
        self.ids.push(id);
    }

    pub fn change_values(&mut self, new_values: Vec<Id>) {
        self.ids = new_values;
    }
}

impl NodeList {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Char('j') => self.list_state.get_mut().select_next(),
            KeyCode::Char('k') => self.list_state.get_mut().select_previous(),
            _ => (),
        }
    }
}
