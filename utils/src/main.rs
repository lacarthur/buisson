use buisson_common::IOBackend as b;
use buisson_common::compat::IOBackend;

fn days_from_level(level: u32) -> u64 {
    match level {
        0 => 1,
        1 => 5,
        2 => 15,
        n => 2 * days_from_level(n - 1),
    }
}

fn lesson_v1_to_v2(input_lesson: buisson_common::LessonInfo) -> buisson_common::compat::LessonInfo {
    let new_status = match input_lesson.status {
        buisson_common::LessonStatus::NotPracticed => buisson_common::compat::LessonStatus::NotPracticed,
        buisson_common::LessonStatus::GoodEnough => buisson_common::compat::LessonStatus::GoodEnough,
        buisson_common::LessonStatus::Practiced { level, date } => {
            let good_until = date + chrono::Days::new(days_from_level(level));
            buisson_common::compat::LessonStatus::Practiced { level, last_practiced: date, good_until }
        },
    };

    buisson_common::compat::LessonInfo {
        name: input_lesson.name,
        direct_prerequisites: input_lesson.direct_prerequisites,
        status: new_status,
        tags: vec![],
    }
}

fn main() {
    let old_path = std::path::PathBuf::from("old_lessons.sqlite");
    let new_path = std::path::PathBuf::from("new_lessons.sqlite");
    let old_backend = buisson_database::SQLiteBackend::open(&old_path).unwrap();
    let new_backend = buisson_database::next::SQLiteBackend::open(&new_path).unwrap();

    let lessons = old_backend.query_lessons().unwrap();

    for (id, lesson) in lessons {
        new_backend.add_new_lesson(id, &lesson_v1_to_v2(lesson)).unwrap();
    }
}
