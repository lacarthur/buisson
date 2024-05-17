use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, ListItem, Paragraph},
    Frame,
};

use crate::{
    components::fuzzyfinder::{FuzzyFinder, FuzzyFinderAction},
    components::lesson_edit_form::LessonEditForm,
    components::node_list::{GraphNodeDisplayer, NodeListDisplay},
    lessons::{Graph, GraphNode, Id, LessonStatus},
    style_from_status,
};

enum AppState {
    BrowsingLessons,
    AddingNewLesson(LessonEditForm),
    EditingLesson(Id, LessonEditForm),
    Searching(FuzzyFinder),
    Quitting,
}

#[derive(Default)]
struct AppNodeDisplayer;

impl GraphNodeDisplayer for AppNodeDisplayer {
    fn render<'a>(&'a self, node: &'a GraphNode) -> ListItem<'_> {
        let text = Text::from(node.lesson.name.as_str());
        ListItem::new(text).style(style_from_status(&node.status))
    }
}

pub struct App {
    lessons: Graph,
    display_list: NodeListDisplay<AppNodeDisplayer>,
    state: AppState,
}

impl App {
    pub fn new() -> std::io::Result<Self> {
        let directories = xdg::BaseDirectories::with_prefix("buisson")?;
        let database_path = directories.get_data_home().join("lessons.sqlite");

        let lessons = Graph::get_from_database(&database_path).unwrap();
        let lesson_list_cache = lessons.lessons().cloned().collect();

        Ok(Self {
            lessons,
            display_list: NodeListDisplay::new(lesson_list_cache, "Lessons".into()),
            state: AppState::BrowsingLessons,
        })
    }

    fn update_cache(&mut self, search_request: Option<String>) {
        match search_request {
            Some(search_request) => self.display_list.update_nodes(
                self.lessons
                    .perform_search(search_request)
                    .cloned()
                    .collect(),
            ),
            None => self
                .display_list
                .update_nodes(self.lessons.lessons().cloned().collect()),
        }
    }

    pub fn is_quitting(&self) -> bool {
        matches!(self.state, AppState::Quitting)
    }

    pub fn render(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        let layout2 =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(layout[1]);
        let layout3 =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(layout[0]);

        let left_panel = layout[0];
        let left_panel_minus_bar = layout3[0];
        let right_panel = layout[1];
        let right_panel_minus_bar = layout2[0];
        let bottom_bar = layout3[1];

        match &self.state {
            AppState::Quitting => (),
            AppState::AddingNewLesson(lesson) => {
                self.display_list.render(left_panel_minus_bar, frame);
                lesson.render(right_panel_minus_bar, frame);
                self.render_status_line(bottom_bar, frame);
            }
            AppState::EditingLesson(_, lesson) => {
                self.display_list
                    .render_stateful(left_panel_minus_bar, frame);
                lesson.render(right_panel_minus_bar, frame);
                self.render_status_line(bottom_bar, frame);
            }
            AppState::BrowsingLessons => {
                self.display_list
                    .render_stateful(left_panel_minus_bar, frame);
                self.render_lesson_display(right_panel_minus_bar, frame);
                self.render_status_line(bottom_bar, frame);
            }
            AppState::Searching(search_input) => {
                self.render_help(right_panel, frame);
                search_input.render(left_panel, frame);
            }
        }
    }

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
        text.extend(node.lesson.depends_on.iter().map(|id| {
            let prereq_node = self.lessons.get(*id as usize);
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
            .block(block)
            .style(Style::new().white());

        frame.render_widget(widget, area);
    }

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

    fn render_lesson_display(&self, area: Rect, frame: &mut Frame<'_>) {
        if let Some(node) = self.selected_node() {
            self.render_node_display(area, frame, node);
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

    fn selected_node(&self) -> Option<&GraphNode> {
        self.display_list
            .selected_id()
            .map(|id| self.lessons.get(id as usize))
    }

    pub fn handle_key(&mut self, key: &KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match &mut self.state {
            AppState::BrowsingLessons => match key.code {
                KeyCode::Char('q') => self.state = AppState::Quitting,
                KeyCode::Char('a') => {
                    self.state = AppState::AddingNewLesson(LessonEditForm::new(
                        "Add New Lesson".into(),
                        String::new(),
                    ))
                }
                KeyCode::Char('s') => {
                    self.state = AppState::Searching(FuzzyFinder::new(
                        self.lessons.lessons().cloned().collect(),
                    ))
                }
                KeyCode::Char('e') => {
                    if let Some(currently_selected) = self.selected_node() {
                        let form = LessonEditForm::new(
                            "Edit Lesson".into(),
                            currently_selected.lesson.name.clone(),
                        );
                        self.state =
                            AppState::EditingLesson(currently_selected.lesson.get_id(), form);
                    }
                }
                _ => self.display_list.handle_key(key),
            },
            AppState::AddingNewLesson(event_name) => match key.code {
                KeyCode::Esc => self.state = AppState::BrowsingLessons,
                KeyCode::Enter => {
                    self.lessons.create_new_node(event_name.to_lesson_info());
                    self.update_cache(None);
                    self.state = AppState::BrowsingLessons;
                }
                _ => event_name.handle_key(key),
            },
            AppState::EditingLesson(id, lesson) => match key.code {
                KeyCode::Esc => self.state = AppState::BrowsingLessons,
                KeyCode::Enter => {
                    self.lessons.edit_node(*id, lesson.to_lesson_info());
                    self.update_cache(None);
                    self.state = AppState::BrowsingLessons;
                }
                _ => lesson.handle_key(key),
            },
            AppState::Searching(finder) => {
                if let FuzzyFinderAction::Terminate(id) = finder.handle_key(key) {
                    self.state = AppState::BrowsingLessons;
                    if let Some(id) = id {
                        self.display_list.select(id);
                    }
                }
            }
            AppState::Quitting => (),
        }
    }

    pub fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            self.handle_key(key);
        }
    }
}
