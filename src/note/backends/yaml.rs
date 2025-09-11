use crate::note::{util, Html, Markdown, Note, NoteId, NotesBackend, NotesError};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
struct NoteData {
    pub content: Markdown,
    pub html: Html,
}

#[derive(Debug)]
struct State {
    notes: Vec<Note>,
}

#[derive(Clone, Debug)]
pub struct YamlBackend {
    path: Option<PathBuf>,
    state: Arc<Mutex<State>>,
}

struct YamlNote {
    timestamp: String,
    content: String,
}

impl NoteId {
    /// Returns the next note id.
    fn next(&self) -> NoteId {
        NoteId(self.0 + 1)
    }
}

impl YamlBackend {
    #[cfg(test)]
    pub fn test(notes: Vec<Note>) -> Self {
        YamlBackend {
            path: None,
            state: Arc::new(Mutex::new(State::new(notes))),
        }
    }

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

                    let content: Markdown = content.parse().unwrap();
                    YamlNote {
                        timestamp,
                        content: content.to_string(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let notes = yaml_notes
            .into_iter()
            .enumerate()
            .map(|(idx, note)| {
                let content: Markdown = note.content.parse().unwrap();

                Note {
                    id: NoteId(idx),
                    timestamp: note.timestamp,
                    html: content.to_html(),
                    content,
                }
            })
            .collect();

        YamlBackend {
            path: Some(path),
            state: Arc::new(Mutex::new(State::new(notes))),
        }
    }

    fn append_note_to_file(&self, note: &Note) -> Result<(), NotesError> {
        let Some(path) = &self.path else {
            return Ok(());
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(NotesError::Io)?;
        append_note_to_file(note, &mut file).map_err(NotesError::Io)
    }

    fn write_notes_to_file<'a, I>(&'a self, notes: I) -> Result<(), NotesError>
    where
        I: Iterator<Item = &'a Note>,
    {
        let Some(path) = &self.path else {
            return Ok(());
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(NotesError::Io)?;

        for note in notes {
            append_note_to_file(note, &mut file).map_err(NotesError::Io)?;
        }

        Ok(())
    }
}

impl NotesBackend for YamlBackend {
    fn create_note(&self, content: Markdown) -> Result<Note, NotesError> {
        let data = NoteData {
            html: content.to_html(),
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

    fn update_note(&self, note_id: NoteId, content: Markdown) -> Result<(), NotesError> {
        let data = NoteData {
            html: content.to_html(),
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

#[cfg(test)]
mod tests {

    #[test]
    fn test_append_note_to_file() {
        let mut output = Vec::new();

        append_note_to_file(
            &Note {
                id: NoteId(0),
                timestamp: "foo".to_owned(),
                content: "bar".parse().unwrap(),
                html: "baz".parse().unwrap(),
            },
            &mut output,
        )
        .unwrap();

        assert_eq!(output, "foo\nbar\n\n---\n\n".as_bytes());
    }

    use super::*;
}
