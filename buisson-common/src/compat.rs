use chrono::NaiveDate;
use std::collections::HashMap;
use serde::Serialize;
use serde::Deserialize;

use crate::Id;

/// The status of a lesson, independant of the runtime
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum LessonStatus {
    /// This lesson has never been practiced
    #[default]
    NotPracticed,
    /// For now, we consider this lesson completely acquired, but in the future, we'll want to
    /// spend more time on it.
    GoodEnough,
    /// This lesson has been practiced, to the level `level`, and the last practice session
    /// happened on `date`.
    Practiced { level: u32, last_practiced: NaiveDate, good_until: NaiveDate },
}

#[derive(Debug, Clone, Default)]
pub struct LessonInfo {
    pub name: String,
    /// The list of all prerequisite lessons, identified by their `id`.
    pub direct_prerequisites: Vec<Id>,
    pub status: LessonStatus,
    pub tags: Vec<String>,
}

pub trait IOBackend {
    type Error: std::fmt::Debug;
    fn query_lessons(&self) -> Result<HashMap<Id, LessonInfo>, Self::Error>;

    fn add_new_lesson(&self, id: Id, lesson: &LessonInfo) -> Result<(), Self::Error>;

    fn update_existing_lesson(&self, id: Id, lesson: &LessonInfo) -> Result<(), Self::Error>;

    fn remove_lesson(&self, id: Id) -> Result<(), Self::Error>;
}
