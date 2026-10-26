import AppKit

final class FileIngress {
    static let shared = FileIngress()

    private var pending: [String] = []
    private var receiver: (([String]) -> Void)?
    private let lock = NSLock()

    private init() {}

    func install(receiver: @escaping ([String]) -> Void) {
        lock.lock()
        self.receiver = receiver
        let paths = pending
        pending.removeAll()
        lock.unlock()

        if !paths.isEmpty {
            DispatchQueue.main.async { receiver(paths) }
        }
    }

    func receive(_ paths: [String]) {
        guard !paths.isEmpty else { return }
        lock.lock()
        if let receiver {
            lock.unlock()
            DispatchQueue.main.async { receiver(paths) }
        } else {
            pending.append(contentsOf: paths)
            lock.unlock()
        }
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
        FileIngress.shared.receive(filenames)
        sender.reply(toOpenOrPrint: .success)
        sender.activate(ignoringOtherApps: true)
        sender.windows.first?.makeKeyAndOrderFront(nil)
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
