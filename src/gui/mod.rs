mod routes;
mod types;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::audio::AudioBuffer;

#[derive(rust_embed::RustEmbed)]
#[folder = "gui/dist/"]
pub(crate) struct Assets;

pub struct AppState {
    pub buffer: Option<AudioBuffer>,
    pub file_path: Option<String>,
    pub format: Option<String>,
    pub cleaned_buffer: Option<AudioBuffer>,
    pub cleaned_file_path: Option<String>,
    pub cleaned_format: Option<String>,
    /// Temp file paths to clean up when state is dropped
    pub temp_paths: Vec<std::path::PathBuf>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            buffer: None,
            file_path: None,
            format: None,
            cleaned_buffer: None,
            cleaned_file_path: None,
            cleaned_format: None,
            temp_paths: Vec::new(),
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        for path in &self.temp_paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

pub async fn start_server(port: u16, no_open: bool) -> Result<()> {
    let state: SharedState = Arc::new(RwLock::new(AppState::new()));
    let app = routes::create_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("Polez GUI running at http://localhost:{port}");

    if !no_open {
        let _ = open::that(format!("http://localhost:{port}"));
    }

    axum::serve(listener, app).await?;
    Ok(())
}
