use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{layout::{Alignment, Constraint, Layout, Rect}, style::{Style, Stylize}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem, Paragraph}, Frame};

use crate::{
    app::Context, components::textinput::TextInput, lessons::{Id, LessonInfo}, style_from_status
};

use super::node_list::NodeList;

/// A fuzzy finder, useful to search for lessons by name.
#[derive(Debug)]
pub struct FuzzyFinder {
    /// The original list of elements you are searching through
    original_list: Vec<(Id, LessonInfo)>,
    /// The component displaying the list of lesssons matching the current search
    match_list: NodeList,
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
    pub fn new(original_list: Vec<(Id, LessonInfo)>) -> Self {
        let id_list = original_list.iter()
            .map(|&(id, _)| id)
            .collect();

        let match_list = NodeList::new(id_list);
        Self {
            original_list,
            match_list,
            search_bar: TextInput::default(),
            state: FuzzyFinderState::TypingSearch,
        }
    }

    fn perform_search(&self) -> Vec<Id> {
        let searched_string = self.search_bar.text();
        self.original_list.iter()
            .filter(|(_, info)| info.name.contains(searched_string))
            .map(|&(id, _)| id)
            .collect()
    }

}

impl FuzzyFinder {
    pub fn handle_key(&mut self, key: &KeyEvent) -> FuzzyFinderAction {
        if key.kind != KeyEventKind::Press {
            return FuzzyFinderAction::Noop;
        }

        match &self.state {
            FuzzyFinderState::TypingSearch => self.handle_key_typing(key),
            FuzzyFinderState::NavigatingResults => self.handle_key_navigating(key),
        }
    }

    fn handle_key_typing(&mut self, key: &KeyEvent) -> FuzzyFinderAction {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.state = FuzzyFinderState::NavigatingResults,
            _ => {
                self.search_bar.handle_key(key);
                let new_matches = self.perform_search();
                self.match_list.change_values(new_matches);
            }
        }
        FuzzyFinderAction::Noop
    }

    fn handle_key_navigating(&mut self, key: &KeyEvent) -> FuzzyFinderAction {
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('i') => {
                self.state = FuzzyFinderState::TypingSearch;
                FuzzyFinderAction::Noop
            }
            KeyCode::Esc => FuzzyFinderAction::Terminate(None),
            KeyCode::Enter => FuzzyFinderAction::Terminate(self.match_list.currently_selected_id()),
            _ => {
                self.match_list.handle_key(key);
                FuzzyFinderAction::Noop
            }
        }
    }
}

impl FuzzyFinder {
    pub fn render(&self, context: Context<'_>, area: Rect, frame: &mut Frame<'_>) {
        let main_layout = Layout::vertical([Constraint::Percentage(100), Constraint::Min(3)]).split(area);

        let list_area = main_layout[0];
        let searchbar_area = main_layout[1];

        self.render_results_list(context, list_area, frame);
        self.render_searchbar(searchbar_area, frame);
    }

    fn render_results_list(&self, context: Context<'_>, area: Rect, frame: &mut Frame<'_>) {
        let block = Block::new()
            .borders(Borders::ALL)
            .title(Line::from("Results").alignment(Alignment::Center))
            .border_style(if let FuzzyFinderState::NavigatingResults = self.state {
                Style::default().bold()
            } else {
                Style::default()
            });
        let list_items = self.match_list.ids().iter()
            .map(|id| {
                let node = context.lessons.get(id).unwrap();
                let name = &node.lesson.name;
                let occurences = name.match_indices(self.search_bar.text());

                let mut spans = vec![];
                let mut prev = 0;

                for (index, _) in occurences {
                    let span_not_match = Span::styled(
                        &name[prev..index],
                        style_from_status(&node.status),
                    );
                    spans.push(span_not_match);
                    let span_match = Span::styled(self.search_bar.text(), Style::default().blue());
                    spans.push(span_match);
                    prev = index + self.search_bar.text().len();
                }

                spans.push(Span::styled(&name[prev..], style_from_status(&node.status)));
                let text = Line::from(spans);
                ListItem::new(text)
            });
        let list = List::new(list_items).block(block).highlight_style(Style::default().reversed());

        match self.state {
            FuzzyFinderState::TypingSearch => {
                frame.render_widget(list, area);
            },
            FuzzyFinderState::NavigatingResults => {
                let list_state = &mut *self.match_list.list_state_refcell().borrow_mut();
                frame.render_stateful_widget(list, area, list_state)
            }
        }
    }

    fn render_searchbar(&self, area: Rect, frame: &mut Frame<'_>) {
        let block = Block::new()
            .title(Line::from("Search").alignment(Alignment::Center))
            .borders(Borders::ALL)
            .border_style(if let FuzzyFinderState::TypingSearch = self.state {
                Style::default().bold()
            } else {
                Style::default()
            });

        let text_widget = Paragraph::new(self.search_bar.text())
            .block(block);

        frame.render_widget(text_widget, area);

        if matches!(self.state, FuzzyFinderState::TypingSearch) {
            frame.set_cursor(area.x + 1 + self.search_bar.text_len(), area.y + 1);
        }
    }
}
