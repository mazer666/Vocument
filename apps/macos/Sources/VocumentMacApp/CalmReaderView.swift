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
                if coordinator.activeSession != nil {
                    Picker("Mode", selection: $coordinator.activeViewMode) {
                        ForEach(MacAppViewMode.allCases) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .frame(width: 320)
                    .help("Switch between Comfort Reader, Professional Review, and Page Workbench modes (ROADMAP M3.1)")
                }
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
                switch coordinator.activeViewMode {
                case .reader:
                    HStack(alignment: .top, spacing: 16) {
                        NavigationAndSearchSidebarView(coordinator: coordinator)
                        DocumentCanvasView(coordinator: coordinator)
                    }
                case .review:
                    HStack(alignment: .top, spacing: 16) {
                        NavigationAndSearchSidebarView(coordinator: coordinator)
                        DocumentCanvasView(coordinator: coordinator)
                        ReviewPanelView(coordinator: coordinator)
                    }
                case .workbench:
                    PageWorkbenchView(coordinator: coordinator)
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
