use mass_print_core::{
    AddReport, BatchPrintReport, DocumentQueue, DocumentStatus, JobOutcome, LpPrintBackend,
    PrintBackend, PrintSettings, PrinterInfo, SortCriterion, SortDirection,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use uuid::Uuid;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct SortSelection {
    criterion: SortCriterion,
    direction: SortDirection,
}

#[derive(Clone)]
struct RuntimeState {
    queue: Arc<Mutex<DocumentQueue>>,
    active_sort: Arc<Mutex<Option<SortSelection>>>,
    canceled: Arc<AtomicBool>,
    submitted_job_ids: Arc<Mutex<Vec<String>>>,
    backend: LpPrintBackend,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            queue: Arc::default(),
            active_sort: Arc::new(Mutex::new(Some(SortSelection {
                criterion: SortCriterion::Original,
                direction: SortDirection::Ascending,
            }))),
            canceled: Arc::default(),
            submitted_job_ids: Arc::default(),
            backend: LpPrintBackend::default(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueueSnapshot {
    documents: Vec<mass_print_core::DocumentEntry>,
    total_known_pages: u32,
    unknown_page_count: usize,
    active_sort: Option<SortSelection>,
}

#[tauri::command]
fn queue_snapshot(state: State<'_, RuntimeState>) -> Result<QueueSnapshot, String> {
    let active_sort = *state
        .active_sort
        .lock()
        .map_err(|error| error.to_string())?;
    let queue = state.queue.lock().map_err(|error| error.to_string())?;
    Ok(QueueSnapshot {
        documents: queue.documents().to_vec(),
        total_known_pages: queue.total_known_pages(),
        unknown_page_count: queue.unknown_page_count(),
        active_sort,
    })
}

#[tauri::command]
fn add_files(paths: Vec<String>, state: State<'_, RuntimeState>) -> Result<AddReport, String> {
    let active_sort = *state
        .active_sort
        .lock()
        .map_err(|error| error.to_string())?;
    let mut queue = state.queue.lock().map_err(|error| error.to_string())?;
    let report = queue.add_paths(paths);
    if let Some(selection) = active_sort {
        queue.sort(selection.criterion, selection.direction);
    }
    Ok(report)
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
        .map_err(|error| error.to_string())?;
    *state
        .active_sort
        .lock()
        .map_err(|error| error.to_string())? = None;
    Ok(())
}

#[tauri::command]
fn move_document(id: Uuid, delta: isize, state: State<'_, RuntimeState>) -> Result<(), String> {
    state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .move_by(id, delta)
        .map_err(|error| error.to_string())?;
    *state
        .active_sort
        .lock()
        .map_err(|error| error.to_string())? = None;
    Ok(())
}

#[tauri::command]
fn sort_queue(
    criterion: SortCriterion,
    direction: SortDirection,
    state: State<'_, RuntimeState>,
) -> Result<(), String> {
    state
        .queue
        .lock()
        .map_err(|error| error.to_string())?
        .sort(criterion, direction);
    *state
        .active_sort
        .lock()
        .map_err(|error| error.to_string())? = Some(SortSelection {
        criterion,
        direction,
    });
    Ok(())
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

const FINDER_ACTION_NAME: &str = "Print with Mass Print.workflow";
const FINDER_ACTION_IDENTIFIER: &str = "com.massprint.finder-action";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FinderActionStatus {
    installed: bool,
    valid: bool,
    needs_update: bool,
    path: String,
    message: String,
}

fn finder_action_target(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .home_dir()
        .map_err(|error| error.to_string())?
        .join("Library/Services")
        .join(FINDER_ACTION_NAME))
}

#[tauri::command]
fn finder_action_status(app: AppHandle) -> Result<FinderActionStatus, String> {
    let source = bundled_finder_action(&app)?;
    let target = finder_action_target(&app)?;
    let path = target.display().to_string();
    if !target.exists() {
        return Ok(FinderActionStatus {
            installed: false,
            valid: false,
            needs_update: false,
            path,
            message: "The Finder Quick Action is not installed".into(),
        });
    }

    match validate_finder_action(&target) {
        Ok(()) => {
            let needs_update = !finder_action_matches(&source, &target)?;
            Ok(FinderActionStatus {
                installed: true,
                valid: true,
                needs_update,
                path,
                message: if needs_update {
                    "The Finder Quick Action should be updated".into()
                } else {
                    "The Finder Quick Action is installed with macOS Services".into()
                },
            })
        }
        Err(error) => Ok(FinderActionStatus {
            installed: true,
            valid: false,
            needs_update: true,
            path,
            message: error,
        }),
    }
}

#[tauri::command]
fn install_finder_action(app: AppHandle) -> Result<(), String> {
    let source = bundled_finder_action(&app)?;
    validate_finder_action(&source)?;
    let target = finder_action_target(&app)?;
    let services = target
        .parent()
        .ok_or_else(|| "Cannot locate the Services directory".to_string())?;
    fs::create_dir_all(services).map_err(|error| error.to_string())?;

    if target.exists() && !finder_action_is_owned(&target)? {
        return Err(format!(
            "A workflow named '{FINDER_ACTION_NAME}' already exists and is not owned by Mass Print"
        ));
    }

    let staging = services.join(format!(".{FINDER_ACTION_NAME}.installing"));
    let backup = services.join(format!(".{FINDER_ACTION_NAME}.backup"));
    remove_owned_temporary_path(&staging)?;
    remove_owned_temporary_path(&backup)?;
    copy_directory(&source, &staging)?;
    validate_finder_action(&staging)?;

    if target.exists() {
        fs::rename(&target, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(&staging, &target) {
        if backup.exists() {
            let _ = fs::rename(&backup, &target);
        }
        return Err(format!("The Finder action could not be installed: {error}"));
    }
    remove_owned_temporary_path(&backup)?;
    refresh_finder_services(&target);
    Ok(())
}

#[tauri::command]
fn uninstall_finder_action(app: AppHandle) -> Result<(), String> {
    let target = finder_action_target(&app)?;
    if !target.exists() {
        return Ok(());
    }
    if !finder_action_is_owned(&target)? {
        return Err("The installed workflow is not owned by Mass Print and was not removed".into());
    }
    fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    refresh_finder_services(&target);
    Ok(())
}

fn bundled_finder_action(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?
        .join("finder")
        .join(FINDER_ACTION_NAME))
}

fn validate_finder_action(workflow: &Path) -> Result<(), String> {
    let info = workflow.join("Contents/Info.plist");
    let document = workflow.join("Contents/document.wflow");
    for file in [&info, &document] {
        if !file.is_file() {
            return Err(format!(
                "Finder action is incomplete: {} is missing",
                file.display()
            ));
        }
        let output = Command::new("/usr/bin/plutil")
            .arg("-lint")
            .arg(file)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("Cannot validate {}: {error}", file.display()))?;
        if !output.status.success() {
            return Err(format!(
                "Finder action plist is invalid: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }

    let info_contents = fs::read_to_string(info).map_err(|error| error.to_string())?;
    let workflow_contents = fs::read_to_string(document).map_err(|error| error.to_string())?;
    if !info_contents.contains(FINDER_ACTION_IDENTIFIER)
        || !info_contents.contains("<string>BNDL</string>")
        || !workflow_contents.contains("com.apple.Automator.servicesMenu")
        || !workflow_contents.contains("com.apple.finder")
        || !workflow_contents.contains("/usr/bin/open -b com.massprint.app")
    {
        return Err("Finder action metadata or file handoff command is invalid".into());
    }
    Ok(())
}

fn finder_action_is_owned(workflow: &Path) -> Result<bool, String> {
    let info = workflow.join("Contents/Info.plist");
    if info.is_file() {
        return fs::read_to_string(info)
            .map(|contents| contents.contains(FINDER_ACTION_IDENTIFIER))
            .map_err(|error| error.to_string());
    }
    let document = workflow.join("Contents/document.wflow");
    if document.is_file() {
        return fs::read_to_string(document)
            .map(|contents| {
                contents.contains("com.massprint.app")
                    || contents.contains("open -a &quot;Mass Print&quot;")
                    || contents.contains("open -a \"Mass Print\"")
            })
            .map_err(|error| error.to_string());
    }
    let is_empty = fs::read_dir(workflow)
        .map_err(|error| error.to_string())?
        .next()
        .is_none();
    Ok(is_empty)
}

fn finder_action_matches(source: &Path, target: &Path) -> Result<bool, String> {
    for relative in ["Contents/Info.plist", "Contents/document.wflow"] {
        let source_bytes = fs::read(source.join(relative)).map_err(|error| error.to_string())?;
        let target_bytes = fs::read(target.join(relative)).map_err(|error| error.to_string())?;
        if source_bytes != target_bytes {
            return Ok(false);
        }
    }
    Ok(true)
}

fn remove_owned_temporary_path(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn refresh_finder_services(target: &Path) {
    if target.exists() {
        let _ = Command::new("/usr/bin/touch")
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        let _ = Command::new(
            "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
        )
        .arg("-f")
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    }
    for argument in ["-flush", "-update"] {
        let _ = Command::new("/System/Library/CoreServices/pbs")
            .arg(argument)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = Command::new("/usr/bin/killall")
        .arg("Finder")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| {
        format!(
            "The bundled Finder action could not be read at {}: {error}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|error| error.to_string())?;
        let destination = target.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            copy_directory(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn add_existing_paths(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let state = app.state::<RuntimeState>();
    let valid_paths: Vec<_> = paths.into_iter().filter(|path| path.is_file()).collect();
    if !valid_paths.is_empty() {
        let active_sort = state.active_sort.lock().ok().and_then(|sort| *sort);
        if let Ok(mut queue) = state.queue.lock() {
            queue.add_paths(valid_paths);
            if let Some(selection) = active_sort {
                queue.sort(selection.criterion, selection.direction);
            }
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
            sort_queue,
            list_printers,
            print_all,
            cancel_printing,
            finder_action_status,
            install_finder_action,
            uninstall_finder_action
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
