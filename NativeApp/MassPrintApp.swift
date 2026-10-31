import SwiftUI

@main
struct MassPrintApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
                .tint(Color(red: 245/255, green: 158/255, blue: 11/255))
        }
        .windowStyle(.titleBar)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Add Files…", action: model.chooseFiles)
                    .keyboardShortcut("o")
            }
            CommandMenu("Queue") {
                Button("Clear Queue", action: model.clear)
                    .disabled(model.snapshot.documents.isEmpty)
                Divider()
                Button("Print All", action: model.printAll)
                    .keyboardShortcut("p")
                    .disabled(model.snapshot.documents.isEmpty || model.isBusy)
            }
        }
    }
}
