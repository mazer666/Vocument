import SwiftUI
import AppKit
import Foundation
import VocumentMacKit
import VocumentFFI

/// Workflow orchestrations for M3.5 Optimization, Image Extraction, and PDF/X & Print Preflight.
@MainActor
extension MacDocumentSessionCoordinator {
    /// Optimizes the active document using the selected options (`M3.5`).
    func workbenchOptimizeDocument(options: FfiOptimizationOptions) {
        guard activeSession != nil else { return }
        requestWorkbenchAction(.optimize(options))
    }

    /// Discovers all embedded images across the document (`M3.5`).
    func workbenchGetDocumentImages() -> [FfiExtractedImageInfo] {
        guard let session = activeSession else { return [] }
        return (try? coreClient.getDocumentImages(sessionId: session.sessionId)) ?? []
    }

    /// Extracts raw PNG image bytes from a page (`M3.5`).
    func workbenchExtractImageBytes(pageIndex: UInt32, imageIndex: UInt32) -> FfiExtractedImageData? {
        guard let session = activeSession else { return nil }
        return try? coreClient.extractImageBytes(
            sessionId: session.sessionId,
            pageIndex: pageIndex,
            imageIndex: imageIndex
        )
    }

    /// Validates document against ISO 15930 PDF/X preflight rules (`M3.5`).
    func workbenchPreflightPdfx() -> FfiPdfxPreflightReport? {
        guard let session = activeSession else { return nil }
        return try? coreClient.preflightPdfx(sessionId: session.sessionId)
    }

    /// Preflights document dimensions and margin boxes for professional printing (`PRINT-003`, `PRINT-004`).
    func workbenchPreflightPrint() -> FfiPrintPreflightReport? {
        guard let session = activeSession else { return nil }
        return try? coreClient.preflightPrint(sessionId: session.sessionId)
    }
}
