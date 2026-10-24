use crate::{DocumentEntry, DocumentStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentQueue {
    documents: Vec<DocumentEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFailure {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddReport {
    pub added: Vec<DocumentEntry>,
    pub rejected: Vec<AddFailure>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum QueueError {
    #[error("document was not found in the queue")]
    NotFound,
    #[error("destination index is outside the queue")]
    InvalidIndex,
}

impl DocumentQueue {
    pub fn documents(&self) -> &[DocumentEntry] {
        &self.documents
    }

    pub fn total_known_pages(&self) -> u32 {
        self.documents
            .iter()
            .filter_map(|item| item.page_count)
            .sum()
    }

    pub fn unknown_page_count(&self) -> usize {
        self.documents
            .iter()
            .filter(|item| item.page_count.is_none())
            .count()
    }

    pub fn add_paths<I, P>(&mut self, paths: I) -> AddReport
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut report = AddReport::default();
        let mut known: HashSet<PathBuf> = self
            .documents
            .iter()
            .map(|document| normalized_path(&document.path))
            .collect();

        for requested in paths {
            let requested = requested.as_ref();
            let normalized = normalized_path(requested);
            if known.contains(&normalized) {
                report.rejected.push(AddFailure {
                    path: requested.to_path_buf(),
                    reason: "This file is already in the queue".into(),
                });
                continue;
            }

            match DocumentEntry::inspect(normalized.clone()) {
                Ok(entry) => {
                    known.insert(normalized);
                    self.documents.push(entry.clone());
                    report.added.push(entry);
                }
                Err(reason) => report.rejected.push(AddFailure {
                    path: requested.to_path_buf(),
                    reason,
                }),
            }
        }
        report
    }

    pub fn remove(&mut self, id: Uuid) -> Result<DocumentEntry, QueueError> {
        let index = self
            .documents
            .iter()
            .position(|document| document.id == id)
            .ok_or(QueueError::NotFound)?;
        Ok(self.documents.remove(index))
    }

    pub fn clear(&mut self) {
        self.documents.clear();
    }

    pub fn reorder(&mut self, id: Uuid, destination: usize) -> Result<(), QueueError> {
        if destination >= self.documents.len() {
            return Err(QueueError::InvalidIndex);
        }
        let source = self
            .documents
            .iter()
            .position(|document| document.id == id)
            .ok_or(QueueError::NotFound)?;
        if source == destination {
            return Ok(());
        }
        let document = self.documents.remove(source);
        self.documents.insert(destination, document);
        Ok(())
    }

    pub fn move_by(&mut self, id: Uuid, delta: isize) -> Result<(), QueueError> {
        let source = self
            .documents
            .iter()
            .position(|document| document.id == id)
            .ok_or(QueueError::NotFound)?;
        let destination = source
            .saturating_add_signed(delta)
            .min(self.documents.len() - 1);
        self.reorder(id, destination)
    }

    pub fn set_status(
        &mut self,
        id: Uuid,
        status: DocumentStatus,
        message: Option<String>,
    ) -> Result<(), QueueError> {
        let document = self
            .documents
            .iter_mut()
            .find(|document| document.id == id)
            .ok_or(QueueError::NotFound)?;
        document.status = status;
        document.status_message = message;
        Ok(())
    }
}

fn normalized_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn queue_with_files(names: &[&str]) -> (tempfile::TempDir, DocumentQueue) {
        let directory = tempdir().unwrap();
        for name in names {
            fs::write(directory.path().join(name), "test\n").unwrap();
        }
        let mut queue = DocumentQueue::default();
        queue.add_paths(names.iter().map(|name| directory.path().join(name)));
        (directory, queue)
    }

    #[test]
    fn adds_multiple_files_in_input_order() {
        let (_directory, queue) = queue_with_files(&["first.txt", "second.txt"]);
        let names: Vec<_> = queue.documents().iter().map(|item| &item.name).collect();
        assert_eq!(names, ["first.txt", "second.txt"]);
    }

    #[test]
    fn rejects_duplicate_paths_without_changing_order() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("same.txt");
        fs::write(&path, "test").unwrap();
        let mut queue = DocumentQueue::default();
        assert_eq!(queue.add_paths([&path]).added.len(), 1);
        let second = queue.add_paths([&path]);
        assert_eq!(second.rejected.len(), 1);
        assert_eq!(queue.documents().len(), 1);
    }

    #[test]
    fn rejects_missing_and_unsupported_files() {
        let directory = tempdir().unwrap();
        let unsupported = directory.path().join("data.bin");
        fs::write(&unsupported, "test").unwrap();
        let mut queue = DocumentQueue::default();
        let report = queue.add_paths([directory.path().join("missing.pdf"), unsupported]);
        assert!(report.added.is_empty());
        assert_eq!(report.rejected.len(), 2);
    }

    #[test]
    fn removes_a_document() {
        let (_directory, mut queue) = queue_with_files(&["a.txt", "b.txt"]);
        let id = queue.documents()[0].id;
        let removed = queue.remove(id).unwrap();
        assert_eq!(removed.name, "a.txt");
        assert_eq!(queue.documents()[0].name, "b.txt");
    }

    #[test]
    fn reorders_by_stable_identifier() {
        let (_directory, mut queue) = queue_with_files(&["a.txt", "b.txt", "c.txt"]);
        let id = queue.documents()[2].id;
        queue.reorder(id, 0).unwrap();
        let names: Vec<_> = queue
            .documents()
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(names, ["c.txt", "a.txt", "b.txt"]);
    }

    #[test]
    fn move_by_clamps_at_queue_edges() {
        let (_directory, mut queue) = queue_with_files(&["a.txt", "b.txt"]);
        let first = queue.documents()[0].id;
        queue.move_by(first, -1).unwrap();
        assert_eq!(queue.documents()[0].name, "a.txt");
        queue.move_by(first, 9).unwrap();
        assert_eq!(queue.documents()[1].name, "a.txt");
    }

    #[test]
    fn clear_empties_the_queue() {
        let (_directory, mut queue) = queue_with_files(&["a.txt"]);
        queue.clear();
        assert!(queue.documents().is_empty());
    }

    #[test]
    fn updates_printing_status_without_changing_order() {
        let (_directory, mut queue) = queue_with_files(&["a.txt", "b.txt"]);
        let id = queue.documents()[0].id;
        queue
            .set_status(id, DocumentStatus::Submitted, Some("Office-42".into()))
            .unwrap();
        assert_eq!(queue.documents()[0].status, DocumentStatus::Submitted);
        assert_eq!(
            queue.documents()[0].status_message.as_deref(),
            Some("Office-42")
        );
        assert_eq!(queue.documents()[1].name, "b.txt");
    }

    #[test]
    fn handles_a_hundred_documents_in_exact_order() {
        let directory = tempdir().unwrap();
        let paths: Vec<_> = (0..100)
            .map(|index| {
                let path = directory.path().join(format!("document-{index:03}.txt"));
                fs::write(&path, format!("Document {index}\n")).unwrap();
                path
            })
            .collect();
        let mut queue = DocumentQueue::default();
        let report = queue.add_paths(&paths);
        assert_eq!(report.added.len(), 100);
        assert!(report.rejected.is_empty());
        assert_eq!(queue.documents()[0].name, "document-000.txt");
        assert_eq!(queue.documents()[99].name, "document-099.txt");
    }

    #[test]
    fn accepts_unicode_and_special_characters_in_paths() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Résumé – final [approved] #1.txt");
        fs::write(&path, "content").unwrap();
        let mut queue = DocumentQueue::default();
        let report = queue.add_paths([path]);
        assert_eq!(report.added[0].name, "Résumé – final [approved] #1.txt");
    }
}
