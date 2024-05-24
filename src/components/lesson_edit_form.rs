use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    components::textinput::TextInput,
    lessons::{GraphNode, Id, LessonInfo, LessonStatus},
};

use super::{
    fuzzyfinder::{FuzzyFinder, FuzzyFinderAction},
    node_list::{BasicNodeDisplayer, NodeListDisplay, NodeListStyle},
    textinput::TextInputStyle,
};

#[derive(Debug)]
enum LessonEditFormState {
    EditingName,
    NavigatingPrereqs,
    AddingPrereq(FuzzyFinder),
    Validating,
}

pub enum FormType {
    NewLesson,
    EditLesson(Id),
}

pub struct LessonEditForm {
    form_type: FormType,
    name_input: TextInput,
    prerequisites: NodeListDisplay<BasicNodeDisplayer>,
    /// does the form actually need to own this? I don't know, something to investigate. It certainly
    /// doesn't modify it.
    all_current_lessons: Vec<GraphNode>,
    state: LessonEditFormState,
    lesson_status: LessonStatus,
}

/// return true if id1 has id2 as a prereq somewhere.
fn depends_on(id1: Id, id2: Id, all_lessons: &[GraphNode]) -> bool {
    if id1 == id2 {
        return true;
    }

    for &prereq_id in &all_lessons[id1 as usize].lesson.depends_on {
        if depends_on(prereq_id, id2, all_lessons) {
            return true;
        }
    }

    false
}

pub enum LessonEditFormAction {
    Noop,
    Terminate(Option<LessonInfo>),
}

impl LessonEditForm {
    pub fn new(
        form_type: FormType,
        lesson: LessonInfo,
        all_current_lessons: Vec<GraphNode>,
    ) -> Self {
        Self {
            form_type,
            name_input: TextInput::new(lesson.name),
            prerequisites: NodeListDisplay::new(
                lesson
                    .depends_on
                    .into_iter()
                    .map(|id| all_current_lessons[id as usize].clone())
                    .collect(),
            ),
            all_current_lessons,
            state: LessonEditFormState::EditingName,
            lesson_status: lesson.status,
        }
    }

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
                    let current_prereq_ids = self.prerequisites.get_all_ids();
                    self.state = LessonEditFormState::AddingPrereq(FuzzyFinder::new(
                        self.all_current_lessons
                            .iter()
                            .filter(|node| {
                                if let FormType::EditLesson(id) = self.form_type {
                                    if depends_on(
                                        node.lesson.get_id(),
                                        id,
                                        &self.all_current_lessons,
                                    ) {
                                        return false;
                                    }
                                }
                                !current_prereq_ids.contains(&node.lesson.get_id())
                            })
                            .cloned()
                            .collect(),
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
                _ => self.prerequisites.handle_key(key),
            },
            LessonEditFormState::AddingPrereq(finder) => match finder.handle_key(key) {
                FuzzyFinderAction::Terminate(Some(id)) => {
                    self.prerequisites
                        .add_new_node(self.all_current_lessons[id as usize].clone());
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

    fn block_name(&self) -> &str {
        match self.form_type {
            FormType::NewLesson => "Add New Lesson",
            FormType::EditLesson(_) => "Edit Lesson",
        }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let main_block = Block::new()
            .title(self.block_name())
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

        self.render_prereq_list(prereqs_area, frame);

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
            finder.render(layout[1], frame);
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

        let textinput_style = if let LessonEditFormState::EditingName = self.state {
            TextInputStyle::default()
                .block(name_input_block)
                .display_cursor()
        } else {
            TextInputStyle::default().block(name_input_block)
        };

        self.name_input
            .render_with_style(area, frame, textinput_style);
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

        let style = if let LessonEditFormState::Validating = self.state {
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

    fn render_prereq_list(&self, area: Rect, frame: &mut Frame<'_>) {
        let style = match &self.state {
            LessonEditFormState::EditingName | LessonEditFormState::Validating => Style::default(),
            LessonEditFormState::NavigatingPrereqs | LessonEditFormState::AddingPrereq(_) => {
                Style::default().bold()
            }
        };
        let prereq = Line::from("Prerequisites").style(style);
        let help = Line::from("Type 'a' to add a prerequisite");

        let layout = Layout::vertical([
            Constraint::Min(1),
            Constraint::Percentage(100),
            Constraint::Min(1),
        ])
        .split(area);

        frame.render_widget(prereq, layout[0]);

        self.prerequisites
            .render_with_style(layout[1], frame, NodeListStyle::default());

        if let LessonEditFormState::NavigatingPrereqs = self.state {
            frame.render_widget(help, layout[2]);
        }
    }

    pub fn to_lesson_info(&self) -> LessonInfo {
        LessonInfo {
            name: self.name_input.to_str().into(),
            depends_on: self.prerequisites.get_all_ids(),
            status: self.lesson_status.clone(),
        }
    }
}
