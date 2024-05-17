use std::{io::Cursor, path::Path};

use chrono::{Days, NaiveDate};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

pub type Id = u64;

fn days_from_level(level: u32) -> u64 {
    match level {
        0 => 1,
        1 => 5,
        2 => 15,
        n => 2 * days_from_level(n - 1),
    }
}

/// The status of a lesson, independant of the runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LessonStatus {
    /// This lesson has never been practiced
    NotPracticed,
    /// For now, we consider this lesson completely acquired, but in the future, we'll want to
    /// spend more time on it.
    GoodEnough,
    /// This lesson has been practiced, to the level `level`, and the last practice session
    /// happened on `date`.
    Practiced { level: u32, date: NaiveDate },
}

impl LessonStatus {
    fn needs_work(&self) -> bool {
        match &self {
            LessonStatus::GoodEnough => false,
            LessonStatus::NotPracticed => true,
            LessonStatus::Practiced { level, date } => {
                let good_until = good_until(*level, *date);
                let today = chrono::offset::Local::now().date_naive();

                today >= good_until
            }
        }
    }
}

fn good_until(level: u32, date: NaiveDate) -> NaiveDate {
    date.checked_add_days(Days::new(days_from_level(level)))
        .unwrap()
}

/// The current status of a node. This is computed at runtime, and depends on the current date, for
/// instance.
#[derive(Debug, PartialEq, Clone)]
pub enum NodeStatus {
    /// The lesson does not need work.
    Ok,
    /// One of the lessons prerequisite needs work, independantly of whether or not this lesson
    /// needs work.
    MissingPrereq(Vec<Id>),
    /// This lesson needs work, and every one of its prerequisites are `Ok`.
    Pending,
}

#[derive(Debug, Clone)]
pub struct LessonInfo {
    pub name: String,
    pub depends_on: Vec<Id>,
    pub status: LessonStatus,
}

/// A lesson, meant to be serialized/deserialized, and storing informations that are independant of
/// runtime.
#[derive(Debug, Clone)]
pub struct Lesson {
    /// A unique `Id`, used to identify this lessson as a prerequisite of other lessons if
    /// necessary.
    id: Id,
    pub name: String,
    /// The list of all prerequisite lessons, identified by their `id`.
    pub depends_on: Vec<Id>,
    pub status: LessonStatus,
}

fn ids_to_bytes(ids: &Vec<Id>) -> Vec<u8> {
    let mut writer = vec![];

    for id in ids {
        writer.write_u64::<BigEndian>(*id).unwrap();
    }
    writer
}

fn ids_from_bytes(bytes: &Vec<u8>) -> Vec<Id> {
    let mut reader = Cursor::new(bytes);

    let mut output = vec![];

    while let Ok(id) = reader.read_u64::<BigEndian>() {
        output.push(id)
    }
    output
}

impl Lesson {
    pub fn get_id(&self) -> Id {
        self.id
    }
}

/// A runtime node of the graph structure.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// The actual lesson represented by the `GraphNode`.
    pub lesson: Lesson,
    pub status: NodeStatus,
}

/// The main data struct of the program. It stores all of the lessons. Right now, the nodes are
/// indexed by the `id` of the lesson that they encapsulate, but this may change in the future.
#[derive(Debug)]
pub struct Graph {
    nodes: Vec<GraphNode>,
    children: Vec<Vec<Id>>,
}

impl Graph {
    pub fn create_new_node(&mut self, lesson_info: LessonInfo) {
        let id = self.nodes.len() as u64;
        for &parent in &lesson_info.depends_on {
            self.children[parent as usize].push(id);
        }
        let lesson = Lesson {
            id,
            name: lesson_info.name,
            depends_on: lesson_info.depends_on,
            status: lesson_info.status,
        };
        let node_status = self.compute_node_status(&lesson.depends_on, &lesson.status);

        self.nodes.push(GraphNode {
            lesson,
            status: node_status,
        });
        self.children.push(vec![]);
    }

    fn update_lesson_status(&mut self, id: Id) {
        let lesson_status = &self.nodes[id as usize].lesson.status;
        let old_lesson_status = self.nodes[id as usize].status.clone();

        let new_lesson_status =
            self.compute_node_status(&self.nodes[id as usize].lesson.depends_on, lesson_status);

        if old_lesson_status != new_lesson_status {
            self.nodes[id as usize].status = new_lesson_status;
            for &child in &self.children[id as usize].clone() {
                self.update_lesson_status(child);
            }
        }
    }

    pub fn edit_node(&mut self, id: Id, lesson_info: LessonInfo) {
        for &parent in &self.nodes[id as usize].lesson.depends_on {
            self.children[parent as usize].retain(|&x| x != id);
        }
        for &parent in &lesson_info.depends_on {
            self.children[parent as usize].push(id);
        }
        self.nodes[id as usize].lesson.name = lesson_info.name;
        self.nodes[id as usize].lesson.depends_on = lesson_info.depends_on;
        self.nodes[id as usize].lesson.status = lesson_info.status;

        self.update_lesson_status(id);
    }

    pub fn perform_search(&self, search_request: String) -> impl Iterator<Item = &GraphNode> {
        self.lessons()
            .filter(move |&node| node.lesson.name.contains(&search_request))
    }

    fn compute_node_status(&self, prereqs: &[Id], lesson_status: &LessonStatus) -> NodeStatus {
        let mut missing_prereqs = vec![];
        for &prereq_id in prereqs {
            if self.nodes[prereq_id as usize].status != NodeStatus::Ok {
                missing_prereqs.push(prereq_id);
            }
        }
        if missing_prereqs.is_empty() {
            if lesson_status.needs_work() {
                NodeStatus::Pending
            } else {
                NodeStatus::Ok
            }
        } else {
            NodeStatus::MissingPrereq(missing_prereqs)
        }
    }

    pub fn get_from_database(database_path: &Path) -> rusqlite::Result<Self> {
        let builder = GraphBuilder::load_from_database(database_path)?;
        Ok(builder.into_graph())
    }

    pub fn lessons(&self) -> impl Iterator<Item = &GraphNode> {
        self.nodes.iter()
    }

    pub fn get(&self, id: usize) -> &GraphNode {
        &self.nodes[id]
    }

    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn num_ok_nodes(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| node.status == NodeStatus::Ok)
            .count()
    }
}

/// A struct used to construct `Graph`s. they are initialized by loading the lessons from the
/// database, and initializing all the statuses to `None`. Then, recursively, the `NodeStatus`es
/// are computed and memoized. Finally, a `Graph` object is produced, when all the `Option`s are
/// `Some`.
#[derive(Debug, Default)]
struct GraphBuilder {
    lessons: Vec<(Lesson, Option<NodeStatus>)>,
}

impl GraphBuilder {
    fn into_graph(mut self) -> Graph {
        self.resolve();
        let mut children = vec![vec![]; self.lessons.len()];
        for (lesson, _) in &self.lessons {
            for &parent in &lesson.depends_on {
                children[parent as usize].push(lesson.id);
            }
        }
        Graph {
            nodes: self
                .lessons
                .into_iter()
                .map(|(lesson, status)| GraphNode {
                    lesson,
                    status: status.unwrap(),
                })
                .collect(),
            children,
        }
    }

    fn create_database(database_path: &Path) -> rusqlite::Result<()> {
        let db = Connection::open(database_path)?;

        db.execute(
            "CREATE TABLE lesson (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                depends_on BLOB,
                status TEXT
            )",
            (),
        )?;

        Ok(())
    }

    fn load_from_database(database_path: &Path) -> rusqlite::Result<Self> {
        if std::fs::metadata(database_path).is_ok() {
            let db = Connection::open(database_path)?;

            let mut stmt = db.prepare("SELECT id, name, depends_on, status FROM lesson")?;

            let lessons = stmt
                .query_map([], |row| {
                    let status_ron: String = row.get(3)?;
                    Ok(Lesson {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        depends_on: ids_from_bytes(&row.get(2)?),
                        status: ron::from_str(&status_ron).unwrap(),
                    })
                })?
                .map(|val| (val.unwrap(), None))
                .collect::<Vec<_>>();

            Ok(GraphBuilder { lessons })
        } else {
            Self::create_database(database_path)?;
            Ok(Self::default())
        }
    }

    fn get_status(&mut self, id: Id) -> NodeStatus {
        if let Some(status) = &self.lessons[id as usize].1 {
            return status.clone();
        }

        let prereqs = self.lessons[id as usize].0.depends_on.clone();
        let mut missing_prereqs = vec![];
        for prereq_id in prereqs {
            if self.get_status(prereq_id) != NodeStatus::Ok {
                missing_prereqs.push(prereq_id);
            }
        }
        let status = if missing_prereqs.is_empty() {
            if self.lessons[id as usize].0.status.needs_work() {
                NodeStatus::Pending
            } else {
                NodeStatus::Ok
            }
        } else {
            NodeStatus::MissingPrereq(missing_prereqs)
        };

        self.lessons[id as usize].1 = Some(status.clone());

        status
    }

    fn resolve(&mut self) {
        for i in 0..self.lessons.len() {
            self.get_status(i as u64);
        }
    }
}