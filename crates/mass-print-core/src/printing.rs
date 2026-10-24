use crate::{DocumentConverter, DocumentEntry, DocumentFormat, LibreOfficeConverter};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;
use uuid::Uuid;

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
        Self::validate(settings)?;
        let temporary = LibreOfficeConverter::temporary_directory().map_err(|error| {
            PrintError::Preparation {
                file: "temporary workspace".into(),
                reason: error.to_string(),
            }
        })?;
        let mut jobs = Vec::with_capacity(documents.len());

        for document in documents {
            if canceled.load(Ordering::SeqCst) {
                jobs.push(JobReport {
                    document_id: document.id,
                    outcome: JobOutcome::Canceled,
                    spooler_job_id: None,
                    message: "Not submitted because printing was canceled".into(),
                });
                continue;
            }
            let prepared = match self.prepare(document, temporary.path()) {
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
            match Self::submit(&prepared, settings) {
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
        Ok(BatchPrintReport { jobs })
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
}
