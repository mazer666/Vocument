import Foundation
import AppKit
import VocumentFFI
import VocumentMacKit

/// Extension on MacDocumentSessionCoordinator providing M4.3 native flow editor state management (`ADR-0020`, `UI-009`).
extension MacDocumentSessionCoordinator {
    /// Creates a new in-memory `.pdoc` flow document session for authoring.
    public func createNewAuthoringDocument(title: String = "Neuer Schriftsatz") {
        do {
            let session = try coreClient.createEditorSession(title: title)
            self.activeEditorSession = session
            self.activeViewMode = .authoring
            self.errorMessage = nil
        } catch {
            self.errorMessage = "Fehler beim Erstellen des Schriftsatzes: \(error.localizedDescription)"
        }
    }

    /// Opens an existing `.pdoc`, `.docx`, or `.odt` file package into the flow editor.
    public func openPdocDocument(at url: URL) {
        let ext = url.pathExtension.lowercased()
        if ext == "docx" || ext == "odt" || ext == "ott" {
            openOfficeDocument(at: url)
            return
        }
        do {
            let session = try coreClient.openPdocEditorSession(at: url.path)
            self.activeEditorSession = session
            self.activeViewMode = .authoring
            self.errorMessage = nil
        } catch {
            self.errorMessage = "Fehler beim Öffnen von .pdoc: \(error.localizedDescription)"
        }
    }

    /// Opens an OOXML `.docx` or ODF `.odt` file into the flow editor (`OFFICE-001`, `OFFICE-002`, `OFFICE-004`).
    public func openOfficeDocument(at url: URL) {
        do {
            let result = try coreClient.openOfficeEditorSession(at: url.path)
            self.activeEditorSession = result.snapshot
            self.activeViewMode = .authoring
            self.errorMessage = nil
            self.saveNotificationMessage = "\(url.lastPathComponent) (\(result.detectedFormat.uppercased())) importiert. \(result.report.summaryText)"
        } catch {
            self.errorMessage = "Fehler beim Office-Import: \(error.localizedDescription)"
        }
    }

    /// Displays an NSOpenPanel to choose and open a `.pdoc`, `.docx`, or `.odt` file.
    public func openPdocViaPanel() {
        let openPanel = NSOpenPanel()
        openPanel.canChooseFiles = true
        openPanel.canChooseDirectories = false
        openPanel.allowsMultipleSelection = false
        openPanel.allowedContentTypes = [.data]
        openPanel.begin { [weak self] response in
            guard let self, response == .OK, let selectedUrl = openPanel.url else { return }
            self.openPdocDocument(at: selectedUrl)
        }
    }

    /// Displays an NSOpenPanel specifically for importing `.docx` / `.odt` documents (`OFFICE-001`, `OFFICE-002`).
    public func openOfficeViaPanel() {
        let openPanel = NSOpenPanel()
        openPanel.canChooseFiles = true
        openPanel.canChooseDirectories = false
        openPanel.allowsMultipleSelection = false
        openPanel.allowedContentTypes = [.data]
        openPanel.begin { [weak self] response in
            guard let self, response == .OK, let selectedUrl = openPanel.url else { return }
            self.openOfficeDocument(at: selectedUrl)
        }
    }

    /// Closes the active editor session.
    public func closeEditorSession() {
        guard let session = activeEditorSession else { return }
        try? coreClient.closeEditorSession(editorId: session.editorId)
        self.activeEditorSession = nil
    }

    // MARK: - Interactive Editing Actions

    public func editorInsertText(_ text: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorInsertText(editorId: session.editorId, text: text)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorInsertParagraphBreak() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorInsertParagraphBreak(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorDeleteBackward() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorDeleteBackward(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorDeleteForward() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorDeleteForward(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorHitTest(pageIndex: UInt32, x: Float, y: Float) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorHitTest(editorId: session.editorId, pageIndex: pageIndex, xPt: x, yPt: y)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorToggleBold() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorToggleBold(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorToggleItalic() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorToggleItalic(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorSetHeadingLevel(_ level: UInt8) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorSetHeadingLevel(editorId: session.editorId, level: level)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorInsertBulletList() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorInsertBulletList(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorInsertNumberedList() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorInsertNumberedList(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorInsertTable(rows: UInt32, cols: UInt32) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorInsertTable(editorId: session.editorId, rows: rows, cols: cols)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorInsertFootnote(text: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorInsertFootnote(editorId: session.editorId, text: text)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorUndo() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorUndo(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorRedo() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorRedo(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorSavePdoc(to url: URL? = nil) {
        guard let session = activeEditorSession else { return }
        if let targetUrl = url {
            do {
                _ = try coreClient.editorSavePdoc(editorId: session.editorId, targetPath: targetUrl.path)
                self.saveNotificationMessage = "Schriftsatz erfolgreich gespeichert: \(targetUrl.lastPathComponent)"
            } catch {
                self.errorMessage = "Fehler beim Speichern: \(error.localizedDescription)"
            }
        } else if let existingPath = session.filePath {
            do {
                _ = try coreClient.editorSavePdoc(editorId: session.editorId, targetPath: existingPath)
                self.saveNotificationMessage = "Schriftsatz gespeichert."
            } catch {
                self.errorMessage = "Fehler beim Speichern: \(error.localizedDescription)"
            }
        } else {
            // Prompt Save Panel
            let savePanel = NSSavePanel()
            savePanel.allowedContentTypes = [.data]
            savePanel.nameFieldStringValue = "\(session.title).pdoc"
            savePanel.begin { [weak self] response in
                guard let self, response == .OK, let target = savePanel.url else { return }
                self.editorSavePdoc(to: target)
            }
        }
    }

    public func editorMeasureLatency() -> UInt64 {
        guard let session = activeEditorSession else { return 0 }
        return (try? coreClient.editorMeasureTypingLatencyMicros(
            editorId: session.editorId,
            sampleText: "Inkrementelle Layoutaktualisierung für juristischen Schriftsatz."
        )) ?? 0
    }

    // MARK: - Reviewing System & Tracked Changes Actions (`TRACK-001`–`006`)

    public func editorEnableTracking(author: String = "Anwalt") {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorEnableTracking(editorId: session.editorId, author: author)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorDisableTracking() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorDisableTracking(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorAddComment(anchorStart: UInt32?, anchorEnd: UInt32?, content: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorAddComment(
                editorId: session.editorId,
                anchorStart: anchorStart,
                anchorEnd: anchorEnd,
                content: content
            )
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorReplyComment(commentId: String, content: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorReplyComment(
                editorId: session.editorId,
                commentId: commentId,
                content: content
            )
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorResolveComment(commentId: String, resolved: Bool) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorResolveComment(
                editorId: session.editorId,
                commentId: commentId,
                resolved: resolved
            )
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorDeleteComment(commentId: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorDeleteComment(editorId: session.editorId, commentId: commentId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorAcceptChange(changeId: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorAcceptChange(editorId: session.editorId, changeId: changeId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorRejectChange(changeId: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorRejectChange(editorId: session.editorId, changeId: changeId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorAcceptAllChanges() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorAcceptAllChanges(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorRejectAllChanges() {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorRejectAllChanges(editorId: session.editorId)
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorExportPdf(to url: URL, includeComments: Bool = true, includeTrackedChanges: Bool = true) {
        guard let session = activeEditorSession else { return }
        do {
            let data = try coreClient.editorExportPdf(
                editorId: session.editorId,
                includeComments: includeComments,
                includeTrackedChanges: includeTrackedChanges
            )
            try data.write(to: url, options: .atomic)
            self.saveNotificationMessage = "PDF/A & PDF/UA erfolgreich exportiert: \(url.lastPathComponent)"
        } catch {
            self.errorMessage = "Fehler beim PDF-Export: \(error.localizedDescription)"
        }
    }

    public func editorReplaceRange(blockId: String, startOffset: UInt32, endOffset: UInt32, text: String) {
        guard let session = activeEditorSession else { return }
        do {
            let updated = try coreClient.editorReplaceRange(
                editorId: session.editorId,
                blockId: blockId,
                startOffset: startOffset,
                endOffset: endOffset,
                text: text
            )
            self.activeEditorSession = updated
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }

    public func editorExportDocx(to url: URL) {
        guard let session = activeEditorSession else { return }
        do {
            let report = try coreClient.editorExportDocx(editorId: session.editorId, targetPath: url.path)
            self.saveNotificationMessage = "DOCX exportiert (\(url.lastPathComponent)). \(report.summaryText)"
        } catch {
            self.errorMessage = "Fehler beim DOCX-Export: \(error.localizedDescription)"
        }
    }

    public func editorExportOdt(to url: URL) {
        guard let session = activeEditorSession else { return }
        do {
            let report = try coreClient.editorExportOdt(editorId: session.editorId, targetPath: url.path)
            self.saveNotificationMessage = "ODT exportiert (\(url.lastPathComponent)). \(report.summaryText)"
        } catch {
            self.errorMessage = "Fehler beim ODT-Export: \(error.localizedDescription)"
        }
    }
}

