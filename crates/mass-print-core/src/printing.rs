use crate::{DocumentConverter, DocumentEntry, DocumentFormat, LibreOfficeConverter};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;
use uuid::Uuid;

const PREPARATION_WINDOW: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintSettings {
    pub printer: String,
    pub copies: u16,
    pub grayscale: bool,
    pub duplex: bool,
    pub paper_size: Option<String>,
    pub landscape: bool,
    pub fit_to_page: bool,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            printer: String::new(),
            copies: 1,
            grayscale: false,
            duplex: false,
            paper_size: None,
            landscape: false,
            fit_to_page: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobOutcome {
    Submitted,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReport {
    pub document_id: Uuid,
    pub outcome: JobOutcome,
    pub spooler_job_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPrintReport {
    pub jobs: Vec<JobReport>,
}

#[derive(Debug, Error)]
pub enum PrintError {
    #[error("no printer is selected")]
    MissingPrinter,
    #[error("the number of copies must be between 1 and 999")]
    InvalidCopies,
    #[error("the macOS printing command is unavailable: {0}")]
    SpoolerUnavailable(String),
    #[error("cannot prepare {file}: {reason}")]
    Preparation { file: String, reason: String },
}

pub trait PrintBackend {
    fn printers(&self) -> Result<Vec<PrinterInfo>, PrintError>;
    fn print_batch(
        &self,
        documents: &[DocumentEntry],
        settings: &PrintSettings,
        canceled: &AtomicBool,
    ) -> Result<BatchPrintReport, PrintError>;
    fn cancel_jobs(&self, job_ids: &[String]);
}

#[derive(Clone, Debug, Default)]
pub struct LpPrintBackend {
    converter: LibreOfficeConverter,
}

impl LpPrintBackend {
    fn validate(settings: &PrintSettings) -> Result<(), PrintError> {
        if settings.printer.trim().is_empty() {
            return Err(PrintError::MissingPrinter);
        }
        if !(1..=999).contains(&settings.copies) {
            return Err(PrintError::InvalidCopies);
        }
        Ok(())
    }

    fn prepare(
        &self,
        document: &DocumentEntry,
        temporary_directory: &Path,
    ) -> Result<PathBuf, PrintError> {
        if document.format.needs_office_conversion() {
            return self
                .converter
                .convert_to_pdf(document, temporary_directory)
                .map_err(|error| PrintError::Preparation {
                    file: document.name.clone(),
                    reason: error.to_string(),
                });
        }
        if document.format == DocumentFormat::Pdf {
            return Ok(document.path.clone());
        }

        let output_path = temporary_directory.join(format!("{}.pdf", document.id));
        let output_file = File::create(&output_path).map_err(|error| PrintError::Preparation {
            file: document.name.clone(),
            reason: error.to_string(),
        })?;
        let output = Command::new("/usr/sbin/cupsfilter")
            .arg("-m")
            .arg("application/pdf")
            .arg(&document.path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(output_file))
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| PrintError::Preparation {
                file: document.name.clone(),
                reason: format!("cupsfilter is unavailable: {error}"),
            })?;
        if !output.status.success() {
            return Err(PrintError::Preparation {
                file: document.name.clone(),
                reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(output_path)
    }

    fn submit(path: &Path, settings: &PrintSettings) -> Result<String, String> {
        let mut command = Command::new("/usr/bin/lp");
        command
            .arg("-d")
            .arg(&settings.printer)
            .arg("-n")
            .arg(settings.copies.to_string());
        if settings.grayscale {
            command.args(["-o", "ColorModel=Gray"]);
        }
        if settings.duplex {
            command.args(["-o", "sides=two-sided-long-edge"]);
        }
        if let Some(paper_size) = settings.paper_size.as_deref() {
            command.args(["-o", &format!("media={paper_size}")]);
        }
        if settings.landscape {
            command.args(["-o", "landscape"]);
        }
        if settings.fit_to_page {
            command.args(["-o", "fit-to-page"]);
        }
        let output = command
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if message.is_empty() {
                format!("lp exited with {}", output.status)
            } else {
                message
            });
        }
        let response = String::from_utf8_lossy(&output.stdout);
        Ok(parse_job_id(&response).unwrap_or_else(|| response.trim().to_string()))
    }

    pub fn print_batch_observing<F>(
        &self,
        documents: &[DocumentEntry],
        settings: &PrintSettings,
        canceled: &AtomicBool,
        on_submitted: F,
    ) -> Result<BatchPrintReport, PrintError>
    where
        F: Fn(&str),
    {
        self.print_batch_observing_with_submit(
            documents,
            settings,
            canceled,
            on_submitted,
            Self::submit,
        )
    }

    fn print_batch_observing_with_submit<F, S>(
        &self,
        documents: &[DocumentEntry],
        settings: &PrintSettings,
        canceled: &AtomicBool,
        on_submitted: F,
        submit: S,
    ) -> Result<BatchPrintReport, PrintError>
    where
        F: Fn(&str),
        S: Fn(&Path, &PrintSettings) -> Result<String, String>,
    {
        self.print_batch_observing_with_prepare_and_submit(
            documents,
            settings,
            canceled,
            on_submitted,
            |document, temporary_directory| self.prepare(document, temporary_directory),
            submit,
        )
    }

    fn print_batch_observing_with_prepare_and_submit<F, P, S>(
        &self,
        documents: &[DocumentEntry],
        settings: &PrintSettings,
        canceled: &AtomicBool,
        on_submitted: F,
        prepare: P,
        submit: S,
    ) -> Result<BatchPrintReport, PrintError>
    where
        F: Fn(&str),
        P: Fn(&DocumentEntry, &Path) -> Result<PathBuf, PrintError> + Sync,
        S: Fn(&Path, &PrintSettings) -> Result<String, String>,
    {
        Self::validate(settings)?;
        let temporary = LibreOfficeConverter::temporary_directory().map_err(|error| {
            PrintError::Preparation {
                file: "temporary workspace".into(),
                reason: error.to_string(),
            }
        })?;
        let mut jobs = Vec::with_capacity(documents.len());
        let mut cursor = 0;

        while cursor < documents.len() {
            if canceled.load(Ordering::SeqCst) {
                jobs.extend(documents[cursor..].iter().map(canceled_report));
                break;
            }

            let window_end = preparation_window_end(documents, cursor);
            let window = &documents[cursor..window_end];
            let prepared = if window.len() == 1 {
                vec![prepare(&window[0], temporary.path())]
            } else {
                std::thread::scope(|scope| {
                    let handles: Vec<_> = window
                        .iter()
                        .map(|document| scope.spawn(|| prepare(document, temporary.path())))
                        .collect();
                    handles
                        .into_iter()
                        .zip(window)
                        .map(|(handle, document)| {
                            handle.join().unwrap_or_else(|_| {
                                Err(PrintError::Preparation {
                                    file: document.name.clone(),
                                    reason: "the preparation worker stopped unexpectedly".into(),
                                })
                            })
                        })
                        .collect()
                })
            };

            for (document, prepared) in window.iter().zip(prepared) {
                if canceled.load(Ordering::SeqCst) {
                    jobs.push(canceled_report(document));
                    continue;
                }
                let prepared = match prepared {
                    Ok(path) => path,
                    Err(error) => {
                        jobs.push(JobReport {
                            document_id: document.id,
                            outcome: JobOutcome::Failed,
                            spooler_job_id: None,
                            message: error.to_string(),
                        });
                        continue;
                    }
                };
                match submit(&prepared, settings) {
                    Ok(job_id) => {
                        on_submitted(&job_id);
                        jobs.push(JobReport {
                            document_id: document.id,
                            outcome: JobOutcome::Submitted,
                            spooler_job_id: Some(job_id),
                            message: "Accepted by the print spooler".into(),
                        });
                    }
                    Err(message) => jobs.push(JobReport {
                        document_id: document.id,
                        outcome: JobOutcome::Failed,
                        spooler_job_id: None,
                        message,
                    }),
                }
            }
            cursor = window_end;
        }
        Ok(BatchPrintReport { jobs })
    }
}

fn preparation_window_end(documents: &[DocumentEntry], cursor: usize) -> usize {
    if documents[cursor].format.needs_office_conversion() {
        return cursor + 1;
    }
    documents[cursor..]
        .iter()
        .take(PREPARATION_WINDOW)
        .position(|document| document.format.needs_office_conversion())
        .map_or_else(
            || (cursor + PREPARATION_WINDOW).min(documents.len()),
            |offset| cursor + offset,
        )
}

fn canceled_report(document: &DocumentEntry) -> JobReport {
    JobReport {
        document_id: document.id,
        outcome: JobOutcome::Canceled,
        spooler_job_id: None,
        message: "Not submitted because printing was canceled".into(),
    }
}

impl PrintBackend for LpPrintBackend {
    fn printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        let default_output = Command::new("/usr/bin/lpstat")
            .arg("-d")
            .output()
            .map_err(|error| PrintError::SpoolerUnavailable(error.to_string()))?;
        let default_name = parse_default_printer(&String::from_utf8_lossy(&default_output.stdout));
        let output = Command::new("/usr/bin/lpstat")
            .arg("-p")
            .output()
            .map_err(|error| PrintError::SpoolerUnavailable(error.to_string()))?;
        if !output.status.success() {
            return Err(PrintError::SpoolerUnavailable(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(parse_printers(
            &String::from_utf8_lossy(&output.stdout),
            default_name.as_deref(),
        ))
    }

    fn print_batch(
        &self,
        documents: &[DocumentEntry],
        settings: &PrintSettings,
        canceled: &AtomicBool,
    ) -> Result<BatchPrintReport, PrintError> {
        self.print_batch_observing(documents, settings, canceled, |_| {})
    }

    fn cancel_jobs(&self, job_ids: &[String]) {
        for job_id in job_ids {
            let _ = Command::new("/usr/bin/cancel")
                .arg(job_id)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn parse_default_printer(output: &str) -> Option<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("system default destination: "))
        .map(str::trim)
        .map(str::to_string)
}

fn parse_printers(output: &str, default: Option<&str>) -> Vec<PrinterInfo> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix("printer "))
        .filter_map(|line| line.split_whitespace().next())
        .map(|name| PrinterInfo {
            name: name.to_string(),
            is_default: default == Some(name),
        })
        .collect()
}

fn parse_job_id(output: &str) -> Option<String> {
    let request = output.trim().strip_prefix("request id is ")?;
    request.split_whitespace().next().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Barrier, Mutex};
    use std::time::Instant;

    #[test]
    fn parses_printers_and_marks_default() {
        let output = "printer Office is idle. enabled since today\nprinter Label is idle.";
        let printers = parse_printers(output, Some("Label"));
        assert_eq!(printers.len(), 2);
        assert!(!printers[0].is_default);
        assert!(printers[1].is_default);
    }

    #[test]
    fn parses_default_printer() {
        assert_eq!(
            parse_default_printer("system default destination: Office\n"),
            Some("Office".into())
        );
        assert_eq!(parse_default_printer("no system default destination"), None);
    }

    #[test]
    fn extracts_spooler_job_id() {
        assert_eq!(
            parse_job_id("request id is Office-42 (1 file(s))\n"),
            Some("Office-42".into())
        );
    }

    #[test]
    fn validates_print_settings() {
        let mut settings = PrintSettings::default();
        assert!(matches!(
            LpPrintBackend::validate(&settings),
            Err(PrintError::MissingPrinter)
        ));
        settings.printer = "Office".into();
        settings.copies = 0;
        assert!(matches!(
            LpPrintBackend::validate(&settings),
            Err(PrintError::InvalidCopies)
        ));
        settings.copies = 2;
        assert!(LpPrintBackend::validate(&settings).is_ok());
    }

    #[test]
    fn cancellation_prevents_any_submission() {
        use std::fs;
        use tempfile::tempdir;

        let directory = tempdir().unwrap();
        let path = directory.path().join("test.pdf");
        fs::write(&path, b"not needed because canceled").unwrap();
        let document = DocumentEntry::inspect(path).unwrap();
        let canceled = AtomicBool::new(true);
        let settings = PrintSettings {
            printer: "Test".into(),
            ..Default::default()
        };
        let report = LpPrintBackend::default()
            .print_batch(&[document], &settings, &canceled)
            .unwrap();
        assert_eq!(report.jobs[0].outcome, JobOutcome::Canceled);
    }

    #[test]
    fn batch_workflow_preserves_order_and_cancels_remaining_jobs_without_spooler() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let documents: Vec<_> = (0..3)
            .map(|index| {
                let path = directory.path().join(format!("document-{index}.pdf"));
                fs::write(&path, b"%PDF-1.4\n%%EOF").unwrap();
                DocumentEntry::inspect(path).unwrap()
            })
            .collect();
        let canceled = AtomicBool::new(false);
        let submitted = AtomicUsize::new(0);
        let settings = PrintSettings {
            printer: "PerformanceAuditSink".into(),
            ..Default::default()
        };

        let report = LpPrintBackend::default()
            .print_batch_observing_with_submit(
                &documents,
                &settings,
                &canceled,
                |_| {},
                |_, _| {
                    let index = submitted.fetch_add(1, Ordering::SeqCst);
                    canceled.store(true, Ordering::SeqCst);
                    Ok(format!("audit-{index}"))
                },
            )
            .unwrap();

        assert_eq!(submitted.load(Ordering::SeqCst), 1);
        assert_eq!(report.jobs[0].document_id, documents[0].id);
        assert_eq!(report.jobs[0].outcome, JobOutcome::Submitted);
        assert_eq!(report.jobs[0].spooler_job_id.as_deref(), Some("audit-0"));
        assert_eq!(report.jobs[1].document_id, documents[1].id);
        assert_eq!(report.jobs[1].outcome, JobOutcome::Canceled);
        assert_eq!(report.jobs[2].document_id, documents[2].id);
        assert_eq!(report.jobs[2].outcome, JobOutcome::Canceled);
    }

    #[test]
    fn bounded_parallel_preparation_preserves_submission_order() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let documents: Vec<_> = (0..4)
            .map(|index| {
                let path = directory.path().join(format!("document-{index}.txt"));
                fs::write(&path, format!("Synthetic document {index}\n")).unwrap();
                DocumentEntry::inspect(path).unwrap()
            })
            .collect();
        let canceled = AtomicBool::new(false);
        let prepared = AtomicUsize::new(0);
        let barrier = Barrier::new(4);
        let submission_order = Mutex::new(Vec::new());
        let settings = PrintSettings {
            printer: "PerformanceAuditSink".into(),
            ..Default::default()
        };

        let report = LpPrintBackend::default()
            .print_batch_observing_with_prepare_and_submit(
                &documents,
                &settings,
                &canceled,
                |_| {},
                |document, _| {
                    prepared.fetch_add(1, Ordering::SeqCst);
                    barrier.wait();
                    let index = document
                        .path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .and_then(|stem| stem.rsplit('-').next())
                        .and_then(|value| value.parse::<u64>().ok())
                        .unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(4 - index));
                    Ok(document.path.clone())
                },
                |path, _| {
                    submission_order
                        .lock()
                        .unwrap()
                        .push(path.file_name().unwrap().to_string_lossy().into_owned());
                    Ok("audit-job".into())
                },
            )
            .unwrap();

        assert_eq!(prepared.load(Ordering::SeqCst), 4);
        assert_eq!(
            submission_order.into_inner().unwrap(),
            [
                "document-0.txt",
                "document-1.txt",
                "document-2.txt",
                "document-3.txt"
            ]
        );
        assert!(report
            .jobs
            .iter()
            .all(|job| job.outcome == JobOutcome::Submitted));
    }

    #[test]
    fn cancellation_does_not_prepare_beyond_the_active_window() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let documents: Vec<_> = (0..8)
            .map(|index| {
                let path = directory.path().join(format!("document-{index}.txt"));
                fs::write(&path, format!("Synthetic document {index}\n")).unwrap();
                DocumentEntry::inspect(path).unwrap()
            })
            .collect();
        let canceled = AtomicBool::new(false);
        let prepared = AtomicUsize::new(0);
        let submitted = AtomicUsize::new(0);
        let settings = PrintSettings {
            printer: "PerformanceAuditSink".into(),
            ..Default::default()
        };

        let report = LpPrintBackend::default()
            .print_batch_observing_with_prepare_and_submit(
                &documents,
                &settings,
                &canceled,
                |_| {},
                |document, _| {
                    prepared.fetch_add(1, Ordering::SeqCst);
                    Ok(document.path.clone())
                },
                |_, _| {
                    submitted.fetch_add(1, Ordering::SeqCst);
                    canceled.store(true, Ordering::SeqCst);
                    Ok("audit-job".into())
                },
            )
            .unwrap();

        assert_eq!(prepared.load(Ordering::SeqCst), 4);
        assert_eq!(submitted.load(Ordering::SeqCst), 1);
        assert_eq!(report.jobs[0].outcome, JobOutcome::Submitted);
        assert!(report.jobs[1..]
            .iter()
            .all(|job| job.outcome == JobOutcome::Canceled));
    }

    #[test]
    fn office_conversion_isolated_from_parallel_preparation_windows() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let documents: Vec<_> = ["first.txt", "office.docx", "last.txt"]
            .into_iter()
            .map(|name| {
                let path = directory.path().join(name);
                fs::write(&path, "synthetic").unwrap();
                DocumentEntry::inspect(path).unwrap()
            })
            .collect();

        assert_eq!(preparation_window_end(&documents, 0), 1);
        assert_eq!(preparation_window_end(&documents, 1), 2);
        assert_eq!(preparation_window_end(&documents, 2), 3);
    }

    #[test]
    #[ignore = "manual performance probe"]
    fn performance_probe_prepares_text_documents_without_submitting() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let documents: Vec<_> = (0..25)
            .map(|index| {
                let path = directory.path().join(format!("document-{index:03}.txt"));
                fs::write(&path, format!("Synthetic print document {index}\n")).unwrap();
                DocumentEntry::inspect(path).unwrap()
            })
            .collect();
        let backend = LpPrintBackend::default();
        let canceled = AtomicBool::new(false);
        let submitted = AtomicUsize::new(0);
        let settings = PrintSettings {
            printer: "PerformanceAuditSink".into(),
            ..Default::default()
        };
        let started = Instant::now();
        let report = backend
            .print_batch_observing_with_submit(
                &documents,
                &settings,
                &canceled,
                |_| {},
                |prepared, _| {
                    assert!(prepared.is_file());
                    let job = submitted.fetch_add(1, Ordering::SeqCst);
                    Ok(format!("audit-{job}"))
                },
            )
            .unwrap();
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(submitted.load(Ordering::SeqCst), documents.len());
        assert!(report
            .jobs
            .iter()
            .all(|job| job.outcome == JobOutcome::Submitted));
        println!(
            "PERF documents={} preparation_ms={elapsed_ms:.3} submitted_jobs=0",
            documents.len()
        );
    }
}
