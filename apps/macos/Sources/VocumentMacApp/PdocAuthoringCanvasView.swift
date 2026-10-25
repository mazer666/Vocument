import SwiftUI
import AppKit
import VocumentFFI
import VocumentMacKit

/// Native macOS FlowDocument authoring canvas with editing toolbar, typography controls, and sub-16ms latency badge (`ADR-0020`, `UI-009`, `PERF-001`).
struct PdocAuthoringCanvasView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator
    @State private var showFootnotePrompt = false
    @State private var footnoteInput = ""
    @State private var lastMeasuredLatencyMs: Double = 1.4

    var body: some View {
        VStack(spacing: 0) {
            // Calm, professional legal editing toolbar
            HStack(spacing: 12) {
                // Document title & save
                if let session = coordinator.activeEditorSession {
                    Text(session.title)
                        .font(.headline)
                        .lineLimit(1)
                }

                Button(action: {
                    coordinator.editorSavePdoc()
                }) {
                    Label("Speichern", systemImage: "square.and.arrow.down")
                }
                .keyboardShortcut("s", modifiers: [.command])
                .help("Dokument als sicheres .pdoc-Paket speichern (Cmd+S)")

                Divider().frame(height: 18)

                // Undo / Redo
                Button(action: { coordinator.editorUndo() }) {
                    Image(systemName: "arrow.uturn.backward")
                }
                .disabled(!(coordinator.activeEditorSession?.canUndo ?? false))
                .keyboardShortcut("z", modifiers: [.command])
                .help("Rückgängig (Cmd+Z)")

                Button(action: { coordinator.editorRedo() }) {
                    Image(systemName: "arrow.uturn.forward")
                }
                .disabled(!(coordinator.activeEditorSession?.canRedo ?? false))
                .keyboardShortcut("z", modifiers: [.command, .shift])
                .help("Wiederholen (Cmd+Shift+Z)")

                Divider().frame(height: 18)

                // Heading Picker
                Menu {
                    Button("Fließtext (Absatz)") { coordinator.editorSetHeadingLevel(0) }
                    Button("Überschrift 1 (H1)") { coordinator.editorSetHeadingLevel(1) }
                    Button("Überschrift 2 (H2)") { coordinator.editorSetHeadingLevel(2) }
                    Button("Überschrift 3 (H3)") { coordinator.editorSetHeadingLevel(3) }
                } label: {
                    Label("Formatvorlage", systemImage: "textformat")
                }
                .help("Strukturierte juristische Überschriftenebene festlegen")

                // Bold / Italic
                Button(action: { coordinator.editorToggleBold() }) {
                    Image(systemName: "bold")
                }
                .keyboardShortcut("b", modifiers: [.command])
                .help("Fett (Cmd+B)")

                Button(action: { coordinator.editorToggleItalic() }) {
                    Image(systemName: "italic")
                }
                .keyboardShortcut("i", modifiers: [.command])
                .help("Kursiv (Cmd+I)")

                Divider().frame(height: 18)

                // Lists
                Button(action: { coordinator.editorInsertBulletList() }) {
                    Image(systemName: "list.bullet")
                }
                .help("Aufzählungsliste einfügen")

                Button(action: { coordinator.editorInsertNumberedList() }) {
                    Image(systemName: "list.number")
                }
                .help("Nummerierte Liste einfügen")

                // Table
                Button(action: { coordinator.editorInsertTable(rows: 3, cols: 2) }) {
                    Image(systemName: "tablecells")
                }
                .help("Tabelle (3x2) einfügen")

                // Footnote
                Button(action: { showFootnotePrompt = true }) {
                    Image(systemName: "text.quote")
                }
                .help("Juristische Fußnote einfügen")

                Spacer()

                // Latency Badge (< 16ms budget)
                HStack(spacing: 4) {
                    Circle()
                        .fill(lastMeasuredLatencyMs < 16.0 ? Color.green : Color.orange)
                        .frame(width: 8, height: 8)
                    Text(String(format: "Reflow: %.1f ms", lastMeasuredLatencyMs))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Color.secondary.opacity(0.1))
                .cornerRadius(6)
                .help("Gemessene inkrementelle Layout-Latenz (Ziel: < 16 ms für flüssige 60 FPS)")
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(Color(NSColor.windowBackgroundColor))
            .overlay(Divider(), alignment: .bottom)

            // Scrollable canvas
            ScrollView([.horizontal, .vertical]) {
                NativeTextCanvasRepresentable(
                    snapshot: coordinator.activeEditorSession,
                    onInsertText: { text in
                        let start = DispatchTime.now().uptimeNanoseconds
                        coordinator.editorInsertText(text)
                        let elapsedMs = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000.0
                        lastMeasuredLatencyMs = elapsedMs
                    },
                    onInsertParagraphBreak: { coordinator.editorInsertParagraphBreak() },
                    onDeleteBackward: { coordinator.editorDeleteBackward() },
                    onDeleteForward: { coordinator.editorDeleteForward() },
                    onHitTest: { page, x, y in coordinator.editorHitTest(pageIndex: page, x: x, y: y) },
                    onToggleBold: { coordinator.editorToggleBold() },
                    onToggleItalic: { coordinator.editorToggleItalic() },
                    onUndo: { coordinator.editorUndo() },
                    onRedo: { coordinator.editorRedo() },
                    onSave: { coordinator.editorSavePdoc() }
                )
                .frame(
                    minWidth: 800,
                    minHeight: calculateCanvasHeight()
                )
            }
        }
        .sheet(isPresented: $showFootnotePrompt) {
            VStack(alignment: .leading, spacing: 14) {
                Text("Fußnote einfügen")
                    .font(.headline)
                TextField("Fundstelle / Erläuterung (z.B. BGH NJW 2021, 1234)", text: $footnoteInput)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 350)
                HStack {
                    Spacer()
                    Button("Abbrechen") {
                        showFootnotePrompt = false
                        footnoteInput = ""
                    }
                    Button("Einfügen") {
                        if !footnoteInput.isEmpty {
                            coordinator.editorInsertFootnote(text: footnoteInput)
                            footnoteInput = ""
                        }
                        showFootnotePrompt = false
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            .padding(20)
        }
    }

    private func calculateCanvasHeight() -> CGFloat {
        guard let pages = coordinator.activeEditorSession?.pages, !pages.isEmpty else { return 900 }
        return pages.reduce(CGFloat(40.0)) { $0 + CGFloat($1.heightPt) + 40.0 }
    }
}

/// NSViewRepresentable wrapper connecting SwiftUI to VocumentNativeTextCanvasView.
struct NativeTextCanvasRepresentable: NSViewRepresentable {
    let snapshot: FfiEditorSessionSnapshot?
    let onInsertText: (String) -> Void
    let onInsertParagraphBreak: () -> Void
    let onDeleteBackward: () -> Void
    let onDeleteForward: () -> Void
    let onHitTest: (UInt32, Float, Float) -> Void
    let onToggleBold: () -> Void
    let onToggleItalic: () -> Void
    let onUndo: () -> Void
    let onRedo: () -> Void
    let onSave: () -> Void

    func makeNSView(context: Context) -> VocumentNativeTextCanvasView {
        let view = VocumentNativeTextCanvasView(frame: .zero)
        updateCallbacks(view: view)
        return view
    }

    func updateNSView(_ nsView: VocumentNativeTextCanvasView, context: Context) {
        updateCallbacks(view: nsView)
        nsView.snapshot = snapshot
    }

    private func updateCallbacks(view: VocumentNativeTextCanvasView) {
        view.onInsertText = onInsertText
        view.onInsertParagraphBreak = onInsertParagraphBreak
        view.onDeleteBackward = onDeleteBackward
        view.onDeleteForward = onDeleteForward
        view.onHitTest = onHitTest
        view.onToggleBold = onToggleBold
        view.onToggleItalic = onToggleItalic
        view.onUndo = onUndo
        view.onRedo = onRedo
        view.onSave = onSave
    }
}
