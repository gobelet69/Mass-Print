use crate::{DocumentEntry, DocumentFormat};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("LibreOffice is required to convert {0}")]
    LibreOfficeUnavailable(&'static str),
    #[error("conversion failed: {0}")]
    Failed(String),
    #[error("the converter did not produce a PDF")]
    MissingOutput,
}

pub trait DocumentConverter: Send + Sync {
    fn is_available(&self) -> bool;
    fn convert_to_pdf(
        &self,
        document: &DocumentEntry,
        output_directory: &Path,
    ) -> Result<PathBuf, ConversionError>;
}

#[derive(Clone, Debug)]
pub struct LibreOfficeConverter {
    executable: Option<PathBuf>,
}

impl Default for LibreOfficeConverter {
    fn default() -> Self {
        Self::detect()
    }
}

impl LibreOfficeConverter {
    pub fn detect() -> Self {
        let candidates = [
            PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice"),
            PathBuf::from("/usr/local/bin/soffice"),
            PathBuf::from("/opt/homebrew/bin/soffice"),
        ];
        Self {
            executable: candidates.into_iter().find(|path| path.is_file()),
        }
    }

    pub fn temporary_directory() -> Result<TempDir, ConversionError> {
        tempfile::Builder::new()
            .prefix("mass-print-")
            .tempdir()
            .map_err(|error| ConversionError::Failed(error.to_string()))
    }
}

impl DocumentConverter for LibreOfficeConverter {
    fn is_available(&self) -> bool {
        self.executable.is_some()
    }

    fn convert_to_pdf(
        &self,
        document: &DocumentEntry,
        output_directory: &Path,
    ) -> Result<PathBuf, ConversionError> {
        if !document.format.needs_office_conversion() {
            return Ok(document.path.clone());
        }
        let executable =
            self.executable
                .as_ref()
                .ok_or(ConversionError::LibreOfficeUnavailable(
                    document.format.label(),
                ))?;

        let output = Command::new(executable)
            .args(["--headless", "--convert-to", "pdf", "--outdir"])
            .arg(output_directory)
            .arg(&document.path)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| ConversionError::Failed(error.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(ConversionError::Failed(if stderr.is_empty() {
                format!("LibreOffice exited with {}", output.status)
            } else {
                stderr
            }));
        }

        let stem = document
            .path
            .file_stem()
            .ok_or(ConversionError::MissingOutput)?;
        let converted = output_directory.join(stem).with_extension("pdf");
        converted
            .is_file()
            .then_some(converted)
            .ok_or(ConversionError::MissingOutput)
    }
}

#[allow(dead_code)]
fn _format_is_explicit(format: DocumentFormat) -> bool {
    format.needs_office_conversion()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DocumentEntry;
    use std::fs;

    #[test]
    fn reports_missing_libreoffice_for_rich_documents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.docx");
        fs::write(&path, "placeholder").unwrap();
        let document = DocumentEntry::inspect(path).unwrap();
        let converter = LibreOfficeConverter { executable: None };
        let error = converter
            .convert_to_pdf(&document, directory.path())
            .unwrap_err();
        assert!(matches!(error, ConversionError::LibreOfficeUnavailable(_)));
    }

    #[test]
    fn native_pdf_does_not_require_libreoffice() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.pdf");
        fs::write(&path, "%PDF-1.4\n%%EOF").unwrap();
        let document = DocumentEntry::inspect(path.clone()).unwrap();
        let converter = LibreOfficeConverter { executable: None };
        assert_eq!(
            converter
                .convert_to_pdf(&document, directory.path())
                .unwrap(),
            path
        );
    }
}
