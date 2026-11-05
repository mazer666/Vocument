using System;
using System.IO;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Xunit;

namespace Vocument.Windows.Tests;

public class WindowsAcceptanceScenarioTests
{
    [Fact]
    public void TestSharedAcceptanceScenario_Windows()
    {
        using var client = new DocumentCoreClient();

        // 1. Handshake verification (NET-001, LOG-001, PERF-001, I18N-002, OCR-002)
        var handshake = client.PerformHandshake();

        Assert.Equal("0.1", handshake.BaselineVersion);
        Assert.Equal("M3", handshake.Milestone);
        Assert.True(handshake.StrictOfflineDefault, "Expected strictOfflineDefault == true (NET-001)");
        Assert.False(handshake.TelemetryEnabled, "Expected telemetryEnabled == false (LOG-001)");
        Assert.Equal(60u, handshake.TargetFps);
        Assert.Equal("de-AT", handshake.DefaultGermanVariant);
        Assert.Equal(new[] { "deu", "eng" }, handshake.BuiltInOcrLanguages);

        // 2. Open real PDF document via Windows File Broker (M0.1)
        var repoRoot = FindRepoRoot();
        var samplePdfPath = Path.Combine(repoRoot, "tests", "corpus", "m0_3_navigation_and_search.pdf");
        Assert.True(File.Exists(samplePdfPath), $"Sample PDF not found at {samplePdfPath}");

        var session = client.OpenDocument(samplePdfPath);
        Assert.StartsWith("sess_", session.SessionId);
        Assert.StartsWith("doc_", session.DocumentId);
        Assert.Equal(4u, session.PageCount);
        Assert.Equal("Vocument M0.3 Navigation and Search Fixture", session.Title);
        Assert.Equal("Vocument Engineering", session.Author);
        Assert.Contains("mupdf", session.EngineIdentifier);

        // 3. Page Geometry and Viewport Tile Render with real MuPDF rasterizer (M0.2)
        var geo = client.GetPageGeometry(session.SessionId, 0);
        Assert.Equal(595_000u, geo.WidthMilliPt);
        Assert.Equal(842_000u, geo.HeightMilliPt);

        var tile = client.RenderViewport(session.SessionId, 0, 1000);
        Assert.Equal(595u, tile.WidthPx);
        Assert.Equal(842u, tile.HeightPx);
        Assert.NotEmpty(tile.ArgbPixels);
        Assert.Equal((int)(595 * 842 * 4), tile.ArgbPixels.Length);

        // 4. Exact SQLite FTS5 / document text search (M0.3)
        var searchHits = client.SearchExact(session.SessionId, "Vocument");
        Assert.NotEmpty(searchHits);
        Assert.Equal(0u, searchHits[0].PageIndex);
        Assert.Contains("Vocument", searchHits[0].Snippet);

        // 5. Native Text Selection from MuPDF text layout (OCRCOPY-001)
        var selection = client.SelectPageText(session.SessionId, 0);
        Assert.Contains("Vocument", selection.RawText);

        // 6. Highlight Annotation recorded via TransactionManager (M0.4)
        var highlight = client.AddHighlight(session.SessionId, 0, "#FFFF00");
        Assert.StartsWith("annot_win_", highlight.AnnotationId);
        Assert.Equal(0u, highlight.PageIndex);

        // 7. Rotate Pages via TransactionManager (PAGE-001)
        var rotation = client.RotatePages(session.SessionId, new[] { 0u }, 90);
        Assert.Equal(90u, rotation);

        // 8. Real Transaction Undo (M0.4)
        var undoSuccess = client.Undo(session.SessionId);
        Assert.True(undoSuccess, "Expected transaction undo to succeed");

        // 9. Real Transaction Redo (M0.4)
        var redoSuccess = client.Redo(session.SessionId);
        Assert.True(redoSuccess, "Expected transaction redo to succeed");

        // 10. Safe Save Session via MuPDF engine (M0.5, M0.6)
        var tempSavePath = Path.Combine(Path.GetTempPath(), $"vocument_win_test_{Guid.NewGuid():N}.pdf");
        try
        {
            client.SaveSession(session.SessionId, tempSavePath);
            Assert.True(File.Exists(tempSavePath), $"Saved file does not exist at {tempSavePath}");

            var savedBytes = File.ReadAllBytes(tempSavePath);
            Assert.True(savedBytes.Length > 500, $"Saved PDF must be real and valid, got length {savedBytes.Length}");
            Assert.Equal((byte)'%', savedBytes[0]);
            Assert.Equal((byte)'P', savedBytes[1]);
            Assert.Equal((byte)'D', savedBytes[2]);
            Assert.Equal((byte)'F', savedBytes[3]);

            // 11. Recovery Checkpoint Verification (M0.6)
            var hasRecovery = client.CheckRecoveryCheckpoint(session.SessionId);
            Assert.True(hasRecovery);

            // 12. Reopen Saved Document through MuPDF
            var reopened = client.OpenDocument(tempSavePath);
            Assert.NotNull(reopened);
            Assert.StartsWith("sess_", reopened.SessionId);
            Assert.Equal(4u, reopened.PageCount);
            client.CloseSession(reopened.SessionId);
        }
        finally
        {
            if (File.Exists(tempSavePath))
            {
                File.Delete(tempSavePath);
            }
        }
    }

    private static string FindRepoRoot()
    {
        var current = AppContext.BaseDirectory;
        while (!string.IsNullOrEmpty(current))
        {
            if (File.Exists(Path.Combine(current, "Cargo.toml")) && Directory.Exists(Path.Combine(current, "tests", "corpus")))
            {
                return current;
            }
            current = Path.GetDirectoryName(current);
        }
        return Directory.GetCurrentDirectory();
    }
}
