use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect}, style::{Style, Stylize}, text::Line, widgets::{Block, Borders, Paragraph}, Frame
};

use crate::{
    components::textinput::TextInput,
    lessons::{GraphNode, LessonInfo, LessonStatus},
};

use super::{fuzzyfinder::{FuzzyFinder, FuzzyFinderAction}, node_list::{BasicNodeDisplayer, NodeListDisplay, NodeListStyle}, textinput::TextInputStyle};

#[derive(Debug)]
enum LessonEditFormState {
    EditingName,
    NavigatingPrereqs,
    AddingPrereq(FuzzyFinder),
    Validating
}

pub struct LessonEditForm {
    block_name: String,
    name_input: TextInput,
    prerequisites: NodeListDisplay<BasicNodeDisplayer>,
    all_current_lessons: Vec<GraphNode>,
    state: LessonEditFormState,
}

pub enum LessonEditFormAction {
    Noop,
    Terminate(Option<LessonInfo>),
}

impl LessonEditForm {
    pub fn new(block_name: String, lesson: LessonInfo, all_current_lessons: Vec<GraphNode>) -> Self {
        Self {
            block_name,
            name_input: TextInput::new(lesson.name),
            prerequisites: NodeListDisplay::new(lesson.depends_on.into_iter().map(|id| all_current_lessons[id as usize].clone()).collect()),
            all_current_lessons,
            state: LessonEditFormState::EditingName,
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> LessonEditFormAction {
        match &mut self.state {
            LessonEditFormState::EditingName => {
                match key.code {
                    KeyCode::Tab => self.state = LessonEditFormState::NavigatingPrereqs,
                    KeyCode::Esc => return LessonEditFormAction::Terminate(None),
                    _ => self.name_input.handle_key(key),
                }
            },
            LessonEditFormState::NavigatingPrereqs => {
                match key.code {
                    KeyCode::Char('a') => {
                        let current_prereq_ids = self.prerequisites.get_all_ids();
                        self.state = LessonEditFormState::AddingPrereq(
                            FuzzyFinder::new(
                                self.all_current_lessons.iter()
                                .filter(|node| !current_prereq_ids.contains(&node.lesson.get_id()))
                                .cloned()
                                .collect()
                            )
                        );
                    }
                    KeyCode::Esc => return LessonEditFormAction::Terminate(None),
                    KeyCode::Tab => self.state = LessonEditFormState::Validating,
                    KeyCode::BackTab => self.state = LessonEditFormState::EditingName,
                    _ => self.prerequisites.handle_key(key),
                }
            },
            LessonEditFormState::AddingPrereq(finder) => {
                match finder.handle_key(key) {
                    FuzzyFinderAction::Terminate(Some(id)) => {
                        self.prerequisites.add_new_node(self.all_current_lessons[id as usize].clone());
                        self.state = LessonEditFormState::NavigatingPrereqs;
                    },
                    FuzzyFinderAction::Terminate(None) => {
                        self.state = LessonEditFormState::NavigatingPrereqs;
                    }
                    FuzzyFinderAction::Noop => (),
                }
            },
            LessonEditFormState::Validating => {
                match key.code {
                    KeyCode::BackTab => self.state = LessonEditFormState::NavigatingPrereqs,
                    KeyCode::Enter => return LessonEditFormAction::Terminate(Some(self.to_lesson_info())),
                    KeyCode::Esc => return LessonEditFormAction::Terminate(None),
                    _ => (),
                }
            }
        }
        LessonEditFormAction::Noop
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let main_block = Block::new()
            .title(self.block_name.as_str())
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::new().bold());

        let main_block_inner = main_block.inner(area);
        frame.render_widget(main_block, area);

        let layout = Layout::vertical([Constraint::Min(3), Constraint::Percentage(100), Constraint::Min(5)])
            .split(main_block_inner);

        let name_input_area = layout[0];
        let prereqs_area = layout[1];
        let validating_button_area = layout[2];

        self.render_name_input(name_input_area, frame);

        self.render_prereq_list(prereqs_area, frame);

        self.render_button(validating_button_area, frame);

        if let LessonEditFormState::AddingPrereq(finder) = &self.state {
            let layout = Layout::vertical([Constraint::Percentage(15), Constraint::Percentage(70), Constraint::Percentage(15)])
                .split(prereqs_area);
            let layout = Layout::horizontal([Constraint::Percentage(10), Constraint::Percentage(80), Constraint::Percentage(10)])
                .split(layout[1]);

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
            TextInputStyle::default().block(name_input_block).display_cursor()
        } else {
            TextInputStyle::default().block(name_input_block)
        };

        self.name_input.render_with_style(area, frame, textinput_style);
    }

    fn render_button(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::horizontal([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(33)])
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
            LessonEditFormState::NavigatingPrereqs | LessonEditFormState::AddingPrereq(_)=> Style::default().bold(),
        };
        let prereq = Line::from("Prerequisites").style(style);
        let help = Line::from("Type 'a' to add a prerequisite");

        let layout = Layout::vertical([Constraint::Min(1), Constraint::Percentage(100), Constraint::Min(1)])
            .split(area);

        frame.render_widget(prereq, layout[0]);

        self.prerequisites.render_with_style(layout[1], frame, NodeListStyle::default());

        if let LessonEditFormState::NavigatingPrereqs = self.state {
            frame.render_widget(help, layout[2]);
        }
    }

    pub fn to_lesson_info(&self) -> LessonInfo {
        LessonInfo {
            name: self.name_input.to_str().into(),
            depends_on: self.prerequisites.get_all_ids(),
            status: LessonStatus::NotPracticed,
        }
    }
}
