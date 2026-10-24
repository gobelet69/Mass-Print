//! Testable application core for Mass Print.
//!
//! The crate deliberately has no GUI dependency. It owns file validation,
//! queue ordering, conversion orchestration, and sequential spooler submission.

mod conversion;
mod document;
mod printing;
mod queue;

pub use conversion::{ConversionError, DocumentConverter, LibreOfficeConverter};
pub use document::{DocumentEntry, DocumentFormat, DocumentStatus};
pub use printing::{
    BatchPrintReport, JobOutcome, LpPrintBackend, PrintBackend, PrintError, PrintSettings,
    PrinterInfo,
};
pub use queue::{AddFailure, AddReport, DocumentQueue, QueueError, SortCriterion, SortDirection};
