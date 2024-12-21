use buisson_common::IOBackend;
use buisson_common::NodeStatus;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use ratatui::style::{Style, Stylize};
use rusqlite::Connection;
use std::{collections::HashMap, io::Cursor, path::Path};

use buisson_common::{Id, LessonInfo};

pub mod app;
pub mod components;

pub fn style_from_status(status: &NodeStatus) -> Style {
    match status {
        NodeStatus::Ok => Style::default().light_green(),
        NodeStatus::Pending => Style::default().light_yellow(),
        NodeStatus::MissingPrereq(_) => Style::default().light_red(),
    }
}

#[derive(Debug)]
pub struct SQLiteBackend {
    connection: rusqlite::Connection,
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

    fn query_lessons(&self) -> Result<HashMap<Id, LessonInfo>, Self::Error> {
        let mut stmt = self
            .connection
            .prepare("SELECT id, name, depends_on, status FROM lesson")?;

        let lessons = stmt
            .query_map([], |row| {
                let status_ron: String = row.get(3)?;
                Ok((
                    row.get(0)?,
                    LessonInfo {
                        name: row.get(1)?,
                        direct_prerequisites: ids_from_bytes(&row.get(2)?),
                        status: ron::from_str(&status_ron).unwrap(),
                        tags: vec![],
                    },
                ))
            })?
            .collect::<Result<HashMap<Id, LessonInfo>, _>>()?;

        Ok(lessons)
    }

    fn add_new_lesson(&self, id: Id, lesson: &LessonInfo) -> Result<(), Self::Error> {
        self.connection.execute(
            "INSERT INTO lesson VALUES (?1, ?2, ?3, ?4)",
            (
                id,
                &lesson.name,
                &ids_to_bytes(&lesson.direct_prerequisites),
                ron::to_string(&lesson.status).unwrap(),
            ),
        )?;
        Ok(())
    }

    fn update_existing_lesson(&self, id: Id, lesson: &LessonInfo) -> Result<(), Self::Error> {
        self.connection.execute(
            "UPDATE lesson SET name = ?2, depends_on = ?3, status = ?4 WHERE id = ?1",
            (
                id,
                &lesson.name,
                &ids_to_bytes(&lesson.direct_prerequisites),
                ron::to_string(&lesson.status).unwrap(),
            ),
        )?;
        Ok(())
    }

    fn remove_lesson(&self, id: Id) -> Result<(), Self::Error> {
        self.connection
            .execute("DELETE FROM lesson WHERE id = ?1", (&id,))?;
        Ok(())
    }
}
