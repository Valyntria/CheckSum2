#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use checksum_core::{
    gpg, hash, jobs::Jobs, report, scan, Algorithm, Error, ErrorKind, HashResult, ScanEvent,
    ScanOptions, ScanReport,
};
use serde::Deserialize;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
use tauri::{State, Window};

struct AppState {
    jobs: Arc<Jobs>,
}

#[derive(Deserialize)]
struct HashRequest {
    path: String,
    algorithms: Vec<Algorithm>,
}
#[derive(Deserialize)]
struct ScanRequest {
    root: String,
    algorithms: Vec<Algorithm>,
    options: ScanOptions,
    manifest_path: Option<String>,
}
#[derive(Deserialize)]
struct GpgRequest {
    file_path: String,
    signature_path: String,
    expected_fingerprint: Option<String>,
}

async fn run_job<T: Send + 'static>(
    jobs: Arc<Jobs>,
    id: String,
    work: impl FnOnce(Arc<AtomicBool>) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let token = jobs.token(&id)?;
    let result = tauri::async_runtime::spawn_blocking(move || work(token)).await;
    jobs.finish(&id);
    result.map_err(|e| Error::new(ErrorKind::Io, format!("Worker failed: {e}")))?
}

#[tauri::command]
fn create_job(state: State<'_, AppState>) -> String {
    state.jobs.create()
}
#[tauri::command]
fn cancel_job(state: State<'_, AppState>, job_id: String) {
    state.jobs.cancel(&job_id);
}

#[tauri::command]
async fn calculate_checksums(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    request: HashRequest,
) -> Result<HashResult, Error> {
    run_job(state.jobs.clone(), job_id.clone(), move |cancel| {
        hash::hash_file(Path::new(&request.path), &request.algorithms, &cancel, |progress| {
            let _ = window.emit("job-event", serde_json::json!({ "job_id": job_id, "kind": "hash_progress", "payload": progress }));
        })
    }).await
}

#[tauri::command]
async fn scan_folder(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    request: ScanRequest,
) -> Result<ScanReport, Error> {
    run_job(state.jobs.clone(), job_id.clone(), move |cancel| {
        scan::scan(
            Path::new(&request.root),
            request.algorithms,
            request.options,
            request.manifest_path.as_deref().map(Path::new),
            &cancel,
            |event| {
                let (kind, payload) = match event {
                    ScanEvent::Progress(p) => ("scan_progress", serde_json::to_value(p)),
                    ScanEvent::Entry(e) => ("entry", serde_json::to_value(e)),
                };
                if let Ok(payload) = payload {
                    let _ = window.emit(
                        "job-event",
                        serde_json::json!({ "job_id": job_id, "kind": kind, "payload": payload }),
                    );
                }
            },
        )
    })
    .await
}

#[tauri::command]
async fn verify_gpg_signature(
    state: State<'_, AppState>,
    job_id: String,
    request: GpgRequest,
) -> Result<gpg::GpgResult, Error> {
    run_job(state.jobs.clone(), job_id, move |cancel| {
        gpg::verify(
            Path::new(&request.file_path),
            Path::new(&request.signature_path),
            request.expected_fingerprint.as_deref(),
            &cancel,
        )
    })
    .await
}

#[tauri::command]
async fn save_report(
    file_path: String,
    data: ScanReport,
    format: report::Format,
    algorithm: Option<Algorithm>,
    exclude_empty: bool,
) -> Result<(), Error> {
    tauri::async_runtime::spawn_blocking(move || {
        report::save(
            Path::new(&file_path),
            &data,
            format,
            algorithm,
            exclude_empty,
        )
    })
    .await
    .map_err(|e| Error::new(ErrorKind::Io, e.to_string()))?
}

#[tauri::command]
async fn path_kind(path: String) -> Result<&'static str, Error> {
    tauri::async_runtime::spawn_blocking(move || {
        let metadata = std::fs::metadata(path)?;
        Ok(if metadata.is_file() {
            "file"
        } else if metadata.is_dir() {
            "directory"
        } else {
            "other"
        })
    })
    .await
    .map_err(|e| Error::new(ErrorKind::Io, e.to_string()))?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(AppState {
            jobs: Arc::new(Jobs::default()),
        })
        .invoke_handler(tauri::generate_handler![
            create_job,
            cancel_job,
            calculate_checksums,
            scan_folder,
            verify_gpg_signature,
            save_report,
            path_kind
        ])
        .run(tauri::generate_context!())
        .expect("Could not start CheckSum");
}
