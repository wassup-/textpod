pub mod backends;
mod util;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct NoteId(pub usize);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub timestamp: String,
    pub content: String,
    pub html: String,
}

#[derive(Debug)]
pub enum NotesError {
    Io(std::io::Error),
    Internal(String),
}

pub trait NotesBackend {
    /// Creates a new note.
    fn create_note(&self, content: String) -> Result<Note, NotesError>;
    /// Updates an existing note.
    fn update_note(&self, note_id: NoteId, content: String) -> Result<(), NotesError>;
    /// Deletes an existing note.
    fn delete_note(&self, note_id: NoteId) -> Result<(), NotesError>;

    /// Returns all notes.
    fn get_all_notes(&self) -> Vec<Note>;
    /// Returns the note with the given id.
    fn get_note_by_id(&self, id: NoteId) -> Option<Note>;
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl fmt::Display for NotesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Internal(err) => write!(f, "internal error: {err}"),
        }
    }
}
