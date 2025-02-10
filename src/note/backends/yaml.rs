use crate::note::{util, Note, NoteId, NotesBackend, NotesError};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
struct NoteData {
    pub content: String,
    pub html: String,
}

#[derive(Debug)]
struct State {
    notes: Vec<Note>,
}

#[derive(Clone, Debug)]
pub struct YamlBackend {
    path: PathBuf,
    state: Arc<Mutex<State>>,
}

struct YamlNote {
    timestamp: String,
    content: String,
    html: String,
}

impl NoteId {
    /// Returns the next note id.
    fn next(&self) -> NoteId {
        NoteId(self.0 + 1)
    }
}

impl YamlBackend {
    pub fn load(path: PathBuf) -> Self {
        let yaml_notes = if let Ok(content) = std::fs::read_to_string(&path) {
            content
                .split("\n\n---\n\n")
                .filter(|s| !s.trim().is_empty())
                .map(|block| {
                    let parts: Vec<&str> = block.splitn(2, '\n').collect();
                    let (timestamp, content) = match parts.as_slice() {
                        [timestamp, content] => {
                            (timestamp.trim().to_string(), content.trim().to_string())
                        }
                        _ => (util::local_timestamp(), block.to_string()),
                    };

                    let html = util::md_to_html(&content);
                    YamlNote {
                        timestamp,
                        content: content.to_string(),
                        html,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let notes = yaml_notes
            .into_iter()
            .enumerate()
            .map(|(idx, note)| Note {
                id: NoteId(idx),
                timestamp: note.timestamp,
                content: note.content,
                html: note.html,
            })
            .collect();

        YamlBackend {
            path,
            state: Arc::new(Mutex::new(State::new(notes))),
        }
    }

    fn append_note_to_file(&self, note: &Note) -> Result<(), NotesError> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(NotesError::Io)?;
        append_note_to_file(note, &mut file).map_err(NotesError::Io)
    }

    fn write_notes_to_file<'a, I>(&'a self, notes: I) -> Result<(), NotesError>
    where
        I: Iterator<Item = &'a Note>,
    {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(NotesError::Io)?;

        for note in notes {
            append_note_to_file(note, &mut file).map_err(NotesError::Io)?;
        }

        Ok(())
    }
}

impl NotesBackend for YamlBackend {
    fn create_note(&self, content: String) -> Result<Note, NotesError> {
        let data = NoteData {
            html: util::md_to_html(&content),
            content,
        };

        let mut state = self
            .state
            .lock()
            .map_err(|err| NotesError::Internal(err.to_string()))?;
        let note = state.append_note(data);
        self.append_note_to_file(&note)?;
        Ok(note)
    }

    fn update_note(&self, note_id: NoteId, content: String) -> Result<(), NotesError> {
        let data = NoteData {
            html: util::md_to_html(&content),
            content,
        };

        let mut state = self
            .state
            .lock()
            .map_err(|err| NotesError::Internal(err.to_string()))?;
        state.update_note(note_id, data);
        self.write_notes_to_file(state.notes())
    }

    fn delete_note(&self, note_id: NoteId) -> Result<(), NotesError> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| NotesError::Internal(err.to_string()))?;
        state.delete_note(note_id);
        self.write_notes_to_file(state.notes())
    }

    fn get_all_notes(&self) -> Vec<Note> {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => {
                tracing::error!("failed to lock state: {}", err);
                return Vec::new();
            }
        };
        state.notes().cloned().collect()
    }

    fn get_note_by_id(&self, id: NoteId) -> Option<Note> {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => {
                tracing::error!("failed to lock state: {}", err);
                return None;
            }
        };
        let mut notes = state.notes();
        notes.find(|n| n.id == id).cloned()
    }
}

impl State {
    pub fn new(notes: Vec<Note>) -> Self {
        State { notes }
    }

    fn next_note_id(&self) -> NoteId {
        self.notes
            .iter()
            .map(|n| n.id.next())
            .max()
            .unwrap_or_default()
    }

    pub fn append_note(&mut self, data: NoteData) -> Note {
        let note_id = self.next_note_id();
        let note = Note {
            id: note_id,
            timestamp: util::local_timestamp(),
            content: data.content,
            html: data.html,
        };
        self.notes.push(note.clone());
        note
    }

    pub fn update_note(&mut self, id: NoteId, data: NoteData) {
        if let Some(idx) = self.notes.iter().position(|n| n.id == id) {
            self.notes[idx].content = data.content;
            self.notes[idx].html = data.html;
        }
    }

    pub fn delete_note(&mut self, id: NoteId) {
        if let Some(idx) = self.notes.iter().position(|n| n.id == id) {
            self.notes.remove(idx);
        };
    }

    pub fn notes(&self) -> impl Iterator<Item = &Note> {
        self.notes.iter()
    }
}

fn append_note_to_file<F>(note: &Note, file: &mut F) -> Result<(), std::io::Error>
where
    F: std::io::Write,
{
    write!(file, "{}\n{}\n\n---\n\n", note.timestamp, note.content)
}
