import AppKit
import Foundation

@MainActor
final class AppModel: ObservableObject {
    @Published private(set) var snapshot = QueueSnapshot.empty
    @Published private(set) var printers: [PrinterInfo] = []
    @Published var selectedPrinter = ""
    @Published var selectedDocumentID: String?
    @Published var copies = 1
    @Published var grayscale = false
    @Published var duplex = false
    @Published var landscape = false
    @Published var fitToPage = true
    @Published var paperSize = ""
    @Published var isBusy = false
    @Published var notice: Notice?

    struct Notice: Identifiable {
        enum Kind { case information, success, error }
        let id = UUID()
        let kind: Kind
        let message: String
    }

    private let engine: RustEngine
    private let engineQueue = DispatchQueue(label: "com.massprint.engine", qos: .userInitiated)
    private let cancellationQueue = DispatchQueue(label: "com.massprint.cancellation", qos: .userInitiated)

    init(printers suppliedPrinters: [PrinterInfo]? = nil) {
        do {
            engine = try RustEngine()
        } catch {
            fatalError("Cannot initialize Rust engine: \(error.localizedDescription)")
        }
        FileIngress.shared.install { [weak self] paths in
            self?.add(paths: paths)
        }
        refresh()
        if let suppliedPrinters {
            printers = suppliedPrinters
            selectedPrinter = suppliedPrinters.first(where: \.isDefault)?.name ?? suppliedPrinters.first?.name ?? ""
        } else {
            loadPrinters()
        }
    }

    func chooseFiles() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.allowedContentTypes = []
        panel.message = "Choose documents to add to the print queue"
        if panel.runModal() == .OK {
            add(paths: panel.urls.map(\.path))
        }
    }

    func add(urls: [URL]) {
        add(paths: urls.filter(\.isFileURL).map(\.path))
    }

    func add(paths: [String]) {
        guard !paths.isEmpty else { return }
        runInBackground(
            request: ["command": "addPaths", "paths": paths],
            as: AddReport.self,
            refreshAfter: true
        ) { [weak self] report in
            guard let self else { return }
            if report.rejected.isEmpty {
                let noun = report.added.count == 1 ? "document" : "documents"
                self.notice = Notice(
                    kind: .success,
                    message: "Added \(report.added.count.formatted()) \(noun) to the queue."
                )
            } else {
                self.notice = Notice(
                    kind: .error,
                    message: "Added \(report.added.count); rejected \(report.rejected.count): \(report.rejected.first?.reason ?? "Unsupported file")"
                )
            }
        }
    }

    func remove(id: String) {
        perform(["command": "remove", "id": id])
    }

    func clear() {
        perform(["command": "clear"])
    }

    func reorder(id: String, before targetID: String?) {
        guard !isBusy, let source = snapshot.documents.firstIndex(where: { $0.id == id }) else { return }
        let rawTarget = targetID.flatMap { target in
            snapshot.documents.firstIndex(where: { $0.id == target })
        } ?? snapshot.documents.count
        var destination = rawTarget
        if source < rawTarget { destination -= 1 }
        destination = max(0, min(destination, snapshot.documents.count - 1))
        guard destination != source else { return }
        perform(["command": "reorder", "id": id, "destination": destination])
    }

    func canMoveDocument(id: String?, offset: Int) -> Bool {
        guard !isBusy, abs(offset) == 1, let id,
              let index = snapshot.documents.firstIndex(where: { $0.id == id }) else { return false }
        return snapshot.documents.indices.contains(index + offset)
    }

    func moveDocument(id: String, offset: Int) {
        guard canMoveDocument(id: id, offset: offset),
              let source = snapshot.documents.firstIndex(where: { $0.id == id }) else { return }
        let beforeIndex = source + offset + (offset > 0 ? 1 : 0)
        selectedDocumentID = id
        reorder(id: id, before: snapshot.documents.indices.contains(beforeIndex)
            ? snapshot.documents[beforeIndex].id : nil)
    }

    func moveSelectedDocument(offset: Int) {
        guard let selectedDocumentID else { return }
        moveDocument(id: selectedDocumentID, offset: offset)
    }

    private func apply(_ snapshot: QueueSnapshot) {
        self.snapshot = snapshot
        if !snapshot.documents.contains(where: { $0.id == selectedDocumentID }) {
            selectedDocumentID = snapshot.documents.first?.id
        }
    }

    func sort(criterion: SortCriterion, direction: SortDirection) {
        perform([
            "command": "sort",
            "criterion": criterion.rawValue,
            "direction": direction.rawValue,
        ])
    }

    func reverseSort() {
        guard let active = snapshot.activeSort else { return }
        sort(
            criterion: active.criterion,
            direction: active.direction == .ascending ? .descending : .ascending
        )
    }

    func printAll() {
        guard !snapshot.documents.isEmpty else { return }
        guard !selectedPrinter.isEmpty else {
            notice = Notice(kind: .error, message: "Select a printer first.")
            return
        }
        let settings: [String: Any] = [
            "printer": selectedPrinter,
            "copies": copies,
            "grayscale": grayscale,
            "duplex": duplex,
            "paperSize": paperSize.isEmpty ? NSNull() : paperSize,
            "landscape": landscape,
            "fitToPage": fitToPage,
        ]
        isBusy = true
        runInBackground(
            request: ["command": "print", "settings": settings],
            as: BatchPrintReport.self,
            refreshAfter: true
        ) { [weak self] report in
            guard let self else { return }
            self.isBusy = false
            let submitted = report.jobs.filter { $0.outcome == .submitted }.count
            let failed = report.jobs.filter { $0.outcome == .failed }.count
            let canceled = report.jobs.filter { $0.outcome == .canceled }.count
            self.notice = Notice(
                kind: failed == 0 ? .success : .error,
                message: "\(submitted.formatted()) submitted · \(failed.formatted()) failed · \(canceled.formatted()) canceled"
            )
        }
    }

    func cancelPrinting() {
        notice = Notice(kind: .information, message: "Cancellation requested.")
        let engine = self.engine
        cancellationQueue.async { [weak self] in
            do {
                let _: Bool = try engine.call(["command": "cancel"])
            } catch {
                DispatchQueue.main.async {
                    self?.show(error)
                }
            }
        }
    }

    func refresh() {
        runInBackground(request: ["command": "snapshot"], as: QueueSnapshot.self) { [weak self] snapshot in
            self?.apply(snapshot)
        }
    }

    func loadPrinters() {
        runInBackground(request: ["command": "printers"], as: [PrinterInfo].self) { [weak self] printers in
            guard let self else { return }
            self.printers = printers
            if self.selectedPrinter.isEmpty {
                self.selectedPrinter = printers.first(where: \.isDefault)?.name ?? printers.first?.name ?? ""
            }
        }
    }

    private func perform(_ request: [String: Any], refreshAfter: Bool = true) {
        let engine = self.engine
        engineQueue.async { [weak self] in
            do {
                let _: Bool = try engine.call(request)
                let snapshot: QueueSnapshot? = if refreshAfter {
                    try engine.call(["command": "snapshot"], as: QueueSnapshot.self)
                } else {
                    nil
                }
                DispatchQueue.main.async {
                    if let snapshot {
                        self?.apply(snapshot)
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    self?.show(error)
                }
            }
        }
    }

    private func runInBackground<Value: Decodable & Sendable>(
        request: [String: Any],
        as type: Value.Type,
        refreshAfter: Bool = false,
        completion: @escaping @MainActor (Value) -> Void
    ) {
        let engine = self.engine
        engineQueue.async { [weak self] in
            do {
                let value = try engine.call(request, as: type)
                let snapshot: QueueSnapshot? = if refreshAfter {
                    try engine.call(["command": "snapshot"], as: QueueSnapshot.self)
                } else {
                    nil
                }
                DispatchQueue.main.async {
                    guard self != nil else { return }
                    if let snapshot {
                        self?.apply(snapshot)
                    }
                    completion(value)
                }
            } catch {
                DispatchQueue.main.async {
                    self?.isBusy = false
                    self?.show(error)
                }
            }
        }
    }

    private func show(_ error: Error) {
        notice = Notice(kind: .error, message: error.localizedDescription)
    }
}
