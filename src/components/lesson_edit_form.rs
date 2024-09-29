use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Position, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::{
    app::Context, components::textinput::TextInput, lessons::{Id, LessonInfo, LessonStatus}, style_from_status
};

use super::{
    fuzzyfinder::{FuzzyFinder, FuzzyFinderAction}, node_list::NodeList
};

#[derive(Debug)]
pub enum LessonEditFormState {
    EditingName,
    NavigatingPrereqs,
    AddingPrereq(FuzzyFinder),
    Validating,
}

pub struct LessonEditForm {
    /// represents the lessons that can be added as prerequisite. It is initialized as all the
    /// existing lessons for new lessons, and all the lessons that don't depend on the edited
    /// lesson for existing lessons
    potential_prerequisites: HashMap<Id, (LessonInfo, bool)>,
    name_input: TextInput,
    prerequisites: NodeList,
    state: LessonEditFormState,
    // why do we (only) need this?
    lesson_status: LessonStatus,
}

pub enum LessonEditFormAction {
    Noop,
    Terminate(Option<LessonInfo>),
}

impl LessonEditForm {
    pub fn new(
        potential_prerequisites: HashMap<Id, LessonInfo>,
        lesson: LessonInfo,
    ) -> Self {
        let potential_prerequisites = potential_prerequisites.into_iter()
            .map(|(id, info)| (id, (info, false)))
            .collect();
        Self {
            potential_prerequisites,
            name_input: TextInput::new(lesson.name),
            prerequisites: NodeList::new(lesson.direct_prerequisites.clone()),
            state: LessonEditFormState::EditingName,
            lesson_status: lesson.status,
        }
    }

    pub fn to_lesson_info(&self) -> LessonInfo {
        LessonInfo {
            name: self.name_input.text().into(),
            direct_prerequisites: self.prerequisites.ids().into(),
            status: self.lesson_status,
        }
    }
}

impl LessonEditForm {
    pub fn handle_key(&mut self, key: &KeyEvent) -> LessonEditFormAction {
        match &mut self.state {
            LessonEditFormState::EditingName => match key.code {
                KeyCode::Tab | KeyCode::Enter => {
                    self.state = LessonEditFormState::NavigatingPrereqs
                }
                KeyCode::Char('j') if key.modifiers == KeyModifiers::ALT => {
                    self.state = LessonEditFormState::NavigatingPrereqs
                }
                KeyCode::Esc => return LessonEditFormAction::Terminate(None),
                _ => self.name_input.handle_key(key),
            },
            LessonEditFormState::NavigatingPrereqs => match key.code {
                KeyCode::Char('a') => {
                    self.state = LessonEditFormState::AddingPrereq(FuzzyFinder::new(
                            self.potential_prerequisites.clone().into_iter()
                            .filter(|(_, (_, already_prereq))| !already_prereq)
                            .map(|(id, (info, _))| (id, info)).collect()
                    ));
                }
                KeyCode::Esc => return LessonEditFormAction::Terminate(None),
                KeyCode::Tab => self.state = LessonEditFormState::Validating,
                KeyCode::BackTab => self.state = LessonEditFormState::EditingName,
                KeyCode::Char('j') if key.modifiers == KeyModifiers::ALT => {
                    self.state = LessonEditFormState::Validating
                }
                KeyCode::Char('k') if key.modifiers == KeyModifiers::ALT => {
                    self.state = LessonEditFormState::EditingName
                }
                KeyCode::Char('d') => {
                    if let Some(id) = self.prerequisites.currently_selected_id() {
                        self.potential_prerequisites.entry(id)
                            .and_modify(|(_, already_prereq)| *already_prereq = false);
                        self.prerequisites.remove_node(id);
                    }
                }
                _ => self.prerequisites.handle_key(key),
            },
            LessonEditFormState::AddingPrereq(finder) => match finder.handle_key(key) {
                FuzzyFinderAction::Terminate(Some(id)) => {
                    self.prerequisites.push(id);
                    self.potential_prerequisites.entry(id)
                        .and_modify(|(_, already_prereq)| *already_prereq = true);
                    self.state = LessonEditFormState::NavigatingPrereqs;
                }
                FuzzyFinderAction::Terminate(None) => {
                    self.state = LessonEditFormState::NavigatingPrereqs;
                }
                FuzzyFinderAction::Noop => (),
            },
            LessonEditFormState::Validating => match key.code {
                KeyCode::Char('k') if key.modifiers == KeyModifiers::ALT => {
                    self.state = LessonEditFormState::NavigatingPrereqs
                }
                KeyCode::BackTab => self.state = LessonEditFormState::NavigatingPrereqs,
                KeyCode::Enter => {
                    return LessonEditFormAction::Terminate(Some(self.to_lesson_info()))
                }
                KeyCode::Esc => return LessonEditFormAction::Terminate(None),
                _ => (),
            },
        }
        LessonEditFormAction::Noop
    }
}

impl LessonEditForm {
    pub fn render(&self, context: Context, area: Rect, frame: &mut Frame<'_>) {
        let main_block = Block::new()
            .title("Lesson Editor")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::new().bold());

        let main_block_inner = main_block.inner(area);
        frame.render_widget(main_block, area);

        let layout = Layout::vertical([
            Constraint::Min(3),
            Constraint::Percentage(100),
            Constraint::Min(5),
        ])
        .split(main_block_inner);

        let name_input_area = layout[0];
        let prereqs_area = layout[1];
        let validating_button_area = layout[2];

        self.render_name_input(name_input_area, frame);

        self.render_prereq_list(context.clone(), prereqs_area, frame);

        self.render_button(validating_button_area, frame);

        if let LessonEditFormState::AddingPrereq(finder) = &self.state {
            let layout = Layout::vertical([
                Constraint::Percentage(15),
                Constraint::Percentage(70),
                Constraint::Percentage(15),
            ])
            .split(prereqs_area);
            let layout = Layout::horizontal([
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10),
            ])
            .split(layout[1]);

            frame.render_widget(Clear, layout[1]);
            finder.render(context, layout[1], frame);
        }
    }

    fn render_name_input(&self, area: Rect, frame: &mut Frame<'_>) {
        let name_input_block = {
            let mut block = Block::new().title("Lesson Name").borders(Borders::ALL);
            if let LessonEditFormState::EditingName = self.state {
                block = block.border_style(Style::default().bold());
            }
            block
        };

        let text_widget = Paragraph::new(self.name_input.text()).block(name_input_block);

        frame.render_widget(text_widget, area);
        if matches!(self.state, LessonEditFormState::EditingName) {
            frame.set_cursor_position(Position { x: area.x + 1 + self.name_input.text_len(), y: area.y + 1 });
        }
    }

    fn render_prereq_list(&self, context: Context, area: Rect, frame: &mut Frame<'_>) {
        let title_style = match self.state {
            LessonEditFormState::EditingName | LessonEditFormState::Validating => Style::default(),
            LessonEditFormState::NavigatingPrereqs | LessonEditFormState::AddingPrereq(_) => {
                Style::default().bold()
            }
        };
        let prereq = Line::from("Prerequisites").style(title_style);
        let help = Line::from("Type 'a' to add a prerequisite");

        let layout = Layout::vertical([
            Constraint::Min(1),
            Constraint::Percentage(100),
            Constraint::Min(1),
        ])
        .split(area);

        frame.render_widget(prereq, layout[0]);

        let items = self.prerequisites
            .ids()
            .iter()
            .map(|id| {
                let node = context.lessons.get(id).unwrap();
                let text = Text::from(node.lesson.name.as_str()).style(style_from_status(&node.status));
                ListItem::from(text)
            });

        let list_widget = List::new(items).highlight_style(Style::default().reversed());

        if matches!(self.state, LessonEditFormState::NavigatingPrereqs) {
            frame.render_stateful_widget(list_widget, layout[1], &mut self.prerequisites.list_state_refcell().borrow_mut());
        } else {
            frame.render_widget(list_widget, layout[1]);
        }

        if matches!(self.state, LessonEditFormState::NavigatingPrereqs) {
            frame.render_widget(help, layout[2]);
        }
    }

    fn render_button(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);
        let layout = Layout::vertical([Constraint::Min(1), Constraint::Min(3), Constraint::Min(1)])
            .split(layout[1]);

        let style = if matches!(self.state, LessonEditFormState::Validating) {
            Style::default().reversed()
        } else {
            Style::default()
        };

        let button_area = layout[1];

        let button_widget = Paragraph::new("OK")
            .alignment(Alignment::Center)
            .block(Block::new().borders(Borders::all()))
            .style(style);

        frame.render_widget(button_widget, button_area);
    }
}
