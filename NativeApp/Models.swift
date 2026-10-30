import Foundation

enum DocumentStatus: String, Codable, Sendable {
    case ready, needsConversion, preparing, submitted, failed, canceled
}

struct DocumentEntry: Codable, Identifiable, Hashable, Sendable {
    let id: String
    let path: String
    let name: String
    let `extension`: String
    let sizeBytes: UInt64
    let modifiedAtMs: UInt64?
    let insertionSequence: UInt64
    let pageCount: UInt32?
    let format: String
    let status: DocumentStatus
    let statusMessage: String?
}

enum SortCriterion: String, Codable, CaseIterable, Identifiable, Sendable {
    case original, name, modified, size, pages, fileType

    var id: String { rawValue }

    var title: String {
        switch self {
        case .original: return "Original Order"
        case .name: return "Name"
        case .modified: return "Modified Date"
        case .size: return "File Size"
        case .pages: return "Page Count"
        case .fileType: return "File Type"
        }
    }
}

enum SortDirection: String, Codable, Sendable {
    case ascending, descending
}

struct SortSelection: Codable, Equatable, Sendable {
    let criterion: SortCriterion
    let direction: SortDirection
}

struct QueueSnapshot: Codable, Sendable {
    let documents: [DocumentEntry]
    let totalKnownPages: UInt32
    let unknownPageCount: Int
    let activeSort: SortSelection?

    static let empty = QueueSnapshot(
        documents: [],
        totalKnownPages: 0,
        unknownPageCount: 0,
        activeSort: SortSelection(criterion: .original, direction: .ascending)
    )
}

struct AddFailure: Codable, Sendable {
    let path: String
    let reason: String
}

struct AddReport: Codable, Sendable {
    let added: [DocumentEntry]
    let rejected: [AddFailure]
}

struct PrinterInfo: Codable, Identifiable, Sendable {
    let name: String
    let isDefault: Bool
    var id: String { name }
}

struct PrintSettings: Codable, Sendable {
    let printer: String
    let copies: UInt16
    let grayscale: Bool
    let duplex: Bool
    let paperSize: String?
    let landscape: Bool
    let fitToPage: Bool
}

enum JobOutcome: String, Codable, Sendable {
    case submitted, failed, canceled
}

struct JobReport: Codable, Sendable {
    let documentId: String
    let outcome: JobOutcome
    let spoolerJobId: String?
    let message: String
}

struct BatchPrintReport: Codable, Sendable {
    let jobs: [JobReport]
}

extension UInt64 {
    var fileSizeText: String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: self), countStyle: .file)
    }
}
