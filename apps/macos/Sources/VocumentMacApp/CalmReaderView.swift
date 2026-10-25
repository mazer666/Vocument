import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native macOS 15+ application entry point (`ARCHITECTURE.md` §3, `VIEW-002` Comfort/Professional Reader preset, `ADR-0020`).
@main
struct VocumentMacApp: App {
    @ObservedObject private var coordinator = MacDocumentSessionCoordinator()

    init() {
        if CommandLine.arguments.contains("--verify-staged") {
            VocumentStagedVerifier.runAndExit()
        }
    }

    var body: some Scene {
        WindowGroup("Vocument") {
            CalmReaderSessionView(coordinator: coordinator)
                .frame(minWidth: 1080, minHeight: 720)
        }
        .commands {
            CommandGroup(replacing: .help) {
                Button("Vocument Offline-Hilfe") {
                    coordinator.openHelp()
                }
                .keyboardShortcut("?", modifiers: [.command])
            }
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
                } else if let editorSession = coordinator.activeEditorSession {
                    Text("• \(editorSession.title) (.pdoc)")
                        .foregroundStyle(.secondary)
                }
                Text(
                    "• \(coordinator.buildInfo.productName) \(coordinator.buildInfo.version)"
                        + " — source \(String(coordinator.buildInfo.sourceRevision.prefix(12)))"
                        + " (\(coordinator.buildInfo.licence))"
                )
                .foregroundStyle(.secondary)
                Spacer()
                if coordinator.activeSession != nil || coordinator.activeEditorSession != nil {
                    Picker("Mode", selection: $coordinator.activeViewMode) {
                        ForEach(MacAppViewMode.allCases) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .frame(width: 400)
                    .help("Switch between Comfort Reader, Professional Review, Page Workbench, and Authoring Canvas modes")
                }
                Button(action: {
                    coordinator.openHelp()
                }) {
                    Image(systemName: "questionmark.circle")
                        .font(.body)
                }
                .buttonStyle(.plain)
                .help("Offline-Dokumentation und Tastaturkürzel anzeigen (HELP-002)")
                .accessibilityLabel("Vocument Hilfe")
                .accessibilityIdentifier("toolbar_help_button")

                Button("Neuer Schriftsatz") {
                    coordinator.createNewAuthoringDocument()
                }
                .help("Neues .pdoc-Schriftsatzdokument im Flow-Editor erstellen")

                Button("Open .pdoc…") {
                    coordinator.openPdocViaPanel()
                }

                Button("Open PDF…") {
                    coordinator.openDocumentViaPanel()
                }
            }

            if let errorMessage = coordinator.errorMessage {
                Text(errorMessage)
                    .foregroundStyle(.red)
            }

            if let saveNotification = coordinator.saveNotificationMessage {
                Text(saveNotification)
                    .foregroundStyle(.green)
            }

            if let recoveryWarning = coordinator.recoveryWarning {
                Text(recoveryWarning)
                    .foregroundStyle(.red)
            }

            if coordinator.activeEditorSession != nil && coordinator.activeViewMode == .authoring {
                PdocAuthoringCanvasView(coordinator: coordinator)
            } else if coordinator.activeSession != nil {
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
                case .authoring:
                    PdocAuthoringCanvasView(coordinator: coordinator)
                }
            } else {
                Text("No document open. Choose 'Neuer Schriftsatz', 'Open .pdoc…', or 'Open PDF…'.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
            }
        }
        .padding(20)
        .sheet(isPresented: $coordinator.showHelpSheet) {
            HelpSheetView(isPresented: $coordinator.showHelpSheet)
        }
        .onOpenURL { url in
            if url.pathExtension.lowercased() == "pdoc" {
                coordinator.openPdocDocument(at: url)
            } else {
                coordinator.openDocument(at: url)
            }
        }
    }
}
