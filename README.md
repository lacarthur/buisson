# `buisson`

`buisson` is a program to help learning complex subjects. It is similar in goals to Anki, but it differs from it in the way it goes about organizing knowledge: the main "object" of Anki is the flashcard, meant to represent a bite-sized piece of knowledge, that doesn't require context to remember. For instance, a chemical formula, the definition of a word, etc...
In contrast, `buisson`'s main "object" is the lesson, meant to represent a complex concept that can't necessaryly be split into flashcards: a complex mathematical theorem, an important historical event, `async` in Rust, etc...

Lessons also have a dependency system: a lesson can have prerequisites, that you are meant to study before it, and the program keeps track of those with a color code. Green is "Studied", orange is "Ready to study", and red is "Missing a prerequisite".

Note: `buisson` does not store the content of a lesson, and is merely a tool to schedule their study.

# Inside

`buisson`'s interface is made with `ratatui` and `crossterm`, using a loose component system. Program data is stored on disk, in a SQLite database.

# Example

![videobuisson](https://github.com/user-attachments/assets/ffd0c20d-ecc6-4468-b152-6adaf0d56cef)


# TODO

- add way to filter lesson list by status
- add tag system
- add calendar
- add possibility to undo
