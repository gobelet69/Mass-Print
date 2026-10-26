import AppKit
import Foundation

@MainActor
final class AppModel: ObservableObject {
    @Published private(set) var snapshot = QueueSnapshot.empty
    @Published private(set) var printers: [PrinterInfo] = []
    @Published var selectedPrinter = ""
    @Published var copies = 1
    @Published var grayscale = false
    @Published var duplex = false
    @Published var landscape = false
    @Published var fitToPage = true
    @Published var isBusy = false
    @Published var notice: Notice?

    struct Notice: Identifiable {
        enum Kind { case information, success, error }
        let id = UUID()
        let kind: Kind
        let message: String
    }

    private let engine: RustEngine

    init() {
        do {
            engine = try RustEngine()
        } catch {
            fatalError("Cannot initialize Rust engine: \(error.localizedDescription)")
        }
        FileIngress.shared.install { [weak self] paths in
            self?.add(paths: paths)
        }
        refresh()
        loadPrinters()
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
            as: AddReport.self
        ) { [weak self] report in
            guard let self else { return }
            self.refresh()
            if report.rejected.isEmpty {
                self.notice = Notice(
                    kind: .success,
                    message: "Added \(report.added.count) document(s) to the queue."
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
        guard let source = snapshot.documents.firstIndex(where: { $0.id == id }) else { return }
        let rawTarget = targetID.flatMap { target in
            snapshot.documents.firstIndex(where: { $0.id == target })
        } ?? snapshot.documents.count
        var destination = rawTarget
        if source < rawTarget { destination -= 1 }
        destination = max(0, min(destination, snapshot.documents.count - 1))
        guard destination != source else { return }
        perform(["command": "reorder", "id": id, "destination": destination])
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
            "paperSize": NSNull(),
            "landscape": landscape,
            "fitToPage": fitToPage,
        ]
        isBusy = true
        runInBackground(
            request: ["command": "print", "settings": settings],
            as: BatchPrintReport.self
        ) { [weak self] report in
            guard let self else { return }
            self.isBusy = false
            self.refresh()
            let submitted = report.jobs.filter { $0.outcome == .submitted }.count
            let failed = report.jobs.filter { $0.outcome == .failed }.count
            let canceled = report.jobs.filter { $0.outcome == .canceled }.count
            self.notice = Notice(
                kind: failed == 0 ? .success : .error,
                message: "\(submitted) submitted, \(failed) failed, \(canceled) canceled."
            )
        }
    }

    func cancelPrinting() {
        perform(["command": "cancel"], refreshAfter: false)
        notice = Notice(kind: .information, message: "Cancellation requested.")
    }

    func refresh() {
        do {
            snapshot = try engine.call(["command": "snapshot"], as: QueueSnapshot.self)
        } catch {
            show(error)
        }
    }

    private func loadPrinters() {
        runInBackground(request: ["command": "printers"], as: [PrinterInfo].self) { [weak self] printers in
            guard let self else { return }
            self.printers = printers
            if self.selectedPrinter.isEmpty {
                self.selectedPrinter = printers.first(where: \.isDefault)?.name ?? printers.first?.name ?? ""
            }
        }
    }

    private func perform(_ request: [String: Any], refreshAfter: Bool = true) {
        do {
            let _: Bool = try engine.call(request)
            if refreshAfter { refresh() }
        } catch {
            show(error)
        }
    }

    private func runInBackground<Value: Decodable>(
        request: [String: Any],
        as type: Value.Type,
        completion: @escaping @MainActor (Value) -> Void
    ) {
        let engine = self.engine
        Task {
            do {
                let value = try await Task.detached {
                    try engine.call(request, as: type)
                }.value
                completion(value)
            } catch {
                self.isBusy = false
                self.show(error)
            }
        }
    }

    private func show(_ error: Error) {
        notice = Notice(kind: .error, message: error.localizedDescription)
    }
}
