import AppKit
import Foundation

@main struct NativeQueueChecks {
    @MainActor static func main() async throws {
        let _ = NSApplication.shared
        let fixture = FileManager.default.temporaryDirectory.appendingPathComponent("mass-print-native-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: fixture, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: fixture) }
        let paths = try ["A.txt", "B.txt", "C.txt"].map { name in
            let url = fixture.appendingPathComponent(name)
            try name.write(to: url, atomically: true, encoding: .utf8)
            return url.path
        }
        let model = AppModel(printers: [PrinterInfo(name: "Audit Printer", isDefault: true)])
        model.add(paths: paths)
        for _ in 0..<200 where model.snapshot.documents.count < 3 { try await Task.sleep(nanoseconds: 10_000_000) }
        precondition(model.snapshot.documents.map(\.name) == ["A.txt", "B.txt", "C.txt"], "Fixture ingress failed")
        let b = model.snapshot.documents[1].id
        model.selectedDocumentID = b
        model.moveDocument(id: b, offset: -1)
        for _ in 0..<200 where model.snapshot.documents.first?.id != b { try await Task.sleep(nanoseconds: 10_000_000) }
        precondition(model.snapshot.documents.map(\.name) == ["B.txt", "A.txt", "C.txt"])
        precondition(!model.canMoveDocument(id: b, offset: -1), "First row must not move up")
        model.moveSelectedDocument(offset: 1)
        for _ in 0..<200 where model.snapshot.documents.first?.id == b { try await Task.sleep(nanoseconds: 10_000_000) }
        precondition(model.snapshot.documents.map(\.name) == ["A.txt", "B.txt", "C.txt"])
        model.isBusy = true
        precondition(!model.canMoveDocument(id: b, offset: -1), "Busy queue must disable reordering")
        precondition(model.selectedDocumentID == b)
        let original = try String(contentsOfFile: paths[0], encoding: .utf8)
        precondition(original == "A.txt")
        print("PASS native queue ingress, move up/down, boundary/busy guards, selection and original-file preservation")
    }
}
