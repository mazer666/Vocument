import Foundation
import AppKit
import VocumentFFI
import VocumentMacKit

/// Extension on MacDocumentSessionCoordinator providing M3.7 Document Comparison logic (`COMPARE-001`–`004`).
extension MacDocumentSessionCoordinator {
    /// Opens an NSOpenPanel to select Document B for comparison.
    public func openSecondDocumentForComparisonViaPanel() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.pdf]
        panel.title = "Zweites Dokument für Vergleich auswählen (Dokument B)"
        panel.prompt = "Vergleichen"

        if panel.runModal() == .OK, let url = panel.url {
            openSecondDocumentForComparison(at: url)
        }
    }

    /// Opens Document B session and runs comparison against activeSession (Document A).
    public func openSecondDocumentForComparison(at url: URL) {
        guard activeSession != nil else {
            self.errorMessage = "Bitte öffnen Sie zuerst Dokument A."
            return
        }

        do {
            let sessionB = try coreClient.openScopedDocument(at: url)
            self.compareSecondSession = sessionB
            self.comparePageA = 0
            self.comparePageB = 0
            self.activeViewMode = .compare
            self.errorMessage = nil

            runComparison()
        } catch {
            self.errorMessage = "Fehler beim Öffnen von Dokument B: \(error.localizedDescription)"
        }
    }

    /// Executes the comparison between activeSession and compareSecondSession.
    public func runComparison() {
        guard let sessionA = activeSession, let sessionB = compareSecondSession else {
            return
        }

        do {
            let options = FfiComparisonOptions(
                granularity: .word,
                ignoreWhitespace: true,
                ignoreCase: false,
                visualPixelThreshold: 16,
                visualClusterDistancePt: 10.0,
                alignClauses: true
            )

            let report = try coreClient.compareDocuments(
                sessionIdA: sessionA.sessionId,
                sessionIdB: sessionB.sessionId,
                options: options
            )

            self.compareReport = report
            self.selectedChangeId = report.changes.first?.id
            self.errorMessage = nil

            renderComparisonViewports()
        } catch {
            self.errorMessage = "Vergleichsfehler: \(error.localizedDescription)"
        }
    }

    /// Renders both pages at current comparePageA and comparePageB, and updates visual diff.
    public func renderComparisonViewports() {
        guard let sessionA = activeSession, let sessionB = compareSecondSession else {
            return
        }

        // Render page A
        if let geomA = try? coreClient.pageGeometry(sessionId: sessionA.sessionId, pageIndex: comparePageA) {
            let reqA = FfiTileRenderRequest(
                pageIndex: comparePageA,
                scaleMilli: 1000,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: UInt32(max(1, geomA.widthMilliPt / 1000)),
                tileHeightPx: UInt32(max(1, geomA.heightMilliPt / 1000))
            )
            if let resA = try? coreClient.renderViewportWithPrefetch(sessionId: sessionA.sessionId, request: reqA) {
                self.compareRenderedImageA = DocumentCoreClient.makeCGImage(from: resA.visibleTile)
            }
        }

        // Render page B
        if let geomB = try? coreClient.pageGeometry(sessionId: sessionB.sessionId, pageIndex: comparePageB) {
            let reqB = FfiTileRenderRequest(
                pageIndex: comparePageB,
                scaleMilli: 1000,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: UInt32(max(1, geomB.widthMilliPt / 1000)),
                tileHeightPx: UInt32(max(1, geomB.heightMilliPt / 1000))
            )
            if let resB = try? coreClient.renderViewportWithPrefetch(sessionId: sessionB.sessionId, request: reqB) {
                self.compareRenderedImageB = DocumentCoreClient.makeCGImage(from: resB.visibleTile)
            }
        }

        // Calculate visual diff for current pair
        let options = FfiComparisonOptions(
            granularity: .word,
            ignoreWhitespace: true,
            ignoreCase: false,
            visualPixelThreshold: 16,
            visualClusterDistancePt: 10.0,
            alignClauses: true
        )
        self.compareVisualDiff = try? coreClient.comparePagesVisual(
            sessionIdA: sessionA.sessionId,
            pageA: comparePageA,
            sessionIdB: sessionB.sessionId,
            pageB: comparePageB,
            options: options
        )
    }

    /// Navigates viewports to the location of a specific change item.
    public func jumpToChange(item: FfiChangeItem) {
        self.selectedChangeId = item.id

        if let pa = item.pageA {
            self.comparePageA = pa
        }
        if let pb = item.pageB {
            self.comparePageB = pb
        } else if compareSyncScroll, let pa = item.pageA, let report = compareReport {
            // Find aligned page in B
            if let align = report.pageAlignments.first(where: { $0.pageA == pa }), let mappedB = align.pageB {
                self.comparePageB = mappedB
            }
        }

        renderComparisonViewports()
    }

    /// Exports the comparison report to disk (Markdown, HTML, or JSON).
    public func exportComparisonReportToFile(format: String) {
        guard let report = compareReport else { return }

        let panel = NSSavePanel()
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = "Vergleichsbericht.\(format.lowercased())"
        panel.title = "Vergleichsbericht exportieren"

        if panel.runModal() == .OK, let targetUrl = panel.url {
            let content: String
            switch format.lowercased() {
            case "html": content = report.htmlReport
            case "json": content = report.jsonReport
            default: content = report.markdownReport
            }

            do {
                try content.write(to: targetUrl, atomically: true, encoding: .utf8)
                self.saveNotificationMessage = "Bericht erfolgreich exportiert: \(targetUrl.lastPathComponent)"
            } catch {
                self.errorMessage = "Fehler beim Exportieren: \(error.localizedDescription)"
            }
        }
    }
}
