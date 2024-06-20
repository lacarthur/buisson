use std::{collections::HashMap, io::Cursor, path::Path};

use chrono::{Days, NaiveDate};
use rand::{seq::IteratorRandom, Rng};
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

pub trait IOBackend {
    type Error: std::fmt::Debug;
    fn query_lessons(&self) -> Result<HashMap<Id, Lesson>, Self::Error>;

    fn add_new_lesson(&self, lesson: &Lesson) -> Result<(), Self::Error>;

    fn update_existing_lesson(&self, lesson: &Lesson) -> Result<(), Self::Error>;
}

pub struct SQLiteBackend {
    connection: rusqlite::Connection,
}

impl SQLiteBackend {
    fn create_database(database_path: &Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(database_path)?;

        connection.execute(
            "CREATE TABLE lesson (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                depends_on BLOB,
                status TEXT
            )",
            (),
        )?;

        Ok(Self { connection })
    }

    pub fn open(database_path: &Path) -> rusqlite::Result<Self> {
        if std::fs::metadata(database_path).is_ok() {
            let connection = Connection::open(database_path)?;

            Ok(Self { connection })
        } else {
            Self::create_database(database_path)
        }
    }
}

impl IOBackend for SQLiteBackend {
    type Error = rusqlite::Error;

    fn query_lessons(&self) -> Result<HashMap<Id, Lesson>, Self::Error> {
        let mut stmt = self
            .connection
            .prepare("SELECT id, name, depends_on, status FROM lesson")?;

        let lessons = stmt
            .query_map([], |row| {
                let status_ron: String = row.get(3)?;
                Ok((row.get(0)?, Lesson {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    depends_on: ids_from_bytes(&row.get(2)?),
                    status: ron::from_str(&status_ron).unwrap(),
                }))
            })?
            .collect::<Result<HashMap<Id, Lesson>, _>>()?;

        Ok(lessons)
    }

    fn add_new_lesson(&self, lesson: &Lesson) -> Result<(), Self::Error> {
        self.connection.execute(
            "INSERT INTO lesson VALUES (?1, ?2, ?3, ?4)",
            (
                &lesson.id,
                &lesson.name,
                &ids_to_bytes(&lesson.depends_on),
                ron::to_string(&lesson.status).unwrap(),
            ),
        )?;
        Ok(())
    }

    fn update_existing_lesson(&self, lesson: &Lesson) -> Result<(), Self::Error> {
        self.connection.execute(
            "UPDATE lesson SET name = ?2, depends_on = ?3, status = ?4 WHERE id = ?1",
            (
                &lesson.id,
                &lesson.name,
                &ids_to_bytes(&lesson.depends_on),
                ron::to_string(&lesson.status).unwrap(),
            ),
        )?;
        Ok(())
    }
}

/// The status of a lesson, independant of the runtime
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum LessonStatus {
    /// This lesson has never been practiced
    #[default]
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

/// the date that a lesson is considered "known", given that it was last practiced on `date` to
/// level `level`.
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

#[derive(Debug, Clone, Default)]
pub struct LessonInfo {
    pub name: String,
    /// The list of all prerequisite lessons, identified by their `id`.
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

/// used to serialize the ids of the prerequisite lessons.
fn ids_to_bytes(ids: &Vec<Id>) -> Vec<u8> {
    let mut writer = vec![];

    for id in ids {
        writer.write_u64::<BigEndian>(*id).unwrap();
    }
    writer
}

/// used to deserialize the ids of the prerequisite lessons.
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

    pub fn to_lesson_info(&self) -> LessonInfo {
        LessonInfo {
            name: self.name.clone(),
            depends_on: self.depends_on.clone(),
            status: self.status.clone(),
        }
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
pub struct Graph<T: IOBackend> {
    nodes: HashMap<Id, GraphNode>,
    /// `children[id]` is the list of lessons that have lesson `id` as a prerequisite. This is kept
    /// in memory to help with updating the nodes at runtime. It is not stored to the disk and is
    /// instead computed at the start of the program
    children: HashMap<Id, Vec<Id>>,
    next_id: Id,

    io_backend: T,
}

impl<T: IOBackend> Graph<T> {
    /// create a new node in the graph, and update the relevant data structures inside. This is a
    /// public facing function, and should be able to be called without altering the correctness of
    /// the state of `self`.
    pub fn create_new_node(&mut self, lesson_info: LessonInfo) {
        let id = self.next_id;
        self.next_id += 1;

        for &parent in &lesson_info.depends_on {
            self.children.get_mut(&parent).unwrap().push(id);
        }
        let lesson = Lesson {
            id,
            name: lesson_info.name,
            depends_on: lesson_info.depends_on,
            status: lesson_info.status,
        };
        let node_status = self.compute_node_status(&lesson.depends_on, &lesson.status);

        self.io_backend.add_new_lesson(&lesson).unwrap();

        self.children.insert(lesson.id, vec![]);
        self.nodes.insert(lesson.id, GraphNode {
            lesson,
            status: node_status,
        });
    }

    /// this function is called when a node is edited. It is useful if a lesson has a new
    /// prerequisite, its status may need updating. It is only the runtime status though.
    fn update_lesson_status(&mut self, id: Id) {
        let lesson_status = &self.nodes.get(&id).unwrap().lesson.status;
        let old_lesson_status = self.nodes.get(&id).unwrap().status.clone();

        let new_lesson_status =
            self.compute_node_status(&self.nodes.get(&id).unwrap().lesson.depends_on, lesson_status);

        // if the status hasnt been updated, there is no need to propagate the change to its
        // children. If it has however, their status may change and we need to recursively call the
        // function.
        if old_lesson_status != new_lesson_status {
            self.nodes.get_mut(&id).unwrap().status = new_lesson_status;
            for &child in &self.children.get(&id).unwrap().clone() {
                self.update_lesson_status(child);
            }
        }
    }

    pub fn edit_node(&mut self, id: Id, lesson_info: LessonInfo) {
        for &parent in &self.nodes.get(&id).unwrap().lesson.depends_on {
            self.children.get_mut(&parent).unwrap().retain(|&x| x != id);
        }
        for &parent in &lesson_info.depends_on {
            self.children.get_mut(&parent).unwrap().push(id);
        }

        self.io_backend
            .update_existing_lesson(&Lesson {
                id,
                name: lesson_info.name.clone(),
                depends_on: lesson_info.depends_on.clone(),
                status: lesson_info.status.clone(),
            })
            .unwrap();

        self.nodes.get_mut(&id).unwrap().lesson.name = lesson_info.name;
        self.nodes.get_mut(&id).unwrap().lesson.depends_on = lesson_info.depends_on;
        self.nodes.get_mut(&id).unwrap().lesson.status = lesson_info.status;

        self.update_lesson_status(id);
    }

    pub fn random_pending<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<&GraphNode> {
        self.nodes.values().filter(|node| matches!(node.status, NodeStatus::Pending))
            .choose(rng)
    }

    pub fn perform_search(&self, search_request: String) -> impl Iterator<Item = &GraphNode> {
        self.lessons()
            .filter(move |&node| node.lesson.name.contains(&search_request))
    }

    /// this function is called when the statuses of all the prereqs have been computed.
    fn compute_node_status(&self, prereqs: &[Id], lesson_status: &LessonStatus) -> NodeStatus {
        if let LessonStatus::GoodEnough = lesson_status {
            return NodeStatus::Ok;
        }
        let mut missing_prereqs = vec![];
        for &prereq_id in prereqs {
            if self.nodes.get(&prereq_id).unwrap().status != NodeStatus::Ok {
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

    pub fn get_from_database(backend: T) -> Result<Self, T::Error> {
        let builder = GraphBuilder::load_from_database(backend)?;
        Ok(builder.into_graph())
    }

    pub fn lessons(&self) -> impl Iterator<Item = &GraphNode> {
        self.nodes.values()
    }

    pub fn get(&self, id: Id) -> &GraphNode {
        self.nodes.get(&id).unwrap()
    }

    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn num_ok_nodes(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| node.status == NodeStatus::Ok)
            .count()
    }

    /// return whether or not `id1` has `id2` as a prerequisite
    pub fn depends_on(&self, id1: Id, id2: Id) -> bool {
        if id1 == id2 {
            return true;
        }

        for &prereq_id in &self.nodes.get(&id1).unwrap().lesson.depends_on {
            if self.depends_on(prereq_id, id2) {
                return true;
            }
        }

        false
    }
}

/// A struct used to construct `Graph`s. they are initialized by loading the lessons from the
/// database, and initializing all the statuses to `None`. Then, recursively, the `NodeStatus`es
/// are computed and memoized. Finally, a `Graph` object is produced, when all the `Option`s are
/// `Some`.
#[derive(Debug, Default)]
struct GraphBuilder<Backend: IOBackend> {
    lessons: HashMap<Id, (Lesson, Option<NodeStatus>)>,
    backend: Backend,
}

impl<Backend: IOBackend> GraphBuilder<Backend> {
    fn into_graph(mut self) -> Graph<Backend> {
        let mut max_id = 0;
        self.resolve();
        let mut children = HashMap::new();
        for (id, (lesson, _)) in &self.lessons {
            max_id = std::cmp::max(max_id, *id);
            for &parent in &lesson.depends_on {
                children.entry(parent).and_modify(|list: &mut Vec<Id>| list.push(lesson.id)).or_insert(vec![lesson.id]);
            }
        }
        
        Graph {
            next_id: max_id + 1,
            nodes: self
                .lessons
                .into_iter()
                .map(|(id, (lesson, status))| 
                    (id, 
                    GraphNode {
                    lesson,
                    status: status.unwrap(),
                }))
                .collect(),
            children,
            io_backend: self.backend,
        }
    }

    fn load_from_database(backend: Backend) -> Result<Self, Backend::Error> {
        let lessons = backend.query_lessons()?;
        Ok(Self {
            lessons: lessons.into_iter().map(|(id, lesson)| (id, (lesson, None))).collect(),
            backend,
        })
    }

    /// this function is to be called recursivley, changing the stored status of the nodes as it
    /// computes it.
    fn get_status(&mut self, id: Id) -> NodeStatus {
        if let Some(status) = &self.lessons.get(&id).unwrap().1 {
            return status.clone();
        }

        if let LessonStatus::GoodEnough = self.lessons.get(&id).unwrap().0.status {
            self.lessons.get_mut(&id).unwrap().1 = Some(NodeStatus::Ok);
            return NodeStatus::Ok;
        }

        let prereqs = self.lessons.get(&id).unwrap().0.depends_on.clone();
        let mut missing_prereqs = vec![];
        for prereq_id in prereqs {
            if self.get_status(prereq_id) != NodeStatus::Ok {
                missing_prereqs.push(prereq_id);
            }
        }
        let status = if missing_prereqs.is_empty() {
            if self.lessons.get(&id).unwrap().0.status.needs_work() {
                NodeStatus::Pending
            } else {
                NodeStatus::Ok
            }
        } else {
            NodeStatus::MissingPrereq(missing_prereqs)
        };

        self.lessons.get_mut(&id).unwrap().1 = Some(status.clone());

        status
    }

    /// ensures every status is being computed
    fn resolve(&mut self) {
        for i in 0..self.lessons.len() {
            self.get_status(i as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyIOBackend {
        lessons: HashMap<Id, Lesson>,
    }

    impl IOBackend for DummyIOBackend {
        type Error = ();

        fn query_lessons(&self) -> Result<HashMap<Id, Lesson>, Self::Error> {
            Ok(self.lessons.clone())
        }

        fn add_new_lesson(&self, _lesson: &Lesson) -> Result<(), Self::Error> {
            Ok(())
        }

        fn update_existing_lesson(&self, _lesson: &Lesson) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn test_dummy_backend() -> DummyIOBackend {
        let lessons_vec = vec![
            Lesson {
                id: 0,
                name: String::from("Test 0"),
                depends_on: vec![1],
                status: LessonStatus::NotPracticed,
            },
            Lesson {
                id: 1,
                name: String::from("Test 1"),
                depends_on: vec![],
                status: LessonStatus::GoodEnough,
            },
            Lesson {
                id: 2,
                name: String::from("Test 2"),
                depends_on: vec![1, 0, 3],
                status: LessonStatus::GoodEnough,
            },
            Lesson {
                id: 3,
                name: String::from("Test 3"),
                depends_on: vec![0],
                status: LessonStatus::NotPracticed,
            },
            Lesson {
                id: 4,
                name: String::from("Test 4"),
                depends_on: vec![2],
                status: LessonStatus::NotPracticed,
            },
        ];

        let lessons = lessons_vec.into_iter().map(|lesson| (lesson.id, lesson)).collect();

        DummyIOBackend { lessons }
    }

    impl PartialEq for LessonStatus {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (
                    Self::Practiced {
                        level: l_level,
                        date: l_date,
                    },
                    Self::Practiced {
                        level: r_level,
                        date: r_date,
                    },
                ) => l_level == r_level && l_date == r_date,
                _ => core::mem::discriminant(self) == core::mem::discriminant(other),
            }
        }
    }

    impl PartialEq for Lesson {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
                && self.name == other.name
                && self.depends_on == other.depends_on
                && self.status == other.status
        }
    }

    impl PartialEq for GraphNode {
        fn eq(&self, other: &Self) -> bool {
            self.lesson == other.lesson && self.status == other.status
        }
    }

    #[test]
    fn test_graph_creation() {
        let backend = test_dummy_backend();

        let g = Graph::get_from_database(backend).unwrap();

        let nodes = vec![
            GraphNode {
                lesson: Lesson {
                    id: 0,
                    name: String::from("Test 0"),
                    depends_on: vec![1],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: Lesson {
                    id: 1,
                    name: String::from("Test 1"),
                    depends_on: vec![],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 2,
                    name: String::from("Test 2"),
                    depends_on: vec![1, 0, 3],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 3,
                    name: String::from("Test 3"),
                    depends_on: vec![0],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::MissingPrereq(vec![0]),
            },
            GraphNode {
                lesson: Lesson {
                    id: 4,
                    name: String::from("Test 4"),
                    depends_on: vec![2],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
        ];

        let nodes = nodes.into_iter().map(|node| (node.lesson.id, node)).collect();

        assert_eq!(g.nodes, nodes)
    }

    #[test]
    fn test_node_adding() {
        let backend = test_dummy_backend();

        let mut g = Graph::get_from_database(backend).unwrap();

        let nodes = vec![
            GraphNode {
                lesson: Lesson {
                    id: 0,
                    name: String::from("Test 0"),
                    depends_on: vec![1],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: Lesson {
                    id: 1,
                    name: String::from("Test 1"),
                    depends_on: vec![],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 2,
                    name: String::from("Test 2"),
                    depends_on: vec![1, 0, 3],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 3,
                    name: String::from("Test 3"),
                    depends_on: vec![0],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::MissingPrereq(vec![0]),
            },
            GraphNode {
                lesson: Lesson {
                    id: 4,
                    name: String::from("Test 4"),
                    depends_on: vec![2],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: Lesson {
                    id: 5,
                    name: String::from("Test 5"),
                    depends_on: vec![2],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: Lesson {
                    id: 6,
                    name: String::from("Test 6"),
                    depends_on: vec![5, 2],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::MissingPrereq(vec![5]),
            },
        ];

        g.create_new_node(LessonInfo {
            name: String::from("Test 5"),
            depends_on: vec![2],
            status: LessonStatus::NotPracticed,
        });

        g.create_new_node(LessonInfo {
            name: String::from("Test 6"),
            depends_on: vec![5, 2],
            status: LessonStatus::NotPracticed,
        });

        let nodes = nodes.into_iter().map(|node| (node.lesson.id, node)).collect();

        assert_eq!(g.nodes, nodes);
    }

    #[test]
    fn test_node_editing() {
        let backend = test_dummy_backend();

        let mut g = Graph::get_from_database(backend).unwrap();

        let nodes = vec![
            GraphNode {
                lesson: Lesson {
                    id: 0,
                    name: String::from("TEST 0"),
                    depends_on: vec![],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 1,
                    name: String::from("Test 1"),
                    depends_on: vec![],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 2,
                    name: String::from("Test 2"),
                    depends_on: vec![1, 0, 3],
                    status: LessonStatus::GoodEnough,
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: Lesson {
                    id: 3,
                    name: String::from("Test 3"),
                    depends_on: vec![0],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: Lesson {
                    id: 4,
                    name: String::from("Test 4"),
                    depends_on: vec![2],
                    status: LessonStatus::NotPracticed,
                },
                status: NodeStatus::Pending,
            },
        ];

        g.edit_node(
            0,
            LessonInfo {
                name: String::from("TEST 0"),
                depends_on: vec![],
                status: LessonStatus::GoodEnough,
            },
        );

        let nodes = nodes.into_iter().map(|node| (node.lesson.id, node)).collect();

        assert_eq!(g.nodes, nodes);
    }
}
