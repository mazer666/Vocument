import SwiftUI
import AppKit
import Combine
import CoreGraphics
import UniformTypeIdentifiers
import VocumentMacKit
import VocumentFFI

/// Observable document session, rendering, navigation, and search view-model for macOS (`ARCHITECTURE.md` §§3–10, 17, `ROADMAP.md` M0.1–M0.3).
/// Uses `ObservableObject` so it compiles cleanly under both Xcode and standalone Command Line Tools.
@MainActor
final class MacDocumentSessionCoordinator: ObservableObject {
    private let coreClient = DocumentCoreClient()
    private var memoryPressureSource: DispatchSourceMemoryPressure?

    @Published var activeSession: FfiDocumentSessionSnapshot?
    @Published var currentPageIndex: UInt32 = 0
    @Published var scaleMilli: UInt32 = 1000
    @Published var renderedPageImage: CGImage?
    @Published var cacheStats: FfiTileCacheStats?
    @Published var prefetchedPages: [UInt32] = []
    @Published var pageDescriptors: [FfiPageDescriptor] = []
    @Published var thumbnailImages: [UInt32: CGImage] = [:]
    @Published var navigationInput: String = ""
    @Published var searchQuery: String = ""
    @Published var searchHits: [FfiSearchHit] = []
    @Published var searchProgress: FfiSearchIndexProgress?
    @Published var selectedTextPreview: String?
    @Published var errorMessage: String?

    init() {
        setupSystemMemoryPressureObserver()
    }

    var pageCount: UInt32 {
        activeSession?.pdfSummary.pageCount ?? 0
    }

    var currentLogicalLabel: String {
        pageDescriptors.first(where: { $0.pageIndex == currentPageIndex })?.logicalLabel
            ?? "\(currentPageIndex + 1)"
    }

    var prefetchedPagesSummary: String {
        prefetchedPages.isEmpty
            ? "none"
            : prefetchedPages.map { String($0 + 1) }.joined(separator: ", ")
    }

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
            let session = try coreClient.openScopedDocument(at: url)
            activeSession = session
            currentPageIndex = 0
            scaleMilli = 1000
            thumbnailImages = [:]
            searchHits = []
            errorMessage = nil
            pageDescriptors = try coreClient.pageDescriptors(sessionId: session.sessionId)
            renderCurrentViewport()
            loadThumbnailsAndInitialSearchBatch(sessionId: session.sessionId)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func canStepPage(by delta: Int) -> Bool {
        guard let session = activeSession else { return false }
        let target = Int(currentPageIndex) + delta
        return target >= 0 && target < Int(session.pdfSummary.pageCount)
    }

    func stepPage(by delta: Int) {
        guard canStepPage(by: delta) else { return }
        jumpToPageIndex(UInt32(Int(currentPageIndex) + delta))
    }

    func jumpToPageIndex(_ index: UInt32) {
        guard index < pageCount else { return }
        currentPageIndex = index
        renderCurrentViewport()
    }

    func jumpToQuery() {
        guard let session = activeSession else { return }
        do {
            let resolved = try coreClient.resolvePageNavigation(
                sessionId: session.sessionId,
                query: navigationInput
            )
            jumpToPageIndex(resolved.pageIndex)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func indexNextBatch() {
        guard let session = activeSession else { return }
        do {
            searchProgress = try coreClient.indexSearchBatch(
                sessionId: session.sessionId,
                preferredStartPage: currentPageIndex,
                maxPages: 2
            )
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func performSearch() {
        guard let session = activeSession else { return }
        do {
            let response = try coreClient.searchExact(
                sessionId: session.sessionId,
                query: searchQuery
            )
            searchHits = response.hits
            searchProgress = response.progress
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func copyCurrentPageText(cleaned: Bool) {
        guard let session = activeSession else { return }
        do {
            let sel = try coreClient.selectPageText(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                selectionRect: nil
            )
            let payload = cleaned ? sel.cleanedText : sel.rawText
            selectedTextPreview = payload
            let pb = NSPasteboard.general
            pb.clearContents()
            pb.setString(payload, forType: .string)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func adjustZoom(deltaMilli: Int) {
        let next = max(500, min(3000, Int(scaleMilli) + deltaMilli))
        guard UInt32(next) != scaleMilli else { return }
        scaleMilli = UInt32(next)
        renderCurrentViewport()
    }

    func applyMemoryPressure(_ level: FfiMemoryPressureLevel) {
        do {
            cacheStats = try coreClient.handleMemoryPressure(level: level)
            if level == .critical {
                thumbnailImages = [:]
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func renderCurrentViewport() {
        guard let session = activeSession, session.pdfSummary.pageCount > 0 else {
            renderedPageImage = nil
            return
        }
        do {
            let geometry = try coreClient.pageGeometry(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex
            )
            let scaledWidth = max(
                1,
                UInt32((UInt64(geometry.widthMilliPt) * UInt64(scaleMilli)) / 1_000_000)
            )
            let scaledHeight = max(
                1,
                UInt32((UInt64(geometry.heightMilliPt) * UInt64(scaleMilli)) / 1_000_000)
            )
            let request = FfiTileRenderRequest(
                pageIndex: currentPageIndex,
                scaleMilli: scaleMilli,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: scaledWidth,
                tileHeightPx: scaledHeight
            )
            let result = try coreClient.renderViewportWithPrefetch(
                sessionId: session.sessionId,
                request: request
            )
            renderedPageImage = DocumentCoreClient.makeCGImage(from: result.visibleTile)
            prefetchedPages = result.prefetchedPages
            cacheStats = result.cacheStats
            if let sel = try? coreClient.selectPageText(
                sessionId: session.sessionId,
                pageIndex: currentPageIndex,
                selectionRect: nil
            ) {
                selectedTextPreview = sel.cleanedText
            }
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func loadThumbnailsAndInitialSearchBatch(sessionId: String) {
        for desc in pageDescriptors {
            if let thumb = try? coreClient.renderPageThumbnail(
                sessionId: sessionId,
                pageIndex: desc.pageIndex,
                maxDimensionPx: 140
            ),
               let cgImg = DocumentCoreClient.makeCGImage(from: thumb) {
                thumbnailImages[desc.pageIndex] = cgImg
            }
        }
        searchProgress = try? coreClient.indexSearchBatch(
            sessionId: sessionId,
            preferredStartPage: currentPageIndex,
            maxPages: 2
        )
    }

    private func setupSystemMemoryPressureObserver() {
        let source = DispatchSource.makeMemoryPressureSource(
            eventMask: [.normal, .warning, .critical],
            queue: .main
        )
        source.setEventHandler { [weak self] in
            guard let self else { return }
            let event = source.data
            if event.contains(.critical) {
                self.applyMemoryPressure(.critical)
            } else if event.contains(.warning) {
                self.applyMemoryPressure(.warning)
            } else {
                self.applyMemoryPressure(.normal)
            }
        }
        source.resume()
        self.memoryPressureSource = source
    }
}

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
                Spacer()
                Button("Open PDF…") {
                    coordinator.openDocumentViaPanel()
                }
            }

            if let errorMessage = coordinator.errorMessage {
                Text(errorMessage)
                    .foregroundStyle(.red)
            }

            if coordinator.activeSession != nil {
                HStack(alignment: .top, spacing: 16) {
                    NavigationAndSearchSidebarView(coordinator: coordinator)
                    DocumentCanvasView(coordinator: coordinator)
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
