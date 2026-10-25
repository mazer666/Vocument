import SwiftUI
import AppKit
import VocumentFFI
import VocumentMacKit

/// Native macOS FlowDocument authoring canvas with editing toolbar, typography controls, and sub-16ms latency badge (`ADR-0020`, `UI-009`, `PERF-001`).
struct PdocAuthoringCanvasView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator
    @State private var showFootnotePrompt = false
    @State private var footnoteInput = ""
    @State private var showCommentPrompt = false
    @State private var commentInput = ""
    @State private var showReviewDrawer = false
    @State private var replyingCommentId: String? = nil
    @State private var replyInput = ""
    @State private var showExportSheet = false
    @State private var exportIncludeComments = true
    @State private var exportIncludeTrackedChanges = true
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

                Divider().frame(height: 18)

                // Track Changes Toggle (`TRACK-001`)
                let isTracking = coordinator.activeEditorSession?.isTracking ?? false
                Button(action: {
                    if isTracking {
                        coordinator.editorDisableTracking()
                    } else {
                        coordinator.editorEnableTracking(author: "Anwalt")
                    }
                }) {
                    Label(
                        isTracking ? "Aufzeichnung aktiv" : "Änderungen verfolgen",
                        systemImage: isTracking ? "record.circle.fill" : "record.circle"
                    )
                    .foregroundStyle(isTracking ? Color.red : Color.primary)
                }
                .help("Änderungsverfolgung umschalten")

                // Add Comment (`TRACK-005`)
                Button(action: { showCommentPrompt = true }) {
                    Image(systemName: "bubble.left.and.bubble.right")
                }
                .help("Randkommentar anheften")

                // Review Drawer Toggle
                let commentCount = coordinator.activeEditorSession?.comments.count ?? 0
                let changeCount = coordinator.activeEditorSession?.trackedChanges.count ?? 0
                Button(action: { showReviewDrawer.toggle() }) {
                    HStack(spacing: 4) {
                        Image(systemName: "sidebar.trailing")
                        if commentCount + changeCount > 0 {
                            Text("\(commentCount + changeCount)")
                                .font(.caption2)
                                .padding(.horizontal, 4)
                                .background(Color.accentColor.opacity(0.2))
                                .cornerRadius(4)
                        }
                    }
                }
                .help("Prüfungsleiste für Kommentare und Änderungen")

                // PDF/A & PDF/UA Export (`EXPORT-001`)
                Button(action: { showExportSheet = true }) {
                    Label("PDF/A & PDF/UA", systemImage: "arrow.down.doc")
                }
                .help("Barrierefreies PDF/A-2b und PDF/UA-1 exportieren")

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

            // Main content area: canvas and review sidebar
            HStack(spacing: 0) {
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
                        onSave: { coordinator.editorSavePdoc() },
                        onReplaceRange: { blockId, startOffset, endOffset, text in
                            let start = DispatchTime.now().uptimeNanoseconds
                            coordinator.editorReplaceRange(blockId: blockId, startOffset: startOffset, endOffset: endOffset, text: text)
                            let elapsedMs = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000.0
                            lastMeasuredLatencyMs = elapsedMs
                        }
                    )
                    .frame(
                        minWidth: 800,
                        minHeight: calculateCanvasHeight()
                    )
                }

                // Collapsible Review Side Panel
                if showReviewDrawer {
                    Divider()
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            Text("Prüfung & Überarbeitung")
                                .font(.headline)
                            Spacer()
                            Button(action: { showReviewDrawer = false }) {
                                Image(systemName: "xmark")
                            }
                            .buttonStyle(.plain)
                        }
                        .padding(.horizontal, 12)
                        .padding(.top, 12)

                        // Tracked Changes Section
                        if let changes = coordinator.activeEditorSession?.trackedChanges, !changes.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                HStack {
                                    Text("Änderungen (\(changes.count))")
                                        .font(.subheadline)
                                        .fontWeight(.semibold)
                                    Spacer()
                                    Button("Alle annehmen") { coordinator.editorAcceptAllChanges() }
                                        .font(.caption)
                                    Button("Alle ablehnen") { coordinator.editorRejectAllChanges() }
                                        .font(.caption)
                                }
                                ForEach(changes, id: \.changeId) { chg in
                                    HStack(alignment: .top, spacing: 6) {
                                        Image(systemName: chg.changeType == "insert" ? "plus.circle.fill" : "minus.circle.fill")
                                            .foregroundStyle(chg.changeType == "insert" ? Color.green : Color.red)
                                        VStack(alignment: .leading, spacing: 2) {
                                            Text("\(chg.author): \(chg.textSnippet ?? "")")
                                                .font(.caption)
                                                .lineLimit(2)
                                            HStack {
                                                Button("Annehmen") { coordinator.editorAcceptChange(changeId: chg.changeId) }
                                                    .font(.caption2)
                                                Button("Ablehnen") { coordinator.editorRejectChange(changeId: chg.changeId) }
                                                    .font(.caption2)
                                            }
                                        }
                                    }
                                    .padding(6)
                                    .background(Color.secondary.opacity(0.08))
                                    .cornerRadius(4)
                                }
                            }
                            .padding(.horizontal, 12)
                        }

                        // Comments Section
                        if let comments = coordinator.activeEditorSession?.comments, !comments.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Kommentare (\(comments.count))")
                                    .font(.subheadline)
                                    .fontWeight(.semibold)
                                ForEach(comments, id: \.commentId) { comm in
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(comm.author)
                                                .font(.caption)
                                                .fontWeight(.bold)
                                            Spacer()
                                            Button(action: {
                                                coordinator.editorResolveComment(commentId: comm.commentId, resolved: !comm.resolved)
                                            }) {
                                                Image(systemName: comm.resolved ? "checkmark.circle.fill" : "circle")
                                                    .foregroundStyle(comm.resolved ? Color.green : Color.secondary)
                                            }
                                            .buttonStyle(.plain)
                                            Button(action: {
                                                coordinator.editorDeleteComment(commentId: comm.commentId)
                                            }) {
                                                Image(systemName: "trash")
                                                    .foregroundStyle(.secondary)
                                            }
                                            .buttonStyle(.plain)
                                        }
                                        if let anchor = comm.anchorText {
                                            Text("„\(anchor)“")
                                                .font(.caption2)
                                                .italic()
                                                .foregroundStyle(.secondary)
                                        }
                                        Text(comm.content)
                                            .font(.caption)

                                        if !comm.replies.isEmpty {
                                            VStack(alignment: .leading, spacing: 3) {
                                                ForEach(comm.replies, id: \.replyId) { rep in
                                                    HStack(alignment: .top, spacing: 4) {
                                                        Image(systemName: "arrow.turn.down.right")
                                                            .font(.caption2)
                                                            .foregroundStyle(.secondary)
                                                        VStack(alignment: .leading, spacing: 1) {
                                                            Text(rep.author)
                                                                .font(.caption2)
                                                                .fontWeight(.semibold)
                                                            Text(rep.content)
                                                                .font(.caption2)
                                                        }
                                                    }
                                                    .padding(.leading, 8)
                                                }
                                            }
                                        }

                                        if replyingCommentId == comm.commentId {
                                            HStack(spacing: 4) {
                                                TextField("Antwort verfassen...", text: $replyInput)
                                                    .textFieldStyle(.roundedBorder)
                                                    .font(.caption)
                                                Button("Senden") {
                                                    if !replyInput.isEmpty {
                                                        coordinator.editorReplyComment(commentId: comm.commentId, content: replyInput)
                                                        replyInput = ""
                                                        replyingCommentId = nil
                                                    }
                                                }
                                                .font(.caption2)
                                                Button(action: {
                                                    replyingCommentId = nil
                                                    replyInput = ""
                                                }) {
                                                    Image(systemName: "xmark")
                                                }
                                                .buttonStyle(.plain)
                                            }
                                            .padding(.top, 4)
                                        } else {
                                            Button("Antworten") {
                                                replyingCommentId = comm.commentId
                                                replyInput = ""
                                            }
                                            .font(.caption2)
                                            .buttonStyle(.plain)
                                            .foregroundStyle(Color.accentColor)
                                        }
                                    }
                                    .padding(8)
                                    .background(comm.resolved ? Color.secondary.opacity(0.04) : Color.accentColor.opacity(0.08))
                                    .cornerRadius(6)
                                }
                            }
                            .padding(.horizontal, 12)
                        }

                        Spacer()
                    }
                    .frame(width: 280)
                    .background(Color(NSColor.controlBackgroundColor))
                }
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
        .sheet(isPresented: $showCommentPrompt) {
            VStack(alignment: .leading, spacing: 14) {
                Text("Kommentar verfassen")
                    .font(.headline)
                TextField("Kommentartext...", text: $commentInput)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 350)
                HStack {
                    Spacer()
                    Button("Abbrechen") {
                        showCommentPrompt = false
                        commentInput = ""
                    }
                    Button("Anheften") {
                        if !commentInput.isEmpty {
                            coordinator.editorAddComment(anchorStart: nil, anchorEnd: nil, content: commentInput)
                            commentInput = ""
                        }
                        showCommentPrompt = false
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            .padding(20)
        }
        .sheet(isPresented: $showExportSheet) {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Image(systemName: "checkmark.seal.fill")
                        .font(.title2)
                        .foregroundStyle(Color.accentColor)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("PDF/A-2b & PDF/UA-1 Export")
                            .font(.headline)
                        Text("Strukturierte, barrierefreie Langzeitarchivierung")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Divider()

                Toggle("Randkommentare als Annotationen einbinden", isOn: $exportIncludeComments)
                    .font(.subheadline)
                Toggle("Änderungsverfolgung visuell darstellen", isOn: $exportIncludeTrackedChanges)
                    .font(.subheadline)

                HStack {
                    Spacer()
                    Button("Abbrechen") {
                        showExportSheet = false
                    }
                    Button("Exportieren...") {
                        showExportSheet = false
                        promptPdfExport(
                            includeComments: exportIncludeComments,
                            includeTrackedChanges: exportIncludeTrackedChanges
                        )
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            .padding(20)
            .frame(width: 420)
        }
    }

    private func promptPdfExport(includeComments: Bool = true, includeTrackedChanges: Bool = true) {
        guard let session = coordinator.activeEditorSession else { return }
        let savePanel = NSSavePanel()
        savePanel.allowedContentTypes = [.pdf]
        savePanel.nameFieldStringValue = "\(session.title).pdf"
        savePanel.begin { response in
            guard response == .OK, let target = savePanel.url else { return }
            coordinator.editorExportPdf(
                to: target,
                includeComments: includeComments,
                includeTrackedChanges: includeTrackedChanges
            )
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
    let onReplaceRange: (String, UInt32, UInt32, String) -> Void

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
        view.onReplaceRange = onReplaceRange
    }
}
