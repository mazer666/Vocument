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

    /// Executes the comparison between activeSession and compareSecondSession asynchronously without blocking the UI.
    public func runComparison() {
        guard let sessionA = activeSession, let sessionB = compareSecondSession else {
            return
        }

        self.isComparing = true
        self.errorMessage = nil

        let sessionIdA = sessionA.sessionId
        let sessionIdB = sessionB.sessionId
        let client = coreClient

        Task.detached(priority: .userInitiated) { [weak self] in
            let options = FfiComparisonOptions(
                granularity: .word,
                ignoreWhitespace: true,
                ignoreCase: false,
                visualPixelThreshold: 16,
                visualClusterDistancePt: 10.0,
                alignClauses: true
            )

            do {
                let report = try client.compareDocuments(
                    sessionIdA: sessionIdA,
                    sessionIdB: sessionIdB,
                    options: options
                )

                await MainActor.run {
                    guard let self = self else { return }
                    self.compareReport = report
                    self.selectedChangeId = report.changes.first?.id
                    self.isComparing = false
                    self.errorMessage = nil
                    self.renderComparisonViewports()
                }
            } catch {
                await MainActor.run {
                    guard let self = self else { return }
                    self.isComparing = false
                    self.errorMessage = "Vergleichsfehler: \(error.localizedDescription)"
                }
            }
        }
    }

    /// Renders both pages at current comparePageA and comparePageB, and updates visual diff asynchronously.
    public func renderComparisonViewports() {
        guard let sessionA = activeSession, let sessionB = compareSecondSession else {
            return
        }

        self.isRenderingComparisonViewports = true
        let pageA = self.comparePageA
        let pageB = self.comparePageB
        let sessionIdA = sessionA.sessionId
        let sessionIdB = sessionB.sessionId
        let client = coreClient

        Task.detached(priority: .userInitiated) { [weak self] in
            var imageA: CGImage? = nil
            var imageB: CGImage? = nil
            var visualDiff: FfiPageDiffResult? = nil

            // Render page A
            if let geomA = try? client.pageGeometry(sessionId: sessionIdA, pageIndex: pageA) {
                let reqA = FfiTileRenderRequest(
                    pageIndex: pageA,
                    scaleMilli: 1000,
                    tileXPx: 0,
                    tileYPx: 0,
                    tileWidthPx: UInt32(max(1, geomA.widthMilliPt / 1000)),
                    tileHeightPx: UInt32(max(1, geomA.heightMilliPt / 1000))
                )
                if let resA = try? client.renderViewportWithPrefetch(sessionId: sessionIdA, request: reqA) {
                    imageA = DocumentCoreClient.makeCGImage(from: resA.visibleTile)
                }
            }

            // Render page B
            if let geomB = try? client.pageGeometry(sessionId: sessionIdB, pageIndex: pageB) {
                let reqB = FfiTileRenderRequest(
                    pageIndex: pageB,
                    scaleMilli: 1000,
                    tileXPx: 0,
                    tileYPx: 0,
                    tileWidthPx: UInt32(max(1, geomB.widthMilliPt / 1000)),
                    tileHeightPx: UInt32(max(1, geomB.heightMilliPt / 1000))
                )
                if let resB = try? client.renderViewportWithPrefetch(sessionId: sessionIdB, request: reqB) {
                    imageB = DocumentCoreClient.makeCGImage(from: resB.visibleTile)
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
            visualDiff = try? client.comparePagesVisual(
                sessionIdA: sessionIdA,
                pageA: pageA,
                sessionIdB: sessionIdB,
                pageB: pageB,
                options: options
            )

            await MainActor.run {
                guard let self = self else { return }
                self.compareRenderedImageA = imageA
                self.compareRenderedImageB = imageB
                self.compareVisualDiff = visualDiff
                self.isRenderingComparisonViewports = false
            }
        }
    }

    /// Navigates viewports to the location of a specific change item, respecting inserted/deleted pages.
    public func jumpToChange(item: FfiChangeItem) {
        self.selectedChangeId = item.id

        if let pa = item.pageA, let pb = item.pageB {
            self.comparePageA = pa
            self.comparePageB = pb
        } else if let pa = item.pageA {
            self.comparePageA = pa
            if compareSyncScroll, let report = compareReport {
                if let align = report.pageAlignments.first(where: { $0.pageA == pa }), let mappedB = align.pageB {
                    self.comparePageB = mappedB
                }
            }
        } else if let pb = item.pageB {
            self.comparePageB = pb
            if compareSyncScroll, let report = compareReport {
                if let align = report.pageAlignments.first(where: { $0.pageB == pb }), let mappedA = align.pageA {
                    self.comparePageA = mappedA
                }
            }
        }

        renderComparisonViewports()
    }

    /// Navigates Page A with automatic page alignment mapping for Page B when sync scroll is enabled.
    public func setComparePageAWithSync(_ targetA: UInt32) {
        self.comparePageA = targetA
        if compareSyncScroll, let report = compareReport {
            if let align = report.pageAlignments.first(where: { $0.pageA == targetA }), let mappedB = align.pageB {
                self.comparePageB = mappedB
            }
        }
        renderComparisonViewports()
    }

    /// Navigates Page B with automatic page alignment mapping for Page A when sync scroll is enabled.
    public func setComparePageBWithSync(_ targetB: UInt32) {
        self.comparePageB = targetB
        if compareSyncScroll, let report = compareReport {
            if let align = report.pageAlignments.first(where: { $0.pageB == targetB }), let mappedA = align.pageA {
                self.comparePageA = mappedA
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
