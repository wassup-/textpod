mod downloader;
mod note;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use clap::Parser;
use note::{backends::YamlBackend, NoteId, NotesBackend};
use std::{
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process,
};
use tower_http::services::ServeDir;
use tracing::{debug, error, info};
use tracing_subscriber;

const INDEX_HTML: &str = include_str!("index.html");
const FAVICON_SVG: &[u8] = include_bytes!("favicon.svg");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Change to DIR before doing anything
    #[arg(short = 'C', long, value_name = "DIR")]
    base_directory: Option<PathBuf>,
    /// Port number for the server
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
    /// Listen address for the server
    #[arg(short, long, default_value = "127.0.0.1")]
    listen: String,
    /// Save notes in FILE
    #[arg(short = 'f', long, value_name = "FILE", default_value = "notes.md")]
    notes_file: PathBuf,
}

#[derive(Clone)]
struct AppState {
    index_html: String,
    backend: YamlBackend,
    attachments_dir: PathBuf,
}

const CONTENT_LENGTH_LIMIT: usize = 500 * 1024 * 1024; // allow uploading up to 500mb files... overkill?

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    if let Some(path) = args.base_directory {
        if let Err(e) = env::set_current_dir(&path) {
            error!("could not change directory to {}: {e}", path.display());
            process::exit(1);
        }
    }

    let attachments_dir = env::current_dir().unwrap().join("attachments");

    if let Err(e) = fs::create_dir_all(&attachments_dir) {
        error!(
            "could not create attachments directory {}: {e}",
            attachments_dir.display()
        );
        process::exit(1);
    }

    let favicon = Base64Display::new(FAVICON_SVG, &STANDARD);
    let index_html = INDEX_HTML.replace(
        "{{FAVICON}}",
        format!("data:image/svg+xml;base64,{favicon}").as_str(),
    );

    let backend = YamlBackend::load(args.notes_file);
    let state = AppState {
        index_html,
        backend,
        attachments_dir: attachments_dir.clone(),
    };

    let app = Router::new()
        .route("/", get(routes::index))
        .route("/notes", get(routes::get_notes).post(routes::save_note))
        .route(
            "/notes/:id",
            get(routes::get_note_by_id).delete(routes::delete_note_by_id),
        ) // TODO PUT/PATCH
        .route("/upload", post(routes::upload_file))
        .layer(DefaultBodyLimit::max(CONTENT_LENGTH_LIMIT))
        .nest_service("/attachments", ServeDir::new(attachments_dir))
        .with_state(state);

    let server_details = format!("{}:{}", args.listen, args.port);
    let addr: SocketAddr = server_details
        .parse()
        .expect("Unable to parse socket address");
    info!("Starting server on http://{}", addr);

    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Server error: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to bind to address {}: {}", addr, e);
        }
    }
}

mod routes {

    use super::{
        downloader,
        note::{Note, NoteId, NotesBackend},
        AppState, DownloaderDelegate,
    };
    use axum::{
        extract::{Multipart, Path, State},
        http::StatusCode,
        response::{Html, IntoResponse},
        Json,
    };
    use tracing::{error, info};

    // route / (root)
    pub async fn index(State(state): State<AppState>) -> Html<String> {
        Html(state.index_html)
    }

    // GET /notes
    pub async fn get_notes(State(state): State<AppState>) -> Json<Vec<Note>> {
        Json(state.backend.get_all_notes())
    }

    // GET /notes/:id
    pub async fn get_note_by_id(
        State(state): State<AppState>,
        Path(id): Path<usize>,
    ) -> Result<impl IntoResponse, (StatusCode, String)> {
        let note_id = NoteId(id);
        let Some(note) = state.backend.get_note_by_id(note_id) else {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("request for non-existent note {note_id}"),
            ));
        };

        Ok(Json(note))
    }

    // DELETE /notes/:id
    pub async fn delete_note_by_id(
        State(state): State<AppState>,
        Path(id): Path<usize>,
    ) -> Result<impl IntoResponse, (StatusCode, String)> {
        let note_id = NoteId(id);
        if let Err(err) = state.backend.delete_note(note_id) {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()));
        }

        info!("Note deleted: {note_id}");

        // TODO return the deleted note, maybe?
        return Ok(StatusCode::NO_CONTENT);
    }

    // POST /notes
    pub async fn save_note(
        State(state): State<AppState>,
        Json(content): Json<String>,
    ) -> Result<(), StatusCode> {
        let mut content = content;
        // Replace "---" with "<hr>" in the content
        content = content.replace("---", "<hr>");

        let note = match state.backend.create_note(content) {
            Ok(note) => note,
            Err(err) => {
                error!("Failed to save note: {}", err);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        info!("Note created: {}", note.id);

        let links_to_download: Vec<String> = note
            .content
            .split_whitespace()
            .filter(|word| word.starts_with("+http"))
            .map(|s| s.to_owned())
            .collect();

        for link in links_to_download {
            // drop the '+' from the front of the link
            let link = link[1..].to_owned();

            let delegate = DownloaderDelegate {
                backend: state.backend.clone(),
                note_id: note.id,
                attachments_dir: state.attachments_dir.clone(),
            };

            tokio::spawn(async move {
                downloader::download_link(&link, delegate).await;
            });
        }

        Ok(())
    }

    // route POST /upload
    pub async fn upload_file(
        State(state): State<AppState>,
        mut multipart: Multipart,
    ) -> Result<Json<String>, StatusCode> {
        while let Some(field) = multipart.next_field().await.unwrap() {
            let name = field.file_name().unwrap().to_string();
            let data = field.bytes().await.unwrap();

            info!("Uploading file: {}", name);
            let original_path = state.attachments_dir.join(name);
            let mut counter = 1;

            let original_stem = original_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let original_ext = original_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            // Generate unique filename if already exists
            let mut path = original_path.clone();
            while path.exists() {
                // e.g: file-1.txt
                let new_name = if original_ext.is_empty() {
                    format!("{}-{}", original_stem, counter)
                } else {
                    format!("{}-{}.{}", original_stem, counter, original_ext)
                };

                path = original_path.parent().unwrap().join(new_name);
                counter += 1;
            }

            std::fs::write(&path, data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            info!("File saved as {}", path.display());
            return Ok(Json(format!(
                "/attachments/{}",
                path.file_name().unwrap().to_str().unwrap()
            )));
        }

        error!("Error uploading file");
        Err(StatusCode::BAD_REQUEST)
    }
}

struct DownloaderDelegate<B>
where
    B: NotesBackend,
{
    backend: B,
    note_id: NoteId,
    attachments_dir: PathBuf,
}

impl<B> downloader::Delegate for DownloaderDelegate<B>
where
    B: NotesBackend,
{
    fn attachments_dir(&self) -> &Path {
        &self.attachments_dir
    }

    fn update_local_link(&self, external_link: &str, local_path: &Path) {
        let Some(note) = self.backend.get_note_by_id(self.note_id) else {
            error!("attempt to update non-existent note {}", self.note_id);
            return;
        };

        let Ok(relative_path) = local_path.strip_prefix(&self.attachments_dir) else {
            error!(
                "attempt to update local link to inaccessible path {}",
                local_path.display()
            );
            return;
        };

        let local_link = format!("/attachments/{}", relative_path.display());
        let new_content = note.content.replace(
            &format!("+{external_link}"),
            &format!("{external_link} ([local copy]({local_link}))"),
        );

        if let Err(err) = self.backend.update_note(note.id, new_content) {
            error!("Failed to update note {}: {}", note.id, err);
        } else {
            debug!("Note updated: {}", note.id);
        }
    }
}
