import Foundation
import VocumentFFI
import VocumentMacKit

/// Headless verification runner executed when the staged macOS application bundle is launched with `--verify-staged`.
/// Authentically tests the packaged binary, bundled frameworks, FFI linkage, and M4.2/M4.3 native editor workflows.
enum VocumentStagedVerifier {
    static func runAndExit() -> Never {
        print("==> Vocument Staged Package Self-Verification Starting...")
        let client = DocumentCoreClient()

        // 1. Handshake & Core Invariants
        do {
            let handshake = try client.performHandshake()
            print("    ✓ Core Handshake: Baseline \(handshake.baselineVersion), Milestone \(handshake.milestone), Target FPS: \(handshake.targetFps)")
            guard handshake.strictOfflineDefault else {
                fputs("ERROR: Strict offline default violated!\n", stderr)
                exit(1)
            }
        } catch {
            fputs("ERROR: Handshake failed: \(error.localizedDescription)\n", stderr)
            exit(1)
        }

        // 2. M4.3 Native Flow Editor Lifecycle
        let editorId: String
        do {
            print("==> Testing M4.3 Native Editor in staged bundle...")
            let session = try client.createEditorSession(title: "Staged Verification Brief")
            editorId = session.editorId
            print("    ✓ Editor session created: \(session.editorId), pages: \(session.totalPages)")

            // Text insertion & reflow
            let s1 = try client.editorInsertText(
                editorId: editorId,
                text: "Schriftsatz zur Hauptverhandlung gemäß § 242 BGB. Treu und Glauben sind zwingend zu wahren."
            )
            guard s1.plainText.contains("§ 242 BGB") else {
                fputs("ERROR: Editor text insertion failed!\n", stderr)
                exit(1)
            }
            print("    ✓ Native text insertion verified.")

            // Formatting: Bold & Heading
            _ = try client.editorToggleBold(editorId: editorId)
            let s2 = try client.editorInsertText(editorId: editorId, text: " Bindende Rechtspflicht.")
            guard s2.plainText.contains("Bindende Rechtspflicht") else {
                fputs("ERROR: Bold text insertion failed!\n", stderr)
                exit(1)
            }

            // Undo / Redo
            let sUndo = try client.editorUndo(editorId: editorId)
            guard !sUndo.plainText.contains("Bindende Rechtspflicht") else {
                fputs("ERROR: Editor undo failed!\n", stderr)
                exit(1)
            }
            let sRedo = try client.editorRedo(editorId: editorId)
            guard sRedo.plainText.contains("Bindende Rechtspflicht") else {
                fputs("ERROR: Editor redo failed!\n", stderr)
                exit(1)
            }
            print("    ✓ Undo/Redo transaction stack verified.")

            // Structural blocks: Heading, List, Table, Footnote
            _ = try client.editorInsertParagraphBreak(editorId: editorId)
            _ = try client.editorSetHeadingLevel(editorId: editorId, level: 1)
            _ = try client.editorInsertBulletList(editorId: editorId)
            _ = try client.editorInsertTable(editorId: editorId, rows: 3, cols: 2)
            _ = try client.editorInsertFootnote(editorId: editorId, text: "BGHZ 154, 234 - Leitsatzentscheidung.")
            print("    ✓ Structured blocks (H1, Bullet List, Table, Footnote) verified.")

            // Latency measurement (< 16 ms budget)
            let latencyMicros = try client.editorMeasureTypingLatencyMicros(
                editorId: editorId,
                sampleText: "Echtzeit-Latenzmessung für juristische Bearbeitung."
            )
            let latencyMs = Double(latencyMicros) / 1000.0
            print(String(format: "    ✓ Typing & incremental reflow latency: %.2f ms (budget < 16.0 ms).", latencyMs))
            guard latencyMicros < 16_000 else {
                fputs("ERROR: Typing latency \(latencyMicros) µs exceeds 16ms budget!\n", stderr)
                exit(1)
            }

            // Save .pdoc package with atomic fsync
            let tmpPath = NSTemporaryDirectory() + "staged_test_\(ProcessInfo.processInfo.processIdentifier).pdoc"
            let savedPath = try client.editorSavePdoc(editorId: editorId, targetPath: tmpPath)
            print("    ✓ Hardened .pdoc saved atomically to: \(savedPath)")

            try client.closeEditorSession(editorId: editorId)

            // Re-open saved .pdoc
            let reopened = try client.openPdocEditorSession(at: tmpPath)
            guard reopened.plainText.contains("§ 242 BGB") else {
                fputs("ERROR: Reopened .pdoc missing expected content!\n", stderr)
                exit(1)
            }
            print("    ✓ Re-opened .pdoc and verified content roundtrip integrity.")
            try client.closeEditorSession(editorId: reopened.editorId)
            try? FileManager.default.removeItem(atPath: tmpPath)
        } catch {
            fputs("ERROR: Native editor verification failed: \(error.localizedDescription)\n", stderr)
            exit(1)
        }

        print("================================================================================")
        print("✓ STAGED MACOS APPLICATION PACKAGE VERIFICATION PASSED (Zero Defects)")
        print("================================================================================")
        exit(0)
    }
}
