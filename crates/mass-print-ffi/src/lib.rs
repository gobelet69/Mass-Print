use mass_print_core::{
    BatchPrintReport, DocumentQueue, DocumentStatus, JobOutcome, LpPrintBackend, PrintBackend,
    PrintSettings, SortCriterion, SortDirection,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct SortSelection {
    criterion: SortCriterion,
    direction: SortDirection,
}

struct MutableState {
    queue: DocumentQueue,
    active_sort: Option<SortSelection>,
    submitted_job_ids: Vec<String>,
}

pub struct Engine {
    state: Arc<Mutex<MutableState>>,
    canceled: Arc<AtomicBool>,
    backend: LpPrintBackend,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(MutableState {
                queue: DocumentQueue::default(),
                active_sort: Some(SortSelection {
                    criterion: SortCriterion::Original,
                    direction: SortDirection::Ascending,
                }),
                submitted_job_ids: Vec::new(),
            })),
            canceled: Arc::default(),
            backend: LpPrintBackend::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "camelCase")]
enum Request {
    Snapshot,
    AddPaths {
        paths: Vec<String>,
    },
    Remove {
        id: Uuid,
    },
    Clear,
    Reorder {
        id: Uuid,
        destination: usize,
    },
    Sort {
        criterion: SortCriterion,
        direction: SortDirection,
    },
    Printers,
    Print {
        settings: PrintSettings,
    },
    Cancel,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    documents: Vec<mass_print_core::DocumentEntry>,
    total_known_pages: u32,
    unknown_page_count: usize,
    active_sort: Option<SortSelection>,
}

impl Engine {
    fn call(&self, request: Request) -> Result<Value, String> {
        match request {
            Request::Snapshot => serde_json::to_value(self.snapshot()?).map_err(stringify),
            Request::AddPaths { paths } => {
                let mut state = self.state.lock().map_err(stringify)?;
                let report = state.queue.add_paths(paths);
                if let Some(selection) = state.active_sort {
                    state.queue.sort(selection.criterion, selection.direction);
                }
                serde_json::to_value(report).map_err(stringify)
            }
            Request::Remove { id } => {
                self.state
                    .lock()
                    .map_err(stringify)?
                    .queue
                    .remove(id)
                    .map_err(stringify)?;
                Ok(Value::Bool(true))
            }
            Request::Clear => {
                self.state.lock().map_err(stringify)?.queue.clear();
                Ok(Value::Bool(true))
            }
            Request::Reorder { id, destination } => {
                let mut state = self.state.lock().map_err(stringify)?;
                state.queue.reorder(id, destination).map_err(stringify)?;
                state.active_sort = None;
                Ok(Value::Bool(true))
            }
            Request::Sort {
                criterion,
                direction,
            } => {
                let mut state = self.state.lock().map_err(stringify)?;
                state.queue.sort(criterion, direction);
                state.active_sort = Some(SortSelection {
                    criterion,
                    direction,
                });
                Ok(Value::Bool(true))
            }
            Request::Printers => {
                serde_json::to_value(self.backend.printers().map_err(stringify)?).map_err(stringify)
            }
            Request::Print { settings } => {
                serde_json::to_value(self.print(settings)?).map_err(stringify)
            }
            Request::Cancel => {
                self.canceled.store(true, Ordering::SeqCst);
                let job_ids = self
                    .state
                    .lock()
                    .map_err(stringify)?
                    .submitted_job_ids
                    .clone();
                self.backend.cancel_jobs(&job_ids);
                Ok(Value::Bool(true))
            }
        }
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        let state = self.state.lock().map_err(stringify)?;
        Ok(Snapshot {
            documents: state.queue.documents().to_vec(),
            total_known_pages: state.queue.total_known_pages(),
            unknown_page_count: state.queue.unknown_page_count(),
            active_sort: state.active_sort,
        })
    }

    fn print(&self, settings: PrintSettings) -> Result<BatchPrintReport, String> {
        let documents = {
            let mut state = self.state.lock().map_err(stringify)?;
            state.submitted_job_ids.clear();
            state.queue.documents().to_vec()
        };
        self.canceled.store(false, Ordering::SeqCst);
        let submitted_state = Arc::clone(&self.state);
        let report = self
            .backend
            .print_batch_observing(&documents, &settings, &self.canceled, |job_id| {
                if let Ok(mut state) = submitted_state.lock() {
                    state.submitted_job_ids.push(job_id.to_string());
                }
            })
            .map_err(stringify)?;

        let mut state = self.state.lock().map_err(stringify)?;
        for job in &report.jobs {
            let status = match job.outcome {
                JobOutcome::Submitted => DocumentStatus::Submitted,
                JobOutcome::Failed => DocumentStatus::Failed,
                JobOutcome::Canceled => DocumentStatus::Canceled,
            };
            let _ = state
                .queue
                .set_status(job.document_id, status, Some(job.message.clone()));
        }
        Ok(report)
    }
}

fn stringify(error: impl ToString) -> String {
    error.to_string()
}

fn response(result: Result<Value, String>) -> *mut c_char {
    let value = match result {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    let serialized = serde_json::to_string(&value)
        .unwrap_or_else(|_| r#"{"ok":false,"error":"serialization failed"}"#.into());
    CString::new(serialized)
        .expect("JSON cannot contain interior nulls")
        .into_raw()
}

#[no_mangle]
pub extern "C" fn mass_print_engine_new() -> *mut Engine {
    Box::into_raw(Box::new(Engine::default()))
}

#[no_mangle]
/// Executes one JSON command against an engine instance.
///
/// # Safety
///
/// `engine` must come from [`mass_print_engine_new`] and remain alive for the
/// duration of the call. `request` must point to a valid null-terminated UTF-8
/// string. The returned string must be released with
/// [`mass_print_string_free`].
pub unsafe extern "C" fn mass_print_engine_call(
    engine: *mut Engine,
    request: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() || request.is_null() {
            return Err("invalid native bridge argument".into());
        }
        let request = CStr::from_ptr(request)
            .to_str()
            .map_err(stringify)
            .and_then(|json| serde_json::from_str::<Request>(json).map_err(stringify))?;
        (*engine).call(request)
    }))
    .unwrap_or_else(|_| Err("the Rust engine encountered an unexpected error".into()));
    response(result)
}

#[no_mangle]
/// Releases a response returned by [`mass_print_engine_call`].
///
/// # Safety
///
/// `value` must be null or a pointer returned by `mass_print_engine_call` that
/// has not previously been released.
pub unsafe extern "C" fn mass_print_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

#[no_mangle]
/// Releases an engine created by [`mass_print_engine_new`].
///
/// # Safety
///
/// `engine` must be null or a pointer returned by `mass_print_engine_new` that
/// is not in use and has not previously been released.
pub unsafe extern "C" fn mass_print_engine_free(engine: *mut Engine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Instant;

    #[test]
    fn bridge_add_sort_reorder_and_snapshot_share_one_queue() {
        let directory = tempfile::tempdir().unwrap();
        let chapter_ten = directory.path().join("Chapter 10.txt");
        let chapter_two = directory.path().join("Chapter 2.txt");
        fs::write(&chapter_ten, "ten").unwrap();
        fs::write(&chapter_two, "two").unwrap();
        let engine = Engine::default();

        engine
            .call(Request::AddPaths {
                paths: vec![
                    chapter_ten.to_string_lossy().into_owned(),
                    chapter_two.to_string_lossy().into_owned(),
                ],
            })
            .unwrap();
        engine
            .call(Request::Sort {
                criterion: SortCriterion::Name,
                direction: SortDirection::Ascending,
            })
            .unwrap();

        let snapshot = engine.snapshot().unwrap();
        assert_eq!(snapshot.documents[0].name, "Chapter 2.txt");
        let last_id = snapshot.documents[1].id;
        engine
            .call(Request::Reorder {
                id: last_id,
                destination: 0,
            })
            .unwrap();
        let snapshot = engine.snapshot().unwrap();
        assert_eq!(snapshot.documents[0].name, "Chapter 10.txt");
        assert!(snapshot.active_sort.is_none());
    }

    #[test]
    fn c_bridge_returns_structured_errors_without_panicking() {
        let engine = mass_print_engine_new();
        let request =
            CString::new(r#"{"command":"remove","id":"00000000-0000-0000-0000-000000000000"}"#)
                .unwrap();
        let response = unsafe { mass_print_engine_call(engine, request.as_ptr()) };
        let value: Value = unsafe { CStr::from_ptr(response) }
            .to_str()
            .map(serde_json::from_str)
            .unwrap()
            .unwrap();
        assert_eq!(value["ok"], false);
        assert!(value["error"].as_str().unwrap().contains("not found"));
        unsafe {
            mass_print_string_free(response);
            mass_print_engine_free(engine);
        }
    }

    #[test]
    #[ignore = "manual performance probe"]
    fn performance_probe_large_queue_bridge_operations() {
        let document_count = std::env::var("MASS_PRINT_PERF_DOCUMENTS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3_000usize);
        let directory = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..document_count)
            .map(|index| {
                let path = directory.path().join(format!(
                    "Résumé document {:05} chapter {}.txt",
                    document_count - index,
                    index % 97
                ));
                fs::write(&path, format!("Synthetic document {index}\n")).unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect();

        for iteration in 1..=5 {
            let engine = mass_print_engine_new();
            let call = |request: Value| {
                let request = CString::new(request.to_string()).unwrap();
                let response = unsafe { mass_print_engine_call(engine, request.as_ptr()) };
                let decoded: Value = unsafe { CStr::from_ptr(response) }
                    .to_str()
                    .map(serde_json::from_str)
                    .unwrap()
                    .unwrap();
                unsafe { mass_print_string_free(response) };
                assert_eq!(decoded["ok"], true, "{decoded}");
                decoded["data"].clone()
            };

            let started = Instant::now();
            call(json!({ "command": "addPaths", "paths": paths }));
            let add_ms = started.elapsed().as_secs_f64() * 1_000.0;

            let started = Instant::now();
            let snapshot = call(json!({ "command": "snapshot" }));
            let snapshot_ms = started.elapsed().as_secs_f64() * 1_000.0;
            let first_id = snapshot["documents"][0]["id"].as_str().unwrap();

            let started = Instant::now();
            call(json!({
                "command": "sort",
                "criterion": "name",
                "direction": "ascending"
            }));
            let sort_ascending_ms = started.elapsed().as_secs_f64() * 1_000.0;

            let started = Instant::now();
            call(json!({
                "command": "sort",
                "criterion": "name",
                "direction": "descending"
            }));
            let sort_descending_ms = started.elapsed().as_secs_f64() * 1_000.0;

            let started = Instant::now();
            call(json!({
                "command": "reorder",
                "id": first_id,
                "destination": document_count - 1
            }));
            let reorder_ms = started.elapsed().as_secs_f64() * 1_000.0;

            println!(
                "PERF iteration={iteration} documents={document_count} add_ms={add_ms:.3} snapshot_ms={snapshot_ms:.3} sort_ascending_ms={sort_ascending_ms:.3} sort_descending_ms={sort_descending_ms:.3} reorder_ms={reorder_ms:.3}"
            );
            unsafe { mass_print_engine_free(engine) };
        }
    }
}
