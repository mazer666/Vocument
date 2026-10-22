import Foundation
import Testing
import VocumentFFI
@testable import VocumentMacKit

/// The proof of the chain of the redaction (`REDACT-001`, `RISK-032`).
///
/// The risk register named the gap: *"the step of the interface — the user
/// marks the region and sees the report — is NOT RUN"*. The chain is now in
/// the kit, and this test walks it the way the interface walks it:
///
/// 1. the user marks a place on the page,
/// 2. the interface shows the text that stands there,
/// 3. the user removes it for good,
/// 4. the report says what was removed and what could not be removed,
/// 5. the text is **gone** — not only from the report, but from the page.
///
/// The fifth point is the one that matters: a page that still shows the text
/// that was just removed for good would be a lie.
@Suite("The redaction of the interface")
struct RedactionReviewTests {
    /// The sample of the corpus: a document with a text layer.
    private func sampleDocument() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/corpus/m0_3_navigation_and_search.pdf")
    }

    /// Opens the sample as a document of the core.
    ///
    /// The document is opened from a buffer: the redaction changes the
    /// document, and a test must not write into the corpus of the project.
    private func openSession(client: DocumentCoreClient) throws -> FfiDocumentSessionSnapshot {
        let bytes = try Data(contentsOf: sampleDocument())
        return try documentCoreOpenBrokeredFile(
            descriptor: FfiBrokeredFileDescriptor(
                sourceToken: "test:redaction-review",
                displayName: "m0_3_navigation_and_search.pdf",
                accessOrigin: .inMemoryBuffer,
                osFileId: nil,
                modifiedUnixMs: nil,
                bytes: bytes
            )
        )
    }

    /// A word of the document that stands **once**, with the page it stands on.
    ///
    /// Two things make a word a bad choice, and both are found here:
    ///
    /// * A word that stands somewhere else would leave a warning: the report
    ///   names the places where the same text still stands, and that warning
    ///   is right. The **case** matters as well — the core looks for the
    ///   word without regard to its case, so a "Preface" that stands once and
    ///   a "preface" that stands next to it are the same word.
    /// * A word of the **title** of the document stands in the metadata as
    ///   well. The core cleans the metadata before it checks — but a word of
    ///   the title is no proof of a quiet removal.
    ///
    /// The test wants the quiet case: the text is gone, and nothing else
    /// carries it.
    private func uniqueWord(
        client: DocumentCoreClient,
        sessionId: String
    ) throws -> (word: FfiPageWord, pageIndex: UInt32) {
        let descriptors = try client.pageDescriptors(sessionId: sessionId)
        let summary = try client.sessionSnapshot(sessionId: sessionId)
        let titleWords = Set(
            (summary.pdfSummary.title ?? "")
                .lowercased()
                .split(whereSeparator: { !$0.isLetter && !$0.isNumber })
                .map(String.init)
        )

        var wordsByPage: [UInt32: [FfiPageWord]] = [:]
        var occurrences: [String: Int] = [:]
        for pageIndex in 0..<descriptors.count {
            let words = try client.pageWords(sessionId: sessionId, pageIndex: UInt32(pageIndex))
            wordsByPage[UInt32(pageIndex)] = words
            for word in words {
                occurrences[word.text.lowercased(), default: 0] += 1
            }
        }
        for pageIndex in wordsByPage.keys.sorted() {
            for word in wordsByPage[pageIndex] ?? [] {
                let lowered = word.text.lowercased()
                guard occurrences[lowered] == 1, !titleWords.contains(lowered) else { continue }
                return (word, pageIndex)
            }
        }
        throw RedactionReviewTestsError.noUniqueWord
    }

    @Test("The marked place shows the text that stands there")
    func theMarkedPlaceShowsTheTextThatStandsThere() throws {
        let client = DocumentCoreClient()
        let session = try openSession(client: client)
        defer { try? client.closeSession(sessionId: session.sessionId) }

        let review = RedactionReview(client: client)
        let found = try uniqueWord(client: client, sessionId: session.sessionId)

        let selection = try review.select(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex,
            rect: found.word.rect
        )

        #expect(selection.pageIndex == found.pageIndex)
        #expect(selection.text.contains(found.word.text))
        // The place belongs to the page it was marked on.
        #expect(selection.rect.x0MilliPt == found.word.rect.x0MilliPt)
        #expect(selection.rect.y1MilliPt == found.word.rect.y1MilliPt)
    }

    @Test("The removal takes the text away and the report says what it removed")
    func theRemovalTakesTheTextAwayAndTheReportSaysWhatItRemoved() throws {
        let client = DocumentCoreClient()
        let session = try openSession(client: client)
        defer { try? client.closeSession(sessionId: session.sessionId) }

        let review = RedactionReview(client: client)
        let found = try uniqueWord(client: client, sessionId: session.sessionId)
        let selection = try review.select(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex,
            rect: found.word.rect
        )

        let outcome = try review.redact(
            sessionId: session.sessionId,
            selection: selection,
            currentPageIndex: found.pageIndex
        )

        #expect(outcome.removedWords.contains(found.word.text))
        // The removal was verified in every respect: the report carries no
        // place where the text still stands and no reason why a check could
        // not be carried out.
        #expect(outcome.warnings.isEmpty)
    }

    @Test("The text is gone from the page after the removal")
    func theTextIsGoneFromThePageAfterTheRemoval() throws {
        let client = DocumentCoreClient()
        let session = try openSession(client: client)
        defer { try? client.closeSession(sessionId: session.sessionId) }

        let review = RedactionReview(client: client)
        let found = try uniqueWord(client: client, sessionId: session.sessionId)
        let selection = try review.select(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex,
            rect: found.word.rect
        )
        _ = try review.redact(
            sessionId: session.sessionId,
            selection: selection,
            currentPageIndex: found.pageIndex
        )

        // The word is not in the text of the page any more. The report said
        // that it was removed — the page has to agree with the report.
        let wordsAfter = try client.pageWords(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex
        )
        #expect(!wordsAfter.contains { $0.text == found.word.text })

        // The place that was marked holds no text any more: the interface
        // shows the text of the marked place, and after the removal it shows
        // nothing.
        let textAfter = try client.selectPageText(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex,
            selectionRect: found.word.rect
        )
        #expect(!textAfter.cleanedText.contains(found.word.text))
    }

    @Test("A marked place of another page is refused before the core is asked")
    func aMarkedPlaceOfAnotherPageIsRefusedBeforeTheCoreIsAsked() throws {
        let client = DocumentCoreClient()
        let session = try openSession(client: client)
        defer { try? client.closeSession(sessionId: session.sessionId) }

        let review = RedactionReview(client: client)
        let found = try uniqueWord(client: client, sessionId: session.sessionId)
        let selection = try review.select(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex,
            rect: found.word.rect
        )
        let otherPage: UInt32 = found.pageIndex == 0 ? 1 : 0

        // The user turns to another page and removes the place of this one. A
        // removal would take away a text the user never saw in the preview.
        #expect(
            throws: RedactionRefusal.markedPlaceOnAnotherPage(
                markedPage: found.pageIndex,
                currentPage: otherPage
            )
        ) {
            try review.redact(
                sessionId: session.sessionId,
                selection: selection,
                currentPageIndex: otherPage
            )
        }

        // The document is as it was: nothing happened.
        let wordsAfter = try client.pageWords(
            sessionId: session.sessionId,
            pageIndex: found.pageIndex
        )
        #expect(wordsAfter.contains { $0.text == found.word.text })
    }
}

/// The reasons why the test cannot do its work.
enum RedactionReviewTestsError: Error {
    /// The corpus carries no word that stands exactly once.
    case noUniqueWord
}
