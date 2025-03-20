use rand::thread_rng;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    Frame,
};

use buisson_common::LessonStatus;

enum StudyEditorState {
    GoodEnough,
    NotPracticed,
    Practiced,
}

impl StudyEditor {
    fn to_lesson_status(&self) -> LessonStatus {
        match self.state {
            StudyEditorState::GoodEnough => LessonStatus::GoodEnough,
            StudyEditorState::NotPracticed => LessonStatus::NotPracticed,
            StudyEditorState::Practiced => LessonStatus::new_status_if_studied(self.step, &mut thread_rng())
        }
    }
}

pub struct StudyEditor {
    state: StudyEditorState,
    step: u32,
}

pub enum StudyEditorAction {
    Noop,
    Terminate(Option<LessonStatus>),
}

impl StudyEditor {
    pub fn new(status: LessonStatus) -> Self {
        let state = match &status {
            LessonStatus::NotPracticed | LessonStatus::Practiced { .. } => {
                StudyEditorState::Practiced
            }
            LessonStatus::GoodEnough => StudyEditorState::GoodEnough,
        };
        let step = match &status {
            LessonStatus::Practiced { level, .. } => level + 1,
            LessonStatus::NotPracticed | LessonStatus::GoodEnough => 0,
        };
        Self { state, step }
    }
    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let not_practiced_text = if let StudyEditorState::NotPracticed = self.state {
            Text::from("Not Practiced").style(Style::default().reversed())
        } else {
            Text::from("Not Practiced").style(Style::default())
        };

        let good_enough_text = if let StudyEditorState::GoodEnough = self.state {
            Text::from("Good Enough").style(Style::default().reversed())
        } else {
            Text::from("Good Enough").style(Style::default())
        };

        let practiced_text = if self.step == 0 {
            if let StudyEditorState::Practiced = self.state {
                Text::from(vec![
                    Line::from(Span::styled(
                        "Practiced (Step 0)",
                        Style::default().reversed(),
                    )),
                    Line::from(Span::raw("Practiced (Step 1)")),
                ])
            } else {
                Text::from(vec![
                    Line::from(Span::raw("Practiced (Step 0)")),
                    Line::from(Span::raw("Practiced (Step 1)")),
                ])
            }
        } else if let StudyEditorState::Practiced = self.state {
            Text::from(vec![
                Line::from(Span::raw(format!("Practiced (Step {})", self.step - 1))),
                Line::from(Span::styled(
                    format!("Practiced (Step {})", self.step),
                    Style::default().reversed(),
                )),
                Line::from(Span::raw(format!("Practiced (Step {})", self.step + 1))),
            ])
        } else {
            Text::from(vec![
                Line::from(Span::raw(format!("Practiced (Step {})", self.step - 1))),
                Line::from(Span::raw(format!("Practiced (Step {})", self.step))),
                Line::from(Span::raw(format!("Practiced (Step {})", self.step + 1))),
            ])
        };

        let layout = Layout::horizontal(Constraint::from_percentages([33, 33, 33])).split(area);

        let area_left = Layout::vertical(Constraint::from_mins([1, 1, 1])).split(layout[0])[1];

        let area_right = Layout::vertical(Constraint::from_mins([1, 1, 1])).split(layout[2])[1];

        let area_middle = if self.step == 0 {
            Layout::vertical(Constraint::from_mins([1, 2])).split(layout[1])[1]
        } else {
            layout[1]
        };

        frame.render_widget(not_practiced_text, area_left);
        frame.render_widget(practiced_text, area_middle);
        frame.render_widget(good_enough_text, area_right);
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> StudyEditorAction {
        match key.code {
            KeyCode::Char('l') | KeyCode::Tab => match self.state {
                StudyEditorState::GoodEnough => (),
                StudyEditorState::NotPracticed => self.state = StudyEditorState::Practiced,
                StudyEditorState::Practiced => self.state = StudyEditorState::GoodEnough,
            },
            KeyCode::Char('h') | KeyCode::BackTab => match self.state {
                StudyEditorState::GoodEnough => self.state = StudyEditorState::Practiced,
                StudyEditorState::NotPracticed => (),
                StudyEditorState::Practiced => self.state = StudyEditorState::NotPracticed,
            },
            KeyCode::Char('j') => {
                if let StudyEditorState::Practiced = self.state {
                    self.step += 1;
                }
            }
            KeyCode::Char('k') => {
                if let StudyEditorState::Practiced = self.state {
                    if self.step > 0 {
                        self.step -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                return StudyEditorAction::Terminate(Some(self.to_lesson_status()));
            }
            KeyCode::Esc => {
                return StudyEditorAction::Terminate(None);
            }
            _ => (),
        }
        StudyEditorAction::Noop
    }
}
