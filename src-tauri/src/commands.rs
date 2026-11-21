use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;

use crate::catalog::{AddOutcome, Library, LibraryError, TagEdit, TrackRecord, TrackSummary};
use crate::error::{ErrorCode, PipelineError};
use crate::pipeline::probe::check_size;
use crate::pipeline::{process_bytes_with_timeout, sha256_hex};
use crate::r2::R2Storage;
use crate::settings::{self, Settings};

const MASK_PREFIX: &str = "••••";

pub struct AppState {
    pub settings_path: PathBuf,
    pub settings: RwLock<Settings>,
    pub library: RwLock<Option<Arc<Library>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadStage {
    Queued,
    Analysing,
    Uploading,
    Done,
    Duplicate,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadProgress {
    pub path: String,
    pub file_name: String,
    pub stage: UploadStage,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub track: Option<TrackSummary>,
}

impl UploadProgress {
    fn new(path: &Path, stage: UploadStage) -> Self {
        Self {
            path: path.to_string_lossy().to_string(),
            file_name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            stage,
            error_code: None,
            message: None,
            track: None,
        }
    }

    fn error(path: &Path, code: &str, message: String) -> Self {
        Self {
            error_code: Some(code.to_string()),
            message: Some(message),
            ..Self::new(path, UploadStage::Error)
        }
    }

    fn with_track(path: &Path, stage: UploadStage, track: TrackSummary) -> Self {
        Self {
            track: Some(track),
            ..Self::new(path, stage)
        }
    }
}

fn pipeline_code(e: &PipelineError) -> String {
    serde_json::to_value(e.code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "INTERNAL".to_string())
}

/// Reads, dedupes on the source hash, processes and adds one file; returns its terminal event.
async fn process_one(
    path: &Path,
    library: &Library,
    emit: &(dyn Fn(UploadProgress) + Send + Sync),
) -> UploadProgress {
    let read_path = path.to_path_buf();
    let read = tokio::task::spawn_blocking(move || -> Result<(String, Vec<u8>), PipelineError> {
        let cannot = |e: std::io::Error| {
            PipelineError::new(ErrorCode::Internal, format!("cannot read file: {e}"))
        };
        let len = std::fs::metadata(&read_path).map_err(cannot)?.len();
        check_size(len)?;
        let bytes = std::fs::read(&read_path).map_err(cannot)?;
        Ok((sha256_hex(&bytes), bytes))
    })
    .await;
    let (hash, bytes) = match read {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return UploadProgress::error(path, &pipeline_code(&e), e.message),
        Err(e) => return UploadProgress::error(path, "INTERNAL", e.to_string()),
    };
    match library.find_by_source(&hash).await {
        Ok(Some(existing)) => {
            return UploadProgress::with_track(path, UploadStage::Duplicate, existing);
        }
        Ok(None) => {}
        Err(e) => return UploadProgress::error(path, e.code(), e.to_string()),
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let processed = match process_bytes_with_timeout(bytes, stem).await {
        Ok(p) => p,
        Err(e) => return UploadProgress::error(path, &pipeline_code(&e), e.message),
    };
    emit(UploadProgress::new(path, UploadStage::Uploading));
    match library.add(processed).await {
        Ok(AddOutcome::Added(t)) => UploadProgress::with_track(path, UploadStage::Done, t),
        Ok(AddOutcome::Duplicate(t)) => {
            UploadProgress::with_track(path, UploadStage::Duplicate, t)
        }
        Err(e) => UploadProgress::error(path, e.code(), e.to_string()),
    }
}

/// Runs every file to a terminal event; a failure in one file never stops the batch.
pub async fn run_batch(
    paths: Vec<PathBuf>,
    library: Arc<Library>,
    emit: impl Fn(UploadProgress) + Send + Sync + 'static,
) {
    let emit = Arc::new(emit);
    for p in &paths {
        emit(UploadProgress::new(p, UploadStage::Queued));
    }
    let permits = std::thread::available_parallelism().map_or(2, |n| n.get());
    let sem = Arc::new(Semaphore::new(permits));
    let mut set = JoinSet::new();
    let mut by_task = HashMap::new();
    for path in paths {
        let (sem, library, emit) = (sem.clone(), library.clone(), emit.clone());
        let task_path = path.clone();
        let handle = set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore is never closed");
            emit(UploadProgress::new(&task_path, UploadStage::Analysing));
            let terminal = process_one(&task_path, &library, emit.as_ref()).await;
            emit(terminal);
        });
        by_task.insert(handle.id(), path);
    }
    while let Some(res) = set.join_next_with_id().await {
        if let Err(e) = res
            && let Some(p) = by_task.get(&e.id())
        {
            emit(UploadProgress::error(p, "INTERNAL", "internal error".to_string()));
        }
    }
}

fn not_configured() -> LibraryError {
    LibraryError::Validation("NOT_CONFIGURED".into())
}

async fn library_of(state: &AppState) -> Result<Arc<Library>, LibraryError> {
    state.library.read().await.clone().ok_or_else(not_configured)
}

async fn build_library(s: &Settings) -> Arc<Library> {
    let storage = R2Storage::new(s).await;
    Arc::new(Library::new(Arc::new(storage), s.public_base_url.clone()))
}

/// Builds the app state at startup from the settings file.
pub async fn init_state(settings_path: PathBuf) -> AppState {
    let s = settings::load(&settings_path).normalize();
    let library = if s.is_complete() {
        Some(build_library(&s).await)
    } else {
        None
    };
    AppState {
        settings_path,
        settings: RwLock::new(s),
        library: RwLock::new(library),
    }
}

/// Swaps a masked secret for the stored one, then normalizes.
async fn resolve_incoming(state: &AppState, mut s: Settings) -> Settings {
    if s.secret_access_key.starts_with(MASK_PREFIX) {
        s.secret_access_key = state.settings.read().await.secret_access_key.clone();
    }
    s.normalize()
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.read().await.masked())
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, s: Settings) -> Result<(), String> {
    let s = resolve_incoming(&state, s).await;
    settings::save(&state.settings_path, &s).map_err(|e| e.to_string())?;
    let library = if s.is_complete() {
        Some(build_library(&s).await)
    } else {
        None
    };
    *state.settings.write().await = s;
    *state.library.write().await = library;
    Ok(())
}

#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, s: Settings) -> Result<(), String> {
    let s = resolve_incoming(&state, s).await;
    R2Storage::new(&s)
        .await
        .test_connection()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tracks(state: State<'_, AppState>) -> Result<Vec<TrackSummary>, LibraryError> {
    library_of(&state).await?.list().await
}

#[tauri::command]
pub async fn get_track(
    state: State<'_, AppState>,
    id: String,
) -> Result<TrackRecord, LibraryError> {
    library_of(&state).await?.detail(&id).await
}

#[tauri::command]
pub async fn upload_files(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<(), String> {
    let library = library_of(&state)
        .await
        .map_err(|_| "NOT_CONFIGURED".to_string())?;
    let paths = paths.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn(run_batch(paths, library, move |e| {
        let _ = app.emit("upload-progress", e);
    }));
    Ok(())
}

#[tauri::command]
pub async fn update_track(
    state: State<'_, AppState>,
    id: String,
    edit: TagEdit,
    cover_path: Option<String>,
) -> Result<TrackSummary, LibraryError> {
    let library = library_of(&state).await?;
    let cover = match cover_path {
        Some(p) => Some(
            tokio::fs::read(p)
                .await
                .map_err(|_| LibraryError::Validation("cannot read cover file".into()))?,
        ),
        None => None,
    };
    library.update(&id, edit, cover).await
}

#[tauri::command]
pub async fn delete_tracks(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), LibraryError> {
    library_of(&state).await?.delete(&ids).await
}
