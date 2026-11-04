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
        // 1. Handshake verification (NET-001, LOG-001, PERF-001, I18N-002, OCR-002)
        var client = new DocumentCoreClient();
        var handshake = client.PerformHandshake();

        Assert.Equal("0.1", handshake.BaselineVersion);
        Assert.Equal("M3", handshake.Milestone);
        Assert.True(handshake.StrictOfflineDefault, "Expected strictOfflineDefault == true (NET-001)");
        Assert.False(handshake.TelemetryEnabled, "Expected telemetryEnabled == false (LOG-001)");
        Assert.Equal(60u, handshake.TargetFps);
        Assert.Equal("de-AT", handshake.DefaultGermanVariant);
        Assert.Equal(new[] { "deu", "eng" }, handshake.BuiltInOcrLanguages);

        // 2. Open document via Windows File Broker (M0.1)
        var repoRoot = FindRepoRoot();
        var samplePdfPath = Path.Combine(repoRoot, "tests", "corpus", "m0_1_sample_two_pages.pdf");
        Assert.True(File.Exists(samplePdfPath), $"Sample PDF not found at {samplePdfPath}");

        var session = client.OpenDocument(samplePdfPath);
        Assert.StartsWith("sess_", session.SessionId);
        Assert.Equal("doc_1a00192b75fb403b", session.DocumentId);
        Assert.Equal(2u, session.PageCount);
        Assert.Equal("Vocument M0.1 Sample Document", session.Title);
        Assert.Equal("Vocument Engineering", session.Author);
        Assert.Contains("mupdf", session.EngineIdentifier);

        // 3. Geometry and Viewport Tile Render (M0.2)
        var geo = client.GetPageGeometry(session.SessionId, 0);
        Assert.Equal(595_000u, geo.WidthMilliPt);
        Assert.Equal(842_000u, geo.HeightMilliPt);

        var tile = client.RenderViewport(session.SessionId, 0, 1000);
        Assert.Equal(595u, tile.WidthPx);
        Assert.Equal(842u, tile.HeightPx);
        Assert.NotEmpty(tile.ArgbPixels);

        // 4. Exact Search (M0.3)
        var searchHits = client.SearchExact(session.SessionId, "Vocument");
        Assert.NotEmpty(searchHits);
        Assert.Equal(0u, searchHits[0].PageIndex);
        Assert.Contains("Vocument", searchHits[0].Snippet);

        // 5. Text Selection (OCRCOPY-001)
        var selection = client.SelectPageText(session.SessionId, 0);
        Assert.Contains("trustworthy offline document workflows", selection.CleanedText);

        // 6. Highlight Annotation (M0.4)
        var highlight = client.AddHighlight(session.SessionId, 0, "#FFFF00");
        Assert.StartsWith("annot_", highlight.AnnotationId);
        Assert.Equal(0u, highlight.PageIndex);

        // 7. Rotate Pages (PAGE-001)
        var rotation = client.RotatePages(session.SessionId, new[] { 0u }, 90);
        Assert.Equal(90u, rotation);

        // 8. Undo (M0.4)
        var undoSuccess = client.Undo(session.SessionId);
        Assert.True(undoSuccess);

        // 9. Redo (M0.4)
        var redoSuccess = client.Redo(session.SessionId);
        Assert.True(redoSuccess);

        // 10. Save Session (M0.5, M0.6)
        var tempSavePath = Path.Combine(Path.GetTempPath(), $"vocument_win_test_{Guid.NewGuid():N}.pdf");
        try
        {
            client.SaveSession(session.SessionId, tempSavePath);
            Assert.True(File.Exists(tempSavePath));

            // 11. Recovery Checkpoint Verification (M0.6)
            var hasRecovery = client.CheckRecoveryCheckpoint(session.SessionId);
            Assert.True(hasRecovery);

            // 12. Reopen Saved Document
            var reopened = client.OpenDocument(tempSavePath);
            Assert.NotNull(reopened);
            Assert.StartsWith("sess_", reopened.SessionId);
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
