import Foundation
import VocumentFFI

/// The place the user marked, and the text that stands there (`REDACT-001`).
public struct RedactionSelection: Equatable {
    /// The page the place was marked on.
    public let pageIndex: UInt32
    /// The marked rectangle in the page space of the engine.
    public let rect: FfiPdfRectMilliPt
    /// The text that lies inside the rectangle, as the core reads it.
    public let text: String

    public init(pageIndex: UInt32, rect: FfiPdfRectMilliPt, text: String) {
        self.pageIndex = pageIndex
        self.rect = rect
        self.text = text
    }
}

/// The result of a removal: what was removed, and what could not be proven
/// (`REDACT-001`).
public struct RedactionOutcome {
    /// The words that lay under the marked place.
    public let removedWords: [String]
    /// The places of the document where the same text still stands, the places
    /// that could not be read, and the reasons why a check of the removal
    /// could not be carried out.
    ///
    /// An **empty** list means that the removal was verified in every
    /// respect. The shell has to show a list that is not empty.
    public let warnings: [String]

    public init(removedWords: [String], warnings: [String]) {
        self.removedWords = removedWords
        self.warnings = warnings
    }
}

/// The reasons why a removal was refused before the core was asked.
public enum RedactionRefusal: Error, Equatable, CustomStringConvertible {
    /// The marked place stands on another page than the one the user is
    /// looking at. A removal would take away a text the user never saw in the
    /// preview.
    case markedPlaceOnAnotherPage(markedPage: UInt32, currentPage: UInt32)

    public var description: String {
        switch self {
        case .markedPlaceOnAnotherPage(let markedPage, let currentPage):
            return
                "The marked place stands on the page \(markedPage), and the page "
                + "\(currentPage) is shown — mark the place again."
        }
    }
}

/// The redaction of the reviewer (`REDACT-001`, `REDACT-002`, `RISK-032`).
///
/// The chain of the user is: **mark a place** on the page — **see the text**
/// that stands there — **remove it for good** — and **see what could not be
/// removed**.
///
/// This type carries that chain, so that it can be proven by a test and not
/// only by a finger on the screen. The shell (the window of the application)
/// owns the pictures and the mouse; the kit owns the chain; the core owns the
/// document. Nobody guesses: the text of the marked place comes from the core,
/// and the report of the removal comes from the core as well.
///
/// The redaction cannot be undone. A user who sees only "the redaction
/// succeeded" would believe that the marked text is gone from the document
/// while it is still in it — therefore the warnings of the report are part of
/// the result, and the shell has to show them.
public struct RedactionReview {
    private let client: DocumentCoreClient

    public init(client: DocumentCoreClient) {
        self.client = client
    }

    /// Asks the core which text lies inside the marked rectangle
    /// (`REDACT-001`).
    ///
    /// The core decides — the shell does not guess. The text of the selection
    /// is shown to the user **before** the removal: a user who removes a place
    /// has to see what stands there.
    public func select(
        sessionId: String,
        pageIndex: UInt32,
        rect: FfiPdfRectMilliPt
    ) throws -> RedactionSelection {
        let selection = try client.selectPageText(
            sessionId: sessionId,
            pageIndex: pageIndex,
            selectionRect: rect
        )
        return RedactionSelection(pageIndex: pageIndex, rect: rect, text: selection.cleanedText)
    }

    /// Removes the content of the marked place for good and verifies the
    /// removal (`REDACT-001`).
    ///
    /// The marked place belongs to the page it was marked on: a removal on
    /// another page would take away a text the user never saw in the preview,
    /// and it is refused before the core is asked.
    ///
    /// A removal that could not be verified in every respect **throws**: the
    /// document stays as it was, and there is no list of warnings — nothing
    /// happened.
    public func redact(
        sessionId: String,
        selection: RedactionSelection,
        currentPageIndex: UInt32
    ) throws -> RedactionOutcome {
        if selection.pageIndex != currentPageIndex {
            throw RedactionRefusal.markedPlaceOnAnotherPage(
                markedPage: selection.pageIndex,
                currentPage: currentPageIndex
            )
        }

        let region = FfiRedactionRegion(
            x0MilliPt: selection.rect.x0MilliPt,
            y0MilliPt: selection.rect.y0MilliPt,
            x1MilliPt: selection.rect.x1MilliPt,
            y1MilliPt: selection.rect.y1MilliPt
        )
        let report = try client.redactPageRegions(
            sessionId: sessionId,
            pageIndex: selection.pageIndex,
            regions: [region]
        )
        return RedactionOutcome(
            removedWords: report.removedWords,
            warnings: DocumentCoreClient.redactionWarnings(report)
        )
    }
}
