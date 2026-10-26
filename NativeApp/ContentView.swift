import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ContentView: View {
    @ObservedObject var model: AppModel
    @State private var externalDropTargeted = false
    @State private var insertionTargetID: String?

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            if let notice = model.notice {
                NoticeView(notice: notice)
                    .padding(.horizontal, 22)
                    .padding(.bottom, 10)
            }
            queue
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding(.horizontal, 22)
                .padding(.bottom, 16)
            Divider()
            printControls
        }
        .frame(minWidth: 860, idealWidth: 1040, minHeight: 600, idealHeight: 720)
        .background(Color(nsColor: .windowBackgroundColor))
        .onDrop(of: [.fileURL], isTargeted: $externalDropTargeted) { providers in
            loadDroppedFiles(providers)
            return true
        }
    }

    private var toolbar: some View {
        HStack(spacing: 14) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 50, height: 50)
            VStack(alignment: .leading, spacing: 2) {
                Text("Mass Print").font(.largeTitle.bold())
                Text("Batch printing, in your order.").foregroundStyle(.secondary)
            }
            Spacer()
            Button("Clear Queue", action: model.clear)
                .disabled(model.snapshot.documents.isEmpty || model.isBusy)
            Button(action: model.chooseFiles) {
                Label("Add Files", systemImage: "plus")
            }
            .buttonStyle(.borderedProminent)
            .disabled(model.isBusy)
        }
        .padding(22)
    }

    @ViewBuilder
    private var queue: some View {
        if model.snapshot.documents.isEmpty {
            VStack(spacing: 14) {
                Image(systemName: externalDropTargeted ? "arrow.down.doc.fill" : "doc.on.doc")
                    .font(.system(size: 48))
                    .foregroundStyle(.tint)
                Text(externalDropTargeted ? "Drop to add to the queue" : "Drop files here to print")
                    .font(.title2.bold())
                Text("PDF, images, text, and Office documents")
                    .foregroundStyle(.secondary)
                Button("Choose Files", action: model.chooseFiles)
                    .buttonStyle(.borderedProminent)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 14))
            .overlay {
                RoundedRectangle(cornerRadius: 14)
                    .stroke(externalDropTargeted ? Color.accentColor : Color.secondary.opacity(0.25), lineWidth: externalDropTargeted ? 3 : 1)
            }
        } else {
            VStack(spacing: 0) {
                sortBar
                Divider()
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(model.snapshot.documents) { document in
                            DocumentRow(
                                document: document,
                                showsInsertionIndicator: insertionTargetID == document.id
                            ) {
                                model.remove(id: document.id)
                            }
                            .draggable(document.id) {
                                DocumentRow(document: document, remove: {})
                                    .frame(width: 420)
                                    .opacity(0.85)
                            }
                            .dropDestination(for: String.self) { ids, _ in
                                guard let id = ids.first else { return false }
                                model.reorder(id: id, before: document.id)
                                return true
                            } isTargeted: { targeted in
                                insertionTargetID = targeted ? document.id : nil
                            }
                            Divider().padding(.leading, 52)
                        }
                        Color.clear
                            .frame(height: 28)
                            .contentShape(Rectangle())
                            .dropDestination(for: String.self) { ids, _ in
                                guard let id = ids.first else { return false }
                                model.reorder(id: id, before: nil)
                                return true
                            }
                    }
                }
            }
            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 14))
            .overlay {
                RoundedRectangle(cornerRadius: 14)
                    .stroke(externalDropTargeted ? Color.accentColor : Color.secondary.opacity(0.22), lineWidth: externalDropTargeted ? 3 : 1)
            }
        }
    }

    private var sortBar: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("\(model.snapshot.documents.count) Documents").font(.headline)
                Text(orderDescription).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            Text("Sort by").foregroundStyle(.secondary)
            Picker("Sort by", selection: sortCriterionBinding) {
                if model.snapshot.activeSort == nil {
                    Text("Custom Order").tag(Optional<SortCriterion>.none)
                }
                ForEach(SortCriterion.allCases) { criterion in
                    Text(criterion.title).tag(Optional(criterion))
                }
            }
            .labelsHidden()
            .frame(width: 175)
            Button(action: model.reverseSort) {
                Image(systemName: model.snapshot.activeSort?.direction == .descending ? "arrow.down" : "arrow.up")
            }
            .help("Reverse sort direction")
            .disabled(model.snapshot.activeSort == nil)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private var printControls: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text("\(model.snapshot.documents.count) documents").font(.headline)
                Text(pageSummary).foregroundStyle(.secondary)
            }
            .frame(width: 150, alignment: .leading)
            Picker("Printer", selection: $model.selectedPrinter) {
                ForEach(model.printers) { printer in
                    Text(printer.isDefault ? "\(printer.name) — Default" : printer.name)
                        .tag(printer.name)
                }
            }
            .frame(minWidth: 240)
            Stepper("Copies: \(model.copies)", value: $model.copies, in: 1...999)
                .frame(width: 125)
            Toggle("Grayscale", isOn: $model.grayscale)
            Toggle("Two-sided", isOn: $model.duplex)
            Spacer()
            if model.isBusy {
                Button("Cancel", action: model.cancelPrinting)
            }
            Button("Print All", action: model.printAll)
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(model.snapshot.documents.isEmpty || model.selectedPrinter.isEmpty || model.isBusy)
        }
        .padding(20)
    }

    private var sortCriterionBinding: Binding<SortCriterion?> {
        Binding(
            get: { model.snapshot.activeSort?.criterion },
            set: { criterion in
                guard let criterion else { return }
                model.sort(criterion: criterion, direction: .ascending)
            }
        )
    }

    private var orderDescription: String {
        guard let sort = model.snapshot.activeSort else { return "Custom Order" }
        return "\(sort.criterion.title) · \(sort.direction == .ascending ? "Ascending" : "Descending")"
    }

    private var pageSummary: String {
        if model.snapshot.unknownPageCount == 0 {
            return "\(model.snapshot.totalKnownPages) pages"
        }
        return "\(model.snapshot.totalKnownPages) known · \(model.snapshot.unknownPageCount) unknown"
    }

    private func loadDroppedFiles(_ providers: [NSItemProvider]) {
        for provider in providers where provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
            provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, _ in
                let url: URL?
                if let data = item as? Data {
                    url = URL(dataRepresentation: data, relativeTo: nil)
                } else if let value = item as? URL {
                    url = value
                } else {
                    url = nil
                }
                if let url {
                    Task { @MainActor in model.add(urls: [url]) }
                }
            }
        }
    }
}

private struct DocumentRow: View {
    let document: DocumentEntry
    var showsInsertionIndicator = false
    let remove: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "line.3.horizontal")
                .foregroundStyle(.secondary)
                .help("Drag to reorder")
            Image(nsImage: NSWorkspace.shared.icon(forFile: document.path))
                .resizable()
                .frame(width: 30, height: 30)
            VStack(alignment: .leading, spacing: 3) {
                Text(document.name).lineLimit(1)
                Text(metadata).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if document.status == .failed {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                    .help(document.statusMessage ?? "Printing failed")
            }
            Button(action: remove) {
                Image(systemName: "xmark.circle.fill")
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
            .help("Remove from queue")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .overlay(alignment: .top) {
            if showsInsertionIndicator {
                Rectangle()
                    .fill(Color.accentColor)
                    .frame(height: 2)
                    .padding(.horizontal, 8)
            }
        }
    }

    private var metadata: String {
        let pages = document.pageCount.map { "\($0) page\($0 == 1 ? "" : "s")" } ?? "Pages unknown"
        return "\(document.extension) · \(pages) · \(document.sizeBytes.fileSizeText)"
    }
}

private struct NoticeView: View {
    let notice: AppModel.Notice

    var body: some View {
        HStack {
            Image(systemName: icon)
            Text(notice.message)
            Spacer()
        }
        .padding(10)
        .foregroundStyle(color)
        .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
    }

    private var color: Color {
        switch notice.kind {
        case .information: return .accentColor
        case .success: return .green
        case .error: return .red
        }
    }

    private var icon: String {
        switch notice.kind {
        case .information: return "info.circle.fill"
        case .success: return "checkmark.circle.fill"
        case .error: return "exclamationmark.triangle.fill"
        }
    }
}
