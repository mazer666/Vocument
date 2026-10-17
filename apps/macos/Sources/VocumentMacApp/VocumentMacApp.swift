import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native macOS 15+ application entry point (`ARCHITECTURE.md` §3, `VIEW-002` Comfort/Professional Reader preset).
/// Uses SwiftUI + AppKit integration and queries `DocumentCore` across UniFFI without hardcoding product state.
@main
struct VocumentMacApp: App {
    private let coreClient = DocumentCoreClient()

    var body: some Scene {
        WindowGroup("Vocument") {
            CalmReaderBootstrapView(coreClient: coreClient)
                .frame(minWidth: 720, minHeight: 480)
        }
    }
}

struct CalmReaderBootstrapView: View {
    let coreClient: DocumentCoreClient

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Vocument")
                .font(.title)
                .bold()

            switch Result(catching: { try coreClient.performHandshake() }) {
            case .success(let handshake):
                Text("Milestone \(handshake.milestone) — Baseline v\(handshake.baselineVersion)")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Divider()
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                    GridRow {
                        Text("Offline-First Policy:")
                        Text(handshake.strictOfflineDefault ? "Active (deny_all)" : "Unexpected")
                    }
                    GridRow {
                        Text("Telemetry:")
                        Text(handshake.telemetryEnabled ? "Enabled" : "Disabled (LOG-001)")
                    }
                    GridRow {
                        Text("Target Render Rate:")
                        Text("\(handshake.targetFps) FPS")
                    }
                    GridRow {
                        Text("Default German Variant:")
                        Text(handshake.defaultGermanVariant)
                    }
                    GridRow {
                        Text("Built-in OCR Languages:")
                        Text(handshake.builtInOcrLanguages.joined(separator: ", "))
                    }
                }
            case .failure(let error):
                Text("DocumentCore bootstrap failed: \(error.localizedDescription)")
                    .foregroundStyle(.red)
            }
        }
        .padding(24)
    }
}
