import SwiftUI
import AppKit
import Combine
import UniformTypeIdentifiers
import VocumentMacKit
import VocumentFFI

/// Observable document session view-model for macOS (`ARCHITECTURE.md` §3, `ROADMAP.md` M0.1).
/// Uses `ObservableObject` so it compiles cleanly under both Xcode and standalone Command Line Tools.
@MainActor
final class MacDocumentSessionCoordinator: ObservableObject {
    private let coreClient = DocumentCoreClient()
    @Published var activeSession: FfiDocumentSessionSnapshot?
    @Published var errorMessage: String?

    func openDocumentViaPanel() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        if panel.runModal() == .OK, let url = panel.url {
            openDocument(at: url)
        }
    }

    func openDocument(at url: URL) {
        do {
            if let existing = activeSession {
                try? coreClient.closeSession(sessionId: existing.sessionId)
            }
            activeSession = try coreClient.openScopedDocument(at: url)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

/// Native macOS 15+ application entry point (`ARCHITECTURE.md` §3, `VIEW-002` Comfort/Professional Reader preset).
@main
struct VocumentMacApp: App {
    @ObservedObject private var coordinator = MacDocumentSessionCoordinator()

    var body: some Scene {
        WindowGroup("Vocument") {
            CalmReaderSessionView(coordinator: coordinator)
                .frame(minWidth: 760, minHeight: 520)
        }
    }
}

struct CalmReaderSessionView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("Vocument")
                    .font(.title)
                    .bold()
                Spacer()
                Button("Open PDF…") {
                    coordinator.openDocumentViaPanel()
                }
            }

            if let errorMessage = coordinator.errorMessage {
                Text(errorMessage)
                    .foregroundStyle(.red)
            }

            if let session = coordinator.activeSession {
                GroupBox("Active DocumentSession (\(session.sessionId))") {
                    Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                        GridRow {
                            Text("Display Name:")
                            Text(session.fileIdentity.displayName).bold()
                        }
                        GridRow {
                            Text("Stable Document ID:")
                            Text(session.documentId).monospaced()
                        }
                        GridRow {
                            Text("SHA-256 Fingerprint:")
                            Text(session.fileIdentity.sha256Hex).monospaced().lineLimit(1)
                        }
                        GridRow {
                            Text("Page Count:")
                            Text("\(session.pdfSummary.pageCount)")
                        }
                        GridRow {
                            Text("PDF Format / Engine:")
                            Text("\(session.pdfSummary.formatVersion ?? "PDF") (\(session.pdfSummary.engineIdentifier))")
                        }
                        GridRow {
                            Text("Title / Author:")
                            Text("\(session.pdfSummary.title ?? "—") / \(session.pdfSummary.author ?? "—")")
                        }
                    }
                    .padding(8)
                }
            } else {
                Text("No document open. Choose 'Open PDF…' or open a file from Finder.")
                    .foregroundStyle(.secondary)
            }
        }
        .padding(24)
        .onOpenURL { url in
            coordinator.openDocument(at: url)
        }
    }
}
