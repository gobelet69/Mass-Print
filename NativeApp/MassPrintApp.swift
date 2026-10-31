import SwiftUI

@main
struct MassPrintApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var model = AppModel()

    /// 111iridescence v2.1 Electric Blue primary accent (`#2266F2`)
    private static let iridescenceElectricBlue = Color(red: 34.0 / 255.0, green: 102.0 / 255.0, blue: 242.0 / 255.0)

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
                .tint(Self.iridescenceElectricBlue)
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
