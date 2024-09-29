use std::collections::HashMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use rand::{rngs::ThreadRng, thread_rng};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::{
    components::{
        fuzzyfinder::{FuzzyFinder, FuzzyFinderAction}, lesson_edit_form::{LessonEditForm, LessonEditFormAction}, node_list::NodeList, study_editor::{StudyEditor, StudyEditorAction}
    },
    lessons::{Graph, GraphNode, Id, LessonInfo, LessonStatus, SQLiteBackend},
    style_from_status,
};

/// The state of the main application
enum AppState {
    BrowsingLessons,
    AddingNewLesson(LessonEditForm),
    EditingLesson(Id, LessonEditForm),
    Studying(Id, StudyEditor),
    Searching(FuzzyFinder),
    Quitting,
}

#[derive(Debug)]
pub enum AppError {
    IOError(std::io::Error),
    SQLiteError(rusqlite::Error),
    XDGError(xdg::BaseDirectoriesError),
}

pub struct App {
    lessons: Graph<SQLiteBackend>,
    main_list: NodeList,
    state: AppState,
    rng: ThreadRng,
}

#[derive(Debug, Clone)]
pub struct Context<'a> {
    pub lessons: &'a HashMap<Id, GraphNode>,
}

impl App {
    pub fn new() -> Result<Self, AppError> {
        let directories =
            xdg::BaseDirectories::with_prefix("buisson").map_err(AppError::XDGError)?;
        let data_path = directories.get_data_home();
        std::fs::create_dir_all(data_path).map_err(AppError::IOError)?;
        let database_path = directories.get_data_home().join("lessons.sqlite");

        let backend = SQLiteBackend::open(&database_path).map_err(AppError::SQLiteError)?;

        let lessons = Graph::get_from_database(backend).map_err(AppError::SQLiteError)?;
        let lesson_ids = lessons.lessons_iter().map(|node| node.lesson.get_id()).collect();

        Ok(Self {
            lessons,
            main_list: NodeList::new(lesson_ids),
            state: AppState::BrowsingLessons,
            rng: thread_rng(),
        })
    }

    fn get_context(&self) -> Context<'_> {
        Context {
            lessons: self.lessons.lessons()
        }
    }

    pub fn is_quitting(&self) -> bool {
        matches!(self.state, AppState::Quitting)
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let main_layout = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        let left_panel = main_layout[0];
        let right_panel = main_layout[1];

        let right_panel_layout =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(right_panel);
        let left_panel_layout =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(left_panel);

        let left_panel_minus_bar = left_panel_layout[0];
        let right_panel_minus_bar = right_panel_layout[0];
        let bottom_bar = left_panel_layout[1];

        let layout = Layout::vertical([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(left_panel_minus_bar);
        let layout = Layout::horizontal([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(layout[1]);

        let fuzzy_finder_area = layout[1];

        self.render_lessons_list(left_panel_minus_bar, frame);
        self.render_status_line(bottom_bar, frame);

        match &self.state {
            AppState::Quitting => (),
            AppState::AddingNewLesson(lesson) => {
                lesson.render(self.get_context(), right_panel_minus_bar, frame);
            }
            AppState::EditingLesson(_, lesson) => {
                lesson.render(self.get_context(),right_panel_minus_bar, frame);
            }
            AppState::BrowsingLessons => {
                self.render_side_panel(right_panel_minus_bar, frame);
            }
            AppState::Searching(search_input) => {
                frame.render_widget(Clear, fuzzy_finder_area);
                self.render_help(right_panel_minus_bar, frame);
                search_input.render(self.get_context(), fuzzy_finder_area, frame);
            }
            AppState::Studying(_, study_editor) => {
                let horizontal_area = Layout::horizontal(Constraint::from_percentages([30, 40, 30])).split(area)[1];
                let top_padding = (horizontal_area.height - 5)/2;
                let bottom_padding = horizontal_area.height - 5 - top_padding;
                let vertical_area = Layout::vertical(Constraint::from_mins([top_padding, 5, bottom_padding])).split(horizontal_area)[1];

                let block = Block::new().title("Study")
                    .borders(Borders::ALL)
                    .border_style(Style::default().bold());

                let study_editor_area = block.inner(vertical_area);

                frame.render_widget(Clear, vertical_area);
                frame.render_widget(block, vertical_area);
                study_editor.render(study_editor_area, frame);
            }
        }
    }

    /// Renders information about `node`.
    fn render_node_display(&self, area: Rect, frame: &mut Frame<'_>, node: &GraphNode) {
        let step_text = match node.lesson.status {
            LessonStatus::GoodEnough => String::from("Step : Known"),
            LessonStatus::NotPracticed => String::from("Step : Never Studied"),
            LessonStatus::Practiced { level, date: _ } => format!("Step : {}", level),
        };
        let style = style_from_status(&node.status);
        let mut text = vec![
            Line::default(),
            Line::from(vec![Span::raw(step_text)]),
            Line::default(),
            Line::from(vec![Span::raw("Prerequisites: ")]),
        ];

        text.extend(node.lesson.direct_prerequisites.iter().map(|id| {
            let prereq_node = self.lessons.get(*id);
            Line::from(vec![Span::styled(
                &prereq_node.lesson.name,
                style_from_status(&prereq_node.status),
            )])
        }));

        let block = Block::new()
            .title(node.lesson.name.as_str())
            .title_alignment(Alignment::Center)
            .border_style(style.bold())
            .borders(Borders::ALL);

        let widget = Paragraph::new(text)
            .style(Style::new().white());
        
        let inner = block.inner(area);

        let layout = Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(inner);

        frame.render_widget(block, area);

        frame.render_widget(widget, layout[0]);

        frame.render_widget(Text::from("Type 'e' to edit this lesson"), layout[1]);
    }

    /// renders help to `area`. Things like keybindings, etc...
    fn render_help(&self, area: Rect, frame: &mut Frame<'_>) {
        let block = Block::new()
            .title("Help")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::new().bold());

        let help_text = Paragraph::new("Type 'q' to quit")
            .block(block)
            .style(Style::new().white());

        frame.render_widget(help_text, area);
    }

    fn render_side_panel(&self, area: Rect, frame: &mut Frame<'_>) {
        if let Some(id) = self.main_list.currently_selected_id() {
            self.render_node_display(area, frame, self.lessons.get(id));
        } else {
            self.render_help(area, frame);
        }
    }

    fn render_status_line(&self, area: Rect, frame: &mut Frame<'_>) {
        let num_ok_lessons = self.lessons.num_ok_nodes();
        let num_lessons = self.lessons.num_nodes();
        let percent_ok_lessons = (num_ok_lessons as f64 / num_lessons as f64) * 100.0;

        frame.render_widget(
            Text::from(format!(
                " OK Lessons : {}/{} ({:.2}%)",
                num_ok_lessons, num_lessons, percent_ok_lessons
            )),
            area,
        );
    }

    fn render_lessons_list(&self, area: Rect, frame: &mut Frame<'_>) {
        let border_style = match &self.state {
            AppState::BrowsingLessons | AppState::Searching(_) => Style::default().bold(),
            _ => Style::default(),
        };
        let block = Block::new()
            .title(Line::from("Lessons").alignment(Alignment::Center))
            .borders(Borders::ALL)
            .style(border_style);

        let list_widget= List::new(self.main_list.ids().iter()
            .map(|id| {
                let node = self.lessons.get(*id);
                let text = Text::from(node.lesson.name.as_str());
                ListItem::new(text).style(style_from_status(&node.status))
            })
        ).block(block)
        .highlight_style(Style::default().reversed());

        match self.state {
            AppState::BrowsingLessons | AppState::EditingLesson(_, _) | AppState::Studying(_, _) => {
                frame.render_stateful_widget(list_widget, area, &mut self.main_list.list_state_refcell().borrow_mut());
            }
            _ => {
                frame.render_widget(list_widget, area);
            }
        }
    }

    pub fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            self.handle_key(key);
        }
    }
}

// input handling code
impl App {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match &mut self.state {
            AppState::BrowsingLessons => self.handle_key_browsing(key),
            AppState::AddingNewLesson(event_name) => match event_name.handle_key(key) {
                LessonEditFormAction::Terminate(Some(lesson_info)) => {
                    let id = self.lessons.create_new_node(lesson_info);
                    self.main_list.push(id);
                    self.state = AppState::BrowsingLessons;
                }
                LessonEditFormAction::Terminate(None) => self.state = AppState::BrowsingLessons,
                LessonEditFormAction::Noop => (),
            },
            AppState::EditingLesson(id, lesson) => match lesson.handle_key(key) {
                LessonEditFormAction::Terminate(Some(lesson_info)) => {
                    self.lessons.edit_node(*id, lesson_info);
                    self.state = AppState::BrowsingLessons;
                }
                LessonEditFormAction::Terminate(None) => self.state = AppState::BrowsingLessons,
                LessonEditFormAction::Noop => (),
            },
            AppState::Searching(finder) => {
                if let FuzzyFinderAction::Terminate(id) = finder.handle_key(key) {
                    self.state = AppState::BrowsingLessons;
                    if let Some(id) = id {
                        self.main_list.select(id);
                    }
                }
            }
            AppState::Studying(id, study_editor) => {
                match study_editor.handle_key(key) {
                    StudyEditorAction::Terminate(Some(lesson_status)) => {
                        let name = self.lessons.get(*id).lesson.name.clone();
                        let direct_prerequisites = self.lessons.get(*id).lesson.direct_prerequisites.clone();
                        self.lessons.edit_node(*id, LessonInfo { name, direct_prerequisites, status: lesson_status });
                        self.state = AppState::BrowsingLessons;
                    }
                    StudyEditorAction::Terminate(None) => self.state = AppState::BrowsingLessons,
                    StudyEditorAction::Noop => (),
                }
            }
            AppState::Quitting => (),
        }
    }

    fn handle_key_browsing(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.state = AppState::Quitting,
            KeyCode::Char('a') => {
                self.state = AppState::AddingNewLesson(LessonEditForm::new(
                    self.lessons.lessons().iter()
                    .map(|(id,node)| (*id, node.lesson.to_lesson_info())).
                    collect(),
                    LessonInfo::default(),
                ))
            }
            KeyCode::Char('/') => {
                self.state = AppState::Searching(FuzzyFinder::new(
                    self.lessons.lessons().iter().map(|(id, node)| (*id, node.lesson.to_lesson_info())).collect(),
                ))
            }
            KeyCode::Char('e') => {
                if let Some(currently_selected) = self.main_list.currently_selected_id() {
                    let form = LessonEditForm::new(
                        self.lessons.lessons().iter()
                        .filter(|(&id, _)| !self.lessons.depends_on(id, currently_selected))
                        .map(|(id, node)| (*id, node.lesson.to_lesson_info()))
                        .collect(),
                        self.lessons.get(currently_selected).lesson.to_lesson_info(),
                    );
                    self.state =
                        AppState::EditingLesson(currently_selected, form);
                }
            }
            KeyCode::Char('l') => {
                if let Some(currently_selected_id) = self.main_list.currently_selected_id() {
                    let status = self.lessons.lessons().get(&currently_selected_id).unwrap().lesson.status;

                    self.state = AppState::Studying(currently_selected_id, StudyEditor::new(status));
                }
            }
            KeyCode::Char('r') => {
                if let Some(node) = self.lessons.random_pending(&mut self.rng) {
                    self.main_list.select(node.lesson.get_id());
                }
            }
            _ => self.main_list.handle_key(key),
        }
    }
}
