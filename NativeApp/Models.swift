import Foundation

enum DocumentStatus: String, Codable {
    case ready, needsConversion, preparing, submitted, failed, canceled
}

struct DocumentEntry: Codable, Identifiable, Hashable {
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

enum SortCriterion: String, Codable, CaseIterable, Identifiable {
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

enum SortDirection: String, Codable {
    case ascending, descending
}

struct SortSelection: Codable, Equatable {
    let criterion: SortCriterion
    let direction: SortDirection
}

struct QueueSnapshot: Codable {
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

struct AddFailure: Codable {
    let path: String
    let reason: String
}

struct AddReport: Codable {
    let added: [DocumentEntry]
    let rejected: [AddFailure]
}

struct PrinterInfo: Codable, Identifiable {
    let name: String
    let isDefault: Bool
    var id: String { name }
}

struct PrintSettings: Codable {
    let printer: String
    let copies: UInt16
    let grayscale: Bool
    let duplex: Bool
    let paperSize: String?
    let landscape: Bool
    let fitToPage: Bool
}

enum JobOutcome: String, Codable {
    case submitted, failed, canceled
}

struct JobReport: Codable {
    let documentId: String
    let outcome: JobOutcome
    let spoolerJobId: String?
    let message: String
}

struct BatchPrintReport: Codable {
    let jobs: [JobReport]
}

extension UInt64 {
    var fileSizeText: String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: self), countStyle: .file)
    }
}
