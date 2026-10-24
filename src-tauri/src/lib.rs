use mass_print_core::{
    AddReport, BatchPrintReport, DocumentQueue, DocumentStatus, JobOutcome, LpPrintBackend,
    PrintBackend, PrintSettings, PrinterInfo,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use uuid::Uuid;

#[derive(Clone, Default)]
struct RuntimeState {
    queue: Arc<Mutex<DocumentQueue>>,
    canceled: Arc<AtomicBool>,
    submitted_job_ids: Arc<Mutex<Vec<String>>>,
    backend: LpPrintBackend,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueueSnapshot {
    documents: Vec<mass_print_core::DocumentEntry>,
    total_known_pages: u32,
    unknown_page_count: usize,
}

#[tauri::command]
fn queue_snapshot(state: State<'_, RuntimeState>) -> Result<QueueSnapshot, String> {
    let queue = state.queue.lock().map_err(|error| error.to_string())?;
    Ok(QueueSnapshot {
        documents: queue.documents().to_vec(),
        total_known_pages: queue.total_known_pages(),
        unknown_page_count: queue.unknown_page_count(),
    })
}

#[tauri::command]
fn add_files(paths: Vec<String>, state: State<'_, RuntimeState>) -> Result<AddReport, String> {
    let mut queue = state.queue.lock().map_err(|error| error.to_string())?;
    Ok(queue.add_paths(paths))
}

#[tauri::command]
fn remove_document(id: Uuid, state: State<'_, RuntimeState>) -> Result<(), String> {
    state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .remove(id)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_queue(state: State<'_, RuntimeState>) -> Result<(), String> {
    state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .clear();
    Ok(())
}

#[tauri::command]
fn reorder_document(
    id: Uuid,
    destination: usize,
    state: State<'_, RuntimeState>,
) -> Result<(), String> {
    state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .reorder(id, destination)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn move_document(id: Uuid, delta: isize, state: State<'_, RuntimeState>) -> Result<(), String> {
    state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .move_by(id, delta)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_printers(state: State<'_, RuntimeState>) -> Result<Vec<PrinterInfo>, String> {
    let backend = state.backend.clone();
    tauri::async_runtime::spawn_blocking(move || {
        backend.printers().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn print_all(
    settings: PrintSettings,
    state: State<'_, RuntimeState>,
) -> Result<BatchPrintReport, String> {
    let documents = state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .documents()
        .to_vec();
    let backend = state.backend.clone();
    let canceled = Arc::clone(&state.canceled);
    let submitted_job_ids = Arc::clone(&state.submitted_job_ids);
    let queue = Arc::clone(&state.queue);
    canceled.store(false, Ordering::SeqCst);
    submitted_job_ids
        .lock()
        .map_err(|error| error.to_string())?
        .clear();

    tauri::async_runtime::spawn_blocking(move || {
        let report = backend
            .print_batch_observing(&documents, &settings, &canceled, |job_id| {
                if let Ok(mut submitted) = submitted_job_ids.lock() {
                    submitted.push(job_id.to_string());
                }
            })
            .map_err(|error| error.to_string())?;
        let mut queue = queue.lock().map_err(|error| error.to_string())?;
        for job in &report.jobs {
            let status = match job.outcome {
                JobOutcome::Submitted => DocumentStatus::Submitted,
                JobOutcome::Failed => DocumentStatus::Failed,
                JobOutcome::Canceled => DocumentStatus::Canceled,
            };
            let _ = queue.set_status(job.document_id, status, Some(job.message.clone()));
        }
        Ok(report)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn cancel_printing(state: State<'_, RuntimeState>) -> Result<(), String> {
    state.canceled.store(true, Ordering::SeqCst);
    let job_ids = state
        .submitted_job_ids
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    state.backend.cancel_jobs(&job_ids);
    Ok(())
}

fn add_existing_paths(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let state = app.state::<RuntimeState>();
    let valid_paths: Vec<_> = paths.into_iter().filter(|path| path.is_file()).collect();
    if !valid_paths.is_empty() {
        if let Ok(mut queue) = state.queue.lock() {
            queue.add_paths(valid_paths);
        }
        let _ = app.emit("queue-changed", ());
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn paths_from_arguments(arguments: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<PathBuf> {
    arguments
        .into_iter()
        .skip(1)
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .filter(|path| path.is_file())
        .collect()
}

pub fn run() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, arguments, cwd| {
            add_existing_paths(app, paths_from_arguments(arguments, Path::new(&cwd)));
        }))
        .manage(RuntimeState::default())
        .setup(|app| {
            let cwd = std::env::current_dir().unwrap_or_default();
            let arguments = std::env::args().collect::<Vec<_>>();
            add_existing_paths(app.handle(), paths_from_arguments(arguments, &cwd));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            queue_snapshot,
            add_files,
            remove_document,
            clear_queue,
            reorder_document,
            move_document,
            list_printers,
            print_all,
            cancel_printing
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Mass Print");

    application.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Opened { urls } = event {
            let paths = urls.into_iter().filter_map(|url| url.to_file_path().ok());
            add_existing_paths(app, paths);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
