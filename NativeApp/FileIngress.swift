import AppKit

final class FileIngress {
    static let shared = FileIngress()

    private var pending: [String] = []
    private var receiver: (([String]) -> Void)?
    private var scheduledDelivery: DispatchWorkItem?
    private let lock = NSLock()

    private init() {}

    func install(receiver: @escaping ([String]) -> Void) {
        lock.lock()
        self.receiver = receiver
        scheduleDeliveryLocked()
        lock.unlock()
    }

    func receive(_ paths: [String]) {
        guard !paths.isEmpty else { return }
        lock.lock()
        pending.append(contentsOf: paths)
        scheduleDeliveryLocked()
        lock.unlock()
    }

    private func scheduleDeliveryLocked() {
        guard receiver != nil else { return }
        scheduledDelivery?.cancel()
        let delivery = DispatchWorkItem { [weak self] in
            self?.deliverPendingFiles()
        }
        scheduledDelivery = delivery
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: delivery)
    }

    private func deliverPendingFiles() {
        lock.lock()
        guard let receiver, !pending.isEmpty else {
            scheduledDelivery = nil
            lock.unlock()
            return
        }
        let paths = pending
        pending.removeAll()
        scheduledDelivery = nil
        lock.unlock()
        receiver(paths)
    }
}

final class FinderServiceProvider: NSObject {
    static let shared = FinderServiceProvider()

    @objc func queueFiles(
        _ pasteboard: NSPasteboard,
        userData: String,
        error: AutoreleasingUnsafeMutablePointer<NSString?>
    ) {
        let urls = pasteboard.readObjects(
            forClasses: [NSURL.self],
            options: [.urlReadingFileURLsOnly: true]
        ) as? [URL] ?? []
        let paths = urls.filter(\.isFileURL).map(\.path)
        guard !paths.isEmpty else {
            error.pointee = "No files were provided by Finder."
            return
        }
        FileIngress.shared.receive(paths)
        NSApplication.shared.activate(ignoringOtherApps: true)
        NSApplication.shared.windows.first?.makeKeyAndOrderFront(nil)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        removeLegacyWorkflow()
        NSApplication.shared.servicesProvider = FinderServiceProvider.shared
        NSUpdateDynamicServices()
    }

    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        receive(filenames, in: sender)
    }

    func application(_ sender: NSApplication, open urls: [URL]) {
        receive(urls.filter(\.isFileURL).map(\.path), in: sender)
    }

    private func receive(_ paths: [String], in application: NSApplication) {
        FileIngress.shared.receive(paths)
        application.reply(toOpenOrPrint: .success)
        application.activate(ignoringOtherApps: true)
        application.windows.first?.makeKeyAndOrderFront(nil)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    private func removeLegacyWorkflow() {
        let workflow = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Services/Print with Mass Print.workflow")
        let info = workflow.appendingPathComponent("Contents/Info.plist")
        guard let contents = try? String(contentsOf: info),
              contents.contains("com.massprint.finder-action") else { return }
        try? FileManager.default.removeItem(at: workflow)
    }
}
