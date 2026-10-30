use crate::{DocumentEntry, DocumentStatus};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortCriterion {
    Original,
    Name,
    Modified,
    Size,
    Pages,
    FileType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentQueue {
    documents: Vec<DocumentEntry>,
    next_insertion_sequence: u64,
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
                Ok(mut entry) => {
                    entry.insertion_sequence = self.next_insertion_sequence;
                    self.next_insertion_sequence = self.next_insertion_sequence.saturating_add(1);
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

    pub fn sort(&mut self, criterion: SortCriterion, direction: SortDirection) {
        if criterion == SortCriterion::Name {
            self.documents.sort_by_cached_key(|document| NameSortKey {
                name: NaturalSortKey::new(&document.name),
                insertion_sequence: document.insertion_sequence,
                direction,
            });
        } else {
            self.documents
                .sort_by(|left, right| compare_documents(left, right, criterion, direction));
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct NameSortKey {
    name: NaturalSortKey,
    insertion_sequence: u64,
    direction: SortDirection,
}

impl Ord for NameSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        apply_direction(self.name.cmp(&other.name), self.direction)
            .then_with(|| self.insertion_sequence.cmp(&other.insertion_sequence))
    }
}

impl PartialOrd for NameSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_documents(
    left: &DocumentEntry,
    right: &DocumentEntry,
    criterion: SortCriterion,
    direction: SortDirection,
) -> Ordering {
    let primary = match criterion {
        SortCriterion::Original => left.insertion_sequence.cmp(&right.insertion_sequence),
        SortCriterion::Name => natural_compare(&left.name, &right.name),
        SortCriterion::Modified => {
            return compare_optional(
                left.modified_at_ms,
                right.modified_at_ms,
                left,
                right,
                direction,
            );
        }
        SortCriterion::Size => left.size_bytes.cmp(&right.size_bytes),
        SortCriterion::Pages => {
            return compare_optional(left.page_count, right.page_count, left, right, direction);
        }
        SortCriterion::FileType => left
            .extension
            .to_lowercase()
            .cmp(&right.extension.to_lowercase()),
    };

    apply_direction(primary, direction)
        .then_with(|| natural_compare(&left.name, &right.name))
        .then_with(|| left.insertion_sequence.cmp(&right.insertion_sequence))
}

fn compare_optional<T: Ord + Copy>(
    left_value: Option<T>,
    right_value: Option<T>,
    left: &DocumentEntry,
    right: &DocumentEntry,
    direction: SortDirection,
) -> Ordering {
    match (left_value, right_value) {
        (Some(left_value), Some(right_value)) => {
            apply_direction(left_value.cmp(&right_value), direction)
                .then_with(|| natural_compare(&left.name, &right.name))
                .then_with(|| left.insertion_sequence.cmp(&right.insertion_sequence))
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => natural_compare(&left.name, &right.name)
            .then_with(|| left.insertion_sequence.cmp(&right.insertion_sequence)),
    }
}

fn apply_direction(ordering: Ordering, direction: SortDirection) -> Ordering {
    match direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
    }
}

fn natural_compare(left: &str, right: &str) -> Ordering {
    NaturalSortKey::new(left).cmp(&NaturalSortKey::new(right))
}

#[derive(Debug, Eq, PartialEq)]
struct NaturalSortKey(Vec<NaturalSegment>);

impl NaturalSortKey {
    fn new(value: &str) -> Self {
        Self(natural_segments(&normalized_sort_name(value)))
    }
}

impl Ord for NaturalSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        for (left_segment, right_segment) in self.0.iter().zip(&other.0) {
            let ordering = match (left_segment, right_segment) {
                (NaturalSegment::Number(left), NaturalSegment::Number(right)) => {
                    compare_number_segments(left, right)
                }
                (NaturalSegment::Text(left), NaturalSegment::Text(right)) => left.cmp(right),
                (NaturalSegment::Number(_), NaturalSegment::Text(_)) => Ordering::Less,
                (NaturalSegment::Text(_), NaturalSegment::Number(_)) => Ordering::Greater,
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        self.0.len().cmp(&other.0.len())
    }
}

impl PartialOrd for NaturalSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn normalized_sort_name(value: &str) -> String {
    value
        .nfkd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
enum NaturalSegment {
    Number(String),
    Text(String),
}

fn natural_segments(value: &str) -> Vec<NaturalSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut digits = None;

    for character in value.chars() {
        let is_digit = character.is_ascii_digit();
        if digits.is_some_and(|current_is_digit| current_is_digit != is_digit) {
            push_segment(
                &mut segments,
                std::mem::take(&mut current),
                digits.unwrap_or(false),
            );
        }
        digits = Some(is_digit);
        current.push(character);
    }
    if !current.is_empty() {
        push_segment(&mut segments, current, digits.unwrap_or(false));
    }
    segments
}

fn push_segment(segments: &mut Vec<NaturalSegment>, value: String, digits: bool) {
    segments.push(if digits {
        NaturalSegment::Number(value)
    } else {
        NaturalSegment::Text(value)
    });
}

fn compare_number_segments(left: &str, right: &str) -> Ordering {
    let left_trimmed = left.trim_start_matches('0');
    let right_trimmed = right.trim_start_matches('0');
    let left_value = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_value = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };
    left_value
        .len()
        .cmp(&right_value.len())
        .then_with(|| left_value.cmp(right_value))
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

    fn names(queue: &DocumentQueue) -> Vec<&str> {
        queue
            .documents()
            .iter()
            .map(|item| item.name.as_str())
            .collect()
    }

    #[test]
    fn natural_name_sort_handles_numbers_case_and_accents() {
        let (_directory, mut queue) = queue_with_files(&[
            "Chapter 10.txt",
            "résumé.txt",
            "Chapter 2.txt",
            "Resume.txt",
            "Chapter 1.txt",
        ]);
        queue.sort(SortCriterion::Name, SortDirection::Ascending);
        assert_eq!(
            names(&queue),
            [
                "Chapter 1.txt",
                "Chapter 2.txt",
                "Chapter 10.txt",
                "résumé.txt",
                "Resume.txt",
            ]
        );
        queue.sort(SortCriterion::Name, SortDirection::Descending);
        assert_eq!(
            &names(&queue)[..3],
            ["résumé.txt", "Resume.txt", "Chapter 10.txt"]
        );
    }

    #[test]
    fn sorts_size_type_pages_and_restores_insertion_order() {
        let directory = tempdir().unwrap();
        let small = directory.path().join("small.txt");
        let large = directory.path().join("large.txt");
        let office = directory.path().join("sheet.xlsx");
        fs::write(&small, "one line\n").unwrap();
        fs::write(&large, "line\n".repeat(120)).unwrap();
        fs::write(&office, "unknown pages").unwrap();
        let mut queue = DocumentQueue::default();
        queue.add_paths([&large, &office, &small]);

        queue.sort(SortCriterion::Size, SortDirection::Ascending);
        assert_eq!(names(&queue)[0], "small.txt");
        queue.sort(SortCriterion::Pages, SortDirection::Descending);
        assert_eq!(names(&queue), ["large.txt", "small.txt", "sheet.xlsx"]);
        queue.sort(SortCriterion::FileType, SortDirection::Ascending);
        assert_eq!(names(&queue), ["large.txt", "small.txt", "sheet.xlsx"]);
        queue.sort(SortCriterion::Original, SortDirection::Ascending);
        assert_eq!(names(&queue), ["large.txt", "sheet.xlsx", "small.txt"]);
    }

    #[test]
    fn sorting_preserves_ids_metadata_counts_and_print_order() {
        let (_directory, mut queue) = queue_with_files(&["Document 10.txt", "Document 2.txt"]);
        let ids: HashSet<_> = queue.documents().iter().map(|item| item.id).collect();
        let pages = queue.total_known_pages();
        queue.sort(SortCriterion::Name, SortDirection::Ascending);
        assert_eq!(names(&queue), ["Document 2.txt", "Document 10.txt"]);
        assert_eq!(
            queue
                .documents()
                .iter()
                .map(|item| item.id)
                .collect::<HashSet<_>>(),
            ids
        );
        assert_eq!(queue.total_known_pages(), pages);
    }

    #[test]
    fn sorts_real_modification_metadata_and_keeps_missing_values_last() {
        let (_directory, mut queue) = queue_with_files(&["new.txt", "unknown.txt", "old.txt"]);
        queue.documents[0].modified_at_ms = Some(300);
        queue.documents[1].modified_at_ms = None;
        queue.documents[2].modified_at_ms = Some(100);

        queue.sort(SortCriterion::Modified, SortDirection::Ascending);
        assert_eq!(names(&queue), ["old.txt", "new.txt", "unknown.txt"]);
        queue.sort(SortCriterion::Modified, SortDirection::Descending);
        assert_eq!(names(&queue), ["new.txt", "old.txt", "unknown.txt"]);
    }

    #[test]
    fn new_files_can_be_resorted_without_disturbing_existing_entries() {
        let directory = tempdir().unwrap();
        for name in ["Chapter 10.txt", "Chapter 1.txt", "Chapter 2.txt"] {
            fs::write(directory.path().join(name), name).unwrap();
        }
        let mut queue = DocumentQueue::default();
        queue.add_paths([
            directory.path().join("Chapter 10.txt"),
            directory.path().join("Chapter 1.txt"),
        ]);
        queue.sort(SortCriterion::Name, SortDirection::Ascending);
        queue.add_paths([directory.path().join("Chapter 2.txt")]);
        queue.sort(SortCriterion::Name, SortDirection::Ascending);
        assert_eq!(
            names(&queue),
            ["Chapter 1.txt", "Chapter 2.txt", "Chapter 10.txt"]
        );
    }
}
