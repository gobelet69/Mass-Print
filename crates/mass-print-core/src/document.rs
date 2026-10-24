use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentFormat {
    Pdf,
    Png,
    Jpeg,
    Tiff,
    Text,
    RichText,
    Word,
    OpenDocumentText,
    Html,
    Excel,
    PowerPoint,
    OpenDocumentSpreadsheet,
    OpenDocumentPresentation,
}

impl DocumentFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "pdf" => Some(Self::Pdf),
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "tif" | "tiff" => Some(Self::Tiff),
            "txt" => Some(Self::Text),
            "rtf" => Some(Self::RichText),
            "doc" | "docx" => Some(Self::Word),
            "odt" => Some(Self::OpenDocumentText),
            "html" | "htm" => Some(Self::Html),
            "xls" | "xlsx" => Some(Self::Excel),
            "ppt" | "pptx" => Some(Self::PowerPoint),
            "ods" => Some(Self::OpenDocumentSpreadsheet),
            "odp" => Some(Self::OpenDocumentPresentation),
            _ => None,
        }
    }

    pub fn needs_office_conversion(self) -> bool {
        !matches!(
            self,
            Self::Pdf | Self::Png | Self::Jpeg | Self::Tiff | Self::Text
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Png => "PNG image",
            Self::Jpeg => "JPEG image",
            Self::Tiff => "TIFF image",
            Self::Text => "Plain text",
            Self::RichText => "Rich Text",
            Self::Word => "Word document",
            Self::OpenDocumentText => "OpenDocument text",
            Self::Html => "HTML document",
            Self::Excel => "Spreadsheet",
            Self::PowerPoint => "Presentation",
            Self::OpenDocumentSpreadsheet => "OpenDocument spreadsheet",
            Self::OpenDocumentPresentation => "OpenDocument presentation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentStatus {
    Ready,
    NeedsConversion,
    Preparing,
    Submitted,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEntry {
    pub id: Uuid,
    pub path: PathBuf,
    pub name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at_ms: Option<u64>,
    pub insertion_sequence: u64,
    pub page_count: Option<u32>,
    pub format: DocumentFormat,
    pub status: DocumentStatus,
    pub status_message: Option<String>,
}

impl DocumentEntry {
    pub(crate) fn inspect(path: PathBuf) -> Result<Self, String> {
        let metadata = path
            .metadata()
            .map_err(|error| format!("Cannot access the file: {error}"))?;
        if !metadata.is_file() {
            return Err("The selected item is not a regular file".into());
        }

        let format = DocumentFormat::from_path(&path)
            .ok_or_else(|| "This file type is not supported".to_string())?;
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| "The filename is not valid Unicode".to_string())?
            .to_string();
        let extension = path
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_uppercase();
        let page_count = inspect_page_count(&path, format);

        Ok(Self {
            id: Uuid::new_v4(),
            path,
            name,
            extension,
            size_bytes: metadata.len(),
            modified_at_ms: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as u64),
            insertion_sequence: 0,
            page_count,
            format,
            status: if format.needs_office_conversion() {
                DocumentStatus::NeedsConversion
            } else {
                DocumentStatus::Ready
            },
            status_message: format
                .needs_office_conversion()
                .then(|| "LibreOffice is required to prepare this format for printing".to_string()),
        })
    }
}

fn inspect_page_count(path: &Path, format: DocumentFormat) -> Option<u32> {
    match format {
        DocumentFormat::Pdf => lopdf::Document::load(path)
            .ok()
            .map(|document| document.get_pages().len() as u32),
        DocumentFormat::Png | DocumentFormat::Jpeg => Some(1),
        DocumentFormat::Text => count_text_pages(path),
        _ => None,
    }
}

fn count_text_pages(path: &Path) -> Option<u32> {
    let reader = BufReader::new(File::open(path).ok()?);
    let lines = reader.lines().count() as u32;
    Some(lines.max(1).div_ceil(55))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_extensions_case_insensitively() {
        assert_eq!(
            DocumentFormat::from_path(Path::new("Résumé.JPEG")),
            Some(DocumentFormat::Jpeg)
        );
        assert_eq!(
            DocumentFormat::from_path(Path::new("report.docx")),
            Some(DocumentFormat::Word)
        );
        assert_eq!(DocumentFormat::from_path(Path::new("archive.zip")), None);
    }

    #[test]
    fn distinguishes_native_and_converted_formats() {
        assert!(!DocumentFormat::Pdf.needs_office_conversion());
        assert!(DocumentFormat::PowerPoint.needs_office_conversion());
    }
}
