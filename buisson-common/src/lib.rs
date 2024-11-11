use std::collections::HashMap;

use chrono::{Days, NaiveDate};
use cli_log::debug;
use rand::{seq::IteratorRandom, Rng};
use serde::{Deserialize, Serialize};

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
    fn query_lessons(&self) -> Result<HashMap<Id, LessonInfo>, Self::Error>;

    fn add_new_lesson(&self, id: Id, lesson: &LessonInfo) -> Result<(), Self::Error>;

    fn update_existing_lesson(&self, id: Id, lesson: &LessonInfo) -> Result<(), Self::Error>;

    fn remove_lesson(&self, id: Id) -> Result<(), Self::Error>;
}

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
    Practiced { level: u32, date: NaiveDate },
}

impl LessonStatus {
    /// Whether or not a lesson is considered "known", irrespective of whether or not its
    /// prerequisites' status
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

/// A struct to represent a lesson in the app. Contains every information about the lesson that we
/// know.
#[derive(Debug, Clone, Default)]
pub struct LessonInfo {
    pub name: String,
    /// The list of all prerequisite lessons, identified by their `id`.
    pub direct_prerequisites: Vec<Id>,
    pub status: LessonStatus,
    pub tags: Vec<String>,
}

/// A runtime node of the graph structure. Contains a lesson and additional runtime info.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// The actual lesson represented by the `GraphNode`.
    pub lesson: LessonInfo,
    pub status: NodeStatus,
}

/// The main data struct of the program. It stores all of the lessons. Right now, the nodes are
/// indexed by the `id` of the lesson that they encapsulate, but this may change in the future.
#[derive(Debug)]
pub struct Graph<T: IOBackend> {
    nodes: HashMap<Id, GraphNode>,
    /// `children[id]` is the list of lessons that have lesson `id` as a prerequisite. This is kept
    /// in memory to help with updating the nodes at runtime. It is not stored to the disk and is
    /// instead computed at the start of the program and updated throughout
    children: HashMap<Id, Vec<Id>>,
    /// the next id to give to a newly created node.
    next_id: Id,

    io_backend: T,
}

impl<T: IOBackend> Graph<T> {
    /// create a new node in the graph, and update the relevant data structures inside. This is a
    /// public facing function, and should be able to be called without altering the correctness of
    /// the state of `self`. It also returns the Id of the newly created node.
    pub fn create_new_node(&mut self, lesson_info: LessonInfo) -> Id {
        let id = self.next_id;
        self.next_id += 1;

        for &parent in &lesson_info.direct_prerequisites {
            self.children.get_mut(&parent).unwrap().push(id);
        }
        let node_status = self.compute_node_status(&lesson_info.direct_prerequisites, &lesson_info.status);

        self.io_backend.add_new_lesson(id, &lesson_info).unwrap();

        self.children.insert(id, vec![]);
        self.nodes.insert(
            id,
            GraphNode {
                lesson: lesson_info,
                status: node_status,
            },
        );

        id
    }

    /// Delete node with id `id`. Keeps the internal state consistent, removes the `id` from
    /// prerequisite lists of other nodes.
    pub fn delete_node(&mut self, id: Id) {
        let node = self.nodes.remove(&id).unwrap();

        // we remove the id from the list of children of its prerequisites
        for prereq in node.lesson.direct_prerequisites {
            self.children
                .get_mut(&prereq)
                .unwrap()
                .retain(|&child| child != id);
        }

        // we remove the id from the direct prerequisites list of its children, and also do it in
        // the database
        let children = self.children.remove(&id).unwrap();
        for &child_id in &children {
            let child = self.nodes.get_mut(&child_id).unwrap();
            child
                .lesson
                .direct_prerequisites
                .retain(|&parent| parent != id);
            self.io_backend
                .update_existing_lesson(
                    child_id,
                    &child.lesson,
                )
                .expect("the database child update to work");
        }

        for &child_id in &children {
            self.update_node_status(child_id);
        }
        self.io_backend
            .remove_lesson(id)
            .expect("the database delete to work");
    }

    /// Compute the runtime status of the node with id `id` in the graph, and updates the current
    /// value. If the value has changed, calls itself on the children of the node whose status we
    /// just modified, as their status depends on the status of `id`.
    fn update_node_status(&mut self, id: Id) {
        let lesson_status = &self.nodes.get(&id).unwrap().lesson.status;
        let old_node_status = self.nodes.get(&id).unwrap().status.clone();

        let new_node_status = self.compute_node_status(
            &self.nodes.get(&id).unwrap().lesson.direct_prerequisites,
            lesson_status,
        );

        // if the status hasnt been updated, there is no need to propagate the change to its
        // children. If it has however, their status may change and we need to recursively call the
        // function.
        if old_node_status != new_node_status {
            self.nodes.get_mut(&id).unwrap().status = new_node_status;
            for &child in &self.children.get(&id).unwrap().clone() {
                self.update_node_status(child);
            }
        }
    }

    /// Edit the lesson with id `id`, replacing its info with `lesson_info`. This function also
    /// maintains the correctness of the state, by updating runtime info to reflect the new value
    /// for the lesson info.
    pub fn edit_node(&mut self, id: Id, lesson_info: LessonInfo) {
        // for a simple update of the parents/children relationship, we just wipe the slate clean
        // and then we rewrite everything with the updated values
        for &parent in &self.nodes.get(&id).unwrap().lesson.direct_prerequisites {
            self.children.get_mut(&parent).unwrap().retain(|&x| x != id);
        }
        for &parent in &lesson_info.direct_prerequisites {
            self.children.get_mut(&parent).unwrap().push(id);
        }

        self.io_backend
            .update_existing_lesson(
                id,
                &lesson_info,
            )
            .unwrap();

        self.nodes.get_mut(&id).unwrap().lesson.name = lesson_info.name;
        self.nodes.get_mut(&id).unwrap().lesson.direct_prerequisites =
            lesson_info.direct_prerequisites;
        self.nodes.get_mut(&id).unwrap().lesson.status = lesson_info.status;

        self.update_node_status(id);
    }

    /// Return the id of a lesson chosen uniformly among all pending lessons. In case there are
    /// no pending lessons, returns `None`.
    pub fn random_pending<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<Id> {
        self.nodes
            .iter()
            .filter(|(_, node)| matches!(node.status, NodeStatus::Pending))
            .choose(rng)
            .map(|(id, _)| *id)
    }

    /// Return an iterator with only the lessons whose name contain the string `search_request`.
    pub fn perform_search(&self, search_request: String) -> impl Iterator<Item = &GraphNode> {
        self.lessons_iter()
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
        let ret = builder.into_graph();
        Ok(ret)
    }

    /// Return an iterator of all lessons in the `Graph`
    pub fn lessons_iter(&self) -> impl Iterator<Item = &GraphNode> {
        self.nodes.values()
    }

    /// Return an owned `Vec` of all the ids of nodes in the graph
    pub fn get_ids(&self) -> Vec<Id> {
        self.nodes.keys().copied().collect()
    }

    /// Return the map between ids and nodes
    pub fn lessons(&self) -> &HashMap<Id, GraphNode> {
        &self.nodes
    }

    /// retrieve node with id `id`
    pub fn get(&self, id: Id) -> &GraphNode {
        self.nodes.get(&id).unwrap()
    }

    /// Return how many nodes are contained in the graph
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Return how many nodes are OK (i.e. don't need work)
    pub fn num_ok_nodes(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| node.status == NodeStatus::Ok)
            .count()
    }

    /// return whether or not `id1` has `id2` as a prerequisite (not necessarily direct)
    pub fn depends_on(&self, id1: Id, id2: Id) -> bool {
        if id1 == id2 {
            return true;
        }

        for &prereq_id in &self.nodes.get(&id1).unwrap().lesson.direct_prerequisites {
            if self.depends_on(prereq_id, id2) {
                return true;
            }
        }

        false
    }

    pub fn get_children(&self, id: &Id) -> &[Id] {
        self.children.get(id).unwrap()
    }
}

/// A struct used to construct `Graph`s. they are initialized by loading the lessons from the
/// database, and initializing all the statuses to `None`. Then, recursively, the `NodeStatus`es
/// are computed and memoized. Finally, a `Graph` object is produced, when all the `Option`s are
/// `Some`.
#[derive(Debug, Default)]
struct GraphBuilder<Backend: IOBackend> {
    lessons: HashMap<Id, (LessonInfo, Option<NodeStatus>)>,
    backend: Backend,
}

impl<Backend: IOBackend> GraphBuilder<Backend> {
    fn into_graph(mut self) -> Graph<Backend> {
        let mut max_id = None;
        self.resolve();
        let mut children = HashMap::new();
        // we do this so that lessons that don't have any children still have an empty vec instead
        // of no entry, which would cause a panic down the road
        for &id in self.lessons.keys() {
            children.insert(id, vec![]);
        }
        for (id, (lesson, _)) in &self.lessons {
            max_id = match max_id {
                None => Some(*id),
                Some(current_max) => Some(std::cmp::max(current_max, *id)),
            };
            for &parent in &lesson.direct_prerequisites {
                children
                    .entry(parent)
                    .and_modify(|list: &mut Vec<Id>| list.push(*id));
            }
        }

        Graph {
            next_id: match max_id {
                None => 0,
                Some(max_id) => max_id + 1,
            },
            nodes: self
                .lessons
                .into_iter()
                .map(|(id, (lesson, status))| {
                    (
                        id,
                        GraphNode {
                            lesson,
                            status: status.unwrap(),
                        },
                    )
                })
                .collect(),
            children,
            io_backend: self.backend,
        }
    }

    /// Retrieve the data from the database in `backend`
    fn load_from_database(backend: Backend) -> Result<Self, Backend::Error> {
        let lessons = backend.query_lessons()?;
        Ok(Self {
            lessons: lessons
                .into_iter()
                .map(|(id, lesson)| (id, (lesson, None)))
                .collect(),
            backend,
        })
    }

    /// this function is to be called recursivley, changing the stored status of the nodes as it
    /// computes it.
    fn get_status(&mut self, id: Id) -> NodeStatus {
        debug!("Calling `get_status` for lesson with id {}", id);
        debug!("lessons = {:?}", &self.lessons);
        if let Some(status) = &self.lessons.get(&id).unwrap().1 {
            return status.clone();
        }

        if let LessonStatus::GoodEnough = self.lessons.get(&id).unwrap().0.status {
            self.lessons.get_mut(&id).unwrap().1 = Some(NodeStatus::Ok);
            return NodeStatus::Ok;
        }

        let prereqs = self
            .lessons
            .get(&id)
            .unwrap()
            .0
            .direct_prerequisites
            .clone();
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
        let keys = self.lessons.keys().cloned().collect::<Vec<_>>();
        for i in keys {
            self.get_status(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyIOBackend {
        lessons: HashMap<Id, LessonInfo>,
    }

    impl IOBackend for DummyIOBackend {
        type Error = ();

        fn query_lessons(&self) -> Result<HashMap<Id, LessonInfo>, Self::Error> {
            Ok(self.lessons.clone())
        }

        fn add_new_lesson(&self, _id: Id, _lesson: &LessonInfo) -> Result<(), Self::Error> {
            Ok(())
        }

        fn update_existing_lesson(&self, _id: Id, _lesson: &LessonInfo) -> Result<(), Self::Error> {
            Ok(())
        }

        fn remove_lesson(&self, _id: Id) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn test_dummy_backend() -> DummyIOBackend {
        let lessons_vec = vec![
            LessonInfo {
                name: String::from("Test 0"),
                direct_prerequisites: vec![1],
                status: LessonStatus::NotPracticed,
                tags: vec![],
            },
            LessonInfo {
                name: String::from("Test 1"),
                direct_prerequisites: vec![],
                status: LessonStatus::GoodEnough,
                tags: vec![],
            },
            LessonInfo {
                name: String::from("Test 2"),
                direct_prerequisites: vec![1, 0, 3],
                status: LessonStatus::GoodEnough,
                tags: vec![],
            },
            LessonInfo {
                name: String::from("Test 3"),
                direct_prerequisites: vec![0],
                status: LessonStatus::NotPracticed,
                tags: vec![],
            },
            LessonInfo {
                name: String::from("Test 4"),
                direct_prerequisites: vec![2],
                status: LessonStatus::NotPracticed,
                tags: vec![],
            },
        ];

        let lessons = lessons_vec
            .into_iter()
            .enumerate()
            .map(|(id, lesson)| (id as u64, lesson))
            .collect();

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

    impl PartialEq for LessonInfo {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
                && self.direct_prerequisites == other.direct_prerequisites
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
                lesson: LessonInfo {
                    name: String::from("Test 0"),
                    direct_prerequisites: vec![1],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 1"),
                    direct_prerequisites: vec![],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 2"),
                    direct_prerequisites: vec![1, 0, 3],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 3"),
                    direct_prerequisites: vec![0],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::MissingPrereq(vec![0]),
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 4"),
                    direct_prerequisites: vec![2],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
        ];

        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(id, node)| (id as u64, node))
            .collect();

        assert_eq!(g.nodes, nodes)
    }

    #[test]
    fn test_node_adding() {
        let backend = test_dummy_backend();

        let mut g = Graph::get_from_database(backend).unwrap();

        let nodes = vec![
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 0"),
                    direct_prerequisites: vec![1],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 1"),
                    direct_prerequisites: vec![],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 2"),
                    direct_prerequisites: vec![1, 0, 3],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 3"),
                    direct_prerequisites: vec![0],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::MissingPrereq(vec![0]),
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 4"),
                    direct_prerequisites: vec![2],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 5"),
                    direct_prerequisites: vec![2],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 6"),
                    direct_prerequisites: vec![5, 2],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::MissingPrereq(vec![5]),
            },
        ];

        g.create_new_node(LessonInfo {
            name: String::from("Test 5"),
            direct_prerequisites: vec![2],
            status: LessonStatus::NotPracticed,
            tags: vec![],
        });

        g.create_new_node(LessonInfo {
            name: String::from("Test 6"),
            direct_prerequisites: vec![5, 2],
            status: LessonStatus::NotPracticed,
            tags: vec![],
        });

        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(id, node)| (id as u64, node))
            .collect();

        assert_eq!(g.nodes, nodes);
    }

    #[test]
    fn test_node_editing() {
        let backend = test_dummy_backend();

        let mut g = Graph::get_from_database(backend).unwrap();

        let nodes = vec![
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("TEST 0"),
                    direct_prerequisites: vec![],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 1"),
                    direct_prerequisites: vec![],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 2"),
                    direct_prerequisites: vec![1, 0, 3],
                    status: LessonStatus::GoodEnough,
                    tags: vec![],
                },
                status: NodeStatus::Ok,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 3"),
                    direct_prerequisites: vec![0],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
            GraphNode {
                lesson: LessonInfo {
                    name: String::from("Test 4"),
                    direct_prerequisites: vec![2],
                    status: LessonStatus::NotPracticed,
                    tags: vec![],
                },
                status: NodeStatus::Pending,
            },
        ];

        g.edit_node(
            0,
            LessonInfo {
                name: String::from("TEST 0"),
                direct_prerequisites: vec![],
                status: LessonStatus::GoodEnough,
                tags: vec![],
            },
        );

        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(id, node)| (id as u64, node))
            .collect();

        assert_eq!(g.nodes, nodes);
    }
}
