import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native macOS 15+ application entry point (`ARCHITECTURE.md` §3, `VIEW-002` Comfort/Professional Reader preset).
@main
struct VocumentMacApp: App {
    @ObservedObject private var coordinator = MacDocumentSessionCoordinator()

    var body: some Scene {
        WindowGroup("Vocument") {
            CalmReaderSessionView(coordinator: coordinator)
                .frame(minWidth: 1080, minHeight: 720)
        }
    }
}

struct CalmReaderSessionView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Vocument")
                    .font(.title2)
                    .bold()
                if let session = coordinator.activeSession {
                    Text("• \(session.fileIdentity.displayName) (\(session.documentId))")
                        .foregroundStyle(.secondary)
                }
                Text(
                    "• \(coordinator.buildInfo.productName) \(coordinator.buildInfo.version)"
                        + " — source \(String(coordinator.buildInfo.sourceRevision.prefix(12)))"
                        + " (\(coordinator.buildInfo.licence))"
                )
                .foregroundStyle(.secondary)
                Spacer()
                Button("Open PDF…") {
                    coordinator.openDocumentViaPanel()
                }
            }

            if let errorMessage = coordinator.errorMessage {
                Text(errorMessage)
                    .foregroundStyle(.red)
            }

            if let recoveryWarning = coordinator.recoveryWarning {
                Text(recoveryWarning)
                    .foregroundStyle(.red)
            }

            if coordinator.activeSession != nil {
                HStack(alignment: .top, spacing: 16) {
                    NavigationAndSearchSidebarView(coordinator: coordinator)
                    DocumentCanvasView(coordinator: coordinator)
                    ReviewPanelView(coordinator: coordinator)
                }
            } else {
                Text("No document open. Choose 'Open PDF…' or open a file from Finder.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
            }
        }
        .padding(20)
        .onOpenURL { url in
            coordinator.openDocument(at: url)
        }
    }
}
