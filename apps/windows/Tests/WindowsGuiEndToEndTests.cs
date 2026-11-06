using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Text.RegularExpressions;
using Vocument.Windows.Comparison;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Vocument.Windows.Views;
using Xunit;

namespace Vocument.Windows.Tests;

/// <summary>
/// End-to-end tests of the user actions of the WinUI 3 shell.
/// <para>
/// The tests drive the same operations that the views of the shell perform, against
/// the real Rust core and the real MuPDF engine: opening, navigating, zooming,
/// selecting text, highlighting, rotating, undo, redo, saving, reopening and
/// comparing. They therefore run on every host that can load the native core
/// (Windows, macOS and Linux), not only on Windows (M0.5, M0.7, QA-003).
/// </para>
/// </summary>
public class WindowsGuiEndToEndTests
{
    [Fact]
    public void ViewerActions_OpenNavigateZoomSelectHighlightRotateUndoRedoSaveAndReopen()
    {
        using var client = new DocumentCoreClient();
        var workflow = new ViewerWorkflow(client, new WindowsFileBroker());
        var corpus = Path.Combine(FindRepoRoot(), "tests", "corpus", "m0_3_navigation_and_search.pdf");

        Assert.True(File.Exists(corpus), $"The corpus document is missing at {corpus}");

        var session = workflow.OpenDocument(corpus);
        Assert.Equal(4u, session.PageCount);
        Assert.Equal("Vocument M0.3 Navigation and Search Fixture", session.Title);

        // Navigation between the pages of the document.
        workflow.NavigateNext();
        Assert.Equal(1u, workflow.CurrentPageIndex);
        workflow.NavigatePrevious();
        Assert.Equal(0u, workflow.CurrentPageIndex);

        // Zooming re-renders the page with the engine of the core.
        var atFullSize = workflow.RenderCurrentPage();
        workflow.ZoomIn();
        var zoomedIn = workflow.RenderCurrentPage();
        Assert.True(zoomedIn.WidthPx > atFullSize.WidthPx, "The zoomed page must be wider than the page at full size.");
        workflow.ZoomOut();

        // Text selection and exact search run over the document index of the core.
        var selection = workflow.SelectCurrentPageText();
        Assert.Contains("Vocument", selection.RawText);

        var hits = workflow.Search("Vocument");
        Assert.NotEmpty(hits);

        // Highlighting adds a real annotation through the transaction manager.
        var highlight = workflow.HighlightCurrentPage();
        Assert.StartsWith("annot_win_", highlight.AnnotationId);
        Assert.Equal(0u, highlight.PageIndex);

        // Rotation, undo and redo are real transactions.
        Assert.Equal(90u, workflow.RotateCurrentPage(90));
        Assert.True(workflow.Undo(), "The rotation must be undoable.");
        Assert.True(workflow.Redo(), "The undone rotation must be redoable.");

        Assert.True(workflow.HasRecoveryCheckpoint());

        // Saving writes the document through the save pipeline of the core.
        var savedPath = Path.Combine(Path.GetTempPath(), $"vocument_gui_e2e_{Guid.NewGuid():N}.pdf");
        try
        {
            workflow.SaveTo(savedPath);
            Assert.True(File.Exists(savedPath), $"The saved document is missing at {savedPath}");

            var savedBytes = File.ReadAllBytes(savedPath);
            Assert.True(savedBytes.Length > 500, $"The saved document must be a real PDF, got {savedBytes.Length} bytes.");
            Assert.Equal("%PDF", Encoding.ASCII.GetString(savedBytes, 0, 4));

            // The state that is shown before the save must survive the round trip.
            var rotatedBeforeSave = workflow.RenderCurrentPage();

            var reopened = workflow.OpenDocument(savedPath);
            Assert.Equal(4u, reopened.PageCount);

            var rotatedAfterReopen = workflow.RenderCurrentPage();
            Assert.Equal(rotatedBeforeSave.WidthPx, rotatedAfterReopen.WidthPx);
            Assert.Equal(rotatedBeforeSave.HeightPx, rotatedAfterReopen.HeightPx);

            var roundTripDiff = PixelDiffEngine.Compare(
                rotatedBeforeSave.ArgbPixels,
                rotatedAfterReopen.ArgbPixels,
                (int)rotatedBeforeSave.WidthPx,
                (int)rotatedBeforeSave.HeightPx);

            Assert.True(
                roundTripDiff.ChangedRatio < 0.02,
                $"The reopened document must show the saved page, but {roundTripDiff.ChangedRatio:P2} of the pixels differ.");

            var reopenedText = workflow.SelectCurrentPageText();
            Assert.Contains("Vocument", reopenedText.RawText);
        }
        finally
        {
            if (File.Exists(savedPath))
            {
                File.Delete(savedPath);
            }
        }
    }

    [Fact]
    public void ViewerActions_RejectAnUnknownFilePath()
    {
        using var client = new DocumentCoreClient();
        var workflow = new ViewerWorkflow(client, new WindowsFileBroker());
        var missingPath = Path.Combine(Path.GetTempPath(), $"vocument_missing_{Guid.NewGuid():N}.pdf");

        Assert.Throws<FileNotFoundException>(() => workflow.OpenDocument(missingPath));
    }

    [Fact]
    public void ViewerActions_RejectAPathTheFileBrokerRefuses()
    {
        using var client = new DocumentCoreClient();
        var workflow = new ViewerWorkflow(client, new AccessDeniedFileBroker());
        var corpus = Path.Combine(FindRepoRoot(), "tests", "corpus", "m0_3_navigation_and_search.pdf");

        Assert.Throws<UnauthorizedAccessException>(() => workflow.OpenDocument(corpus));
    }

    [Fact]
    public void CompareActions_DetectTheDifferencesOfTwoRealDocuments()
    {
        using var client = new DocumentCoreClient();
        var workflow = new CompareWorkflow(client);
        var repositoryRoot = FindRepoRoot();

        var original = client.OpenDocument(Path.Combine(repositoryRoot, "tests", "corpus", "m0_3_navigation_and_search.pdf"));
        var modified = client.OpenDocument(Path.Combine(repositoryRoot, "tests", "corpus", "m0_1_sample_two_pages.pdf"));

        var result = workflow.Compare(original, modified);

        Assert.NotEmpty(result.Entries);
        Assert.True(result.RemovedWords + result.InsertedWords > 0, "Two different documents must show text differences.");
        Assert.Contains(result.Entries, entry => entry.Kind == "Text");

        client.CloseSession(original.SessionId);
        client.CloseSession(modified.SessionId);
    }

    [Fact]
    public void CompareActions_ReportNoDifferenceForTheSameDocument()
    {
        using var client = new DocumentCoreClient();
        var workflow = new CompareWorkflow(client);
        var corpus = Path.Combine(FindRepoRoot(), "tests", "corpus", "m0_3_navigation_and_search.pdf");

        var first = client.OpenDocument(corpus);
        var second = client.OpenDocument(corpus);

        var result = workflow.Compare(first, second);

        Assert.Empty(result.Entries);
        Assert.Equal(0, result.RemovedWords);
        Assert.Equal(0, result.InsertedWords);
        Assert.Equal(0, result.ChangedPixels);

        client.CloseSession(first.SessionId);
        client.CloseSession(second.SessionId);
    }

#if WINUI_DESKTOP
    [Fact]
    public void WinUiViews_DeclareEveryEventHandlerOfTheirXaml()
    {
        var windowsRoot = FindWindowsProjectRoot();
        var views = new (string File, Type Type)[]
        {
            ("Views/PdfViewerView.xaml", typeof(Vocument.Windows.Views.PdfViewerView)),
            ("Views/WorkbenchView.xaml", typeof(Vocument.Windows.Views.WorkbenchView)),
            ("Views/EditorView.xaml", typeof(Vocument.Windows.Views.EditorView)),
            ("Views/CompareView.xaml", typeof(Vocument.Windows.Views.CompareView))
        };

        var handlerPattern = new Regex("(?:Click|Checked|KeyDown|SelectionChanged|Toggled)=\"([A-Za-z0-9_]+)\"");

        foreach (var (file, type) in views)
        {
            var xaml = File.ReadAllText(Path.Combine(windowsRoot, file));
            var handlers = handlerPattern.Matches(xaml)
                .Cast<Match>()
                .Select(match => match.Groups[1].Value)
                .Distinct()
                .ToList();

            Assert.NotEmpty(handlers);

            foreach (var handler in handlers)
            {
                var method = type.GetMethod(handler, BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public);
                Assert.True(method is not null, $"{file} declares the handler '{handler}', but {type.Name} does not implement it.");

                var parameters = method!.GetParameters();
                Assert.Equal(2, parameters.Length);
                Assert.Equal(typeof(object), parameters[0].ParameterType);
                Assert.True(
                    typeof(Microsoft.UI.Xaml.RoutedEventArgs).IsAssignableFrom(parameters[1].ParameterType),
                    $"The handler '{handler}' must take a routed event argument.");
            }
        }
    }

    [Fact]
    public void WinUiShell_DerivesFromTheWindowsAppSdkBaseTypes()
    {
        Assert.True(typeof(Microsoft.UI.Xaml.Application).IsAssignableFrom(typeof(Vocument.Windows.App)));
        Assert.True(typeof(Microsoft.UI.Xaml.Window).IsAssignableFrom(typeof(Vocument.Windows.MainWindow)));
        Assert.True(typeof(Microsoft.UI.Xaml.Controls.UserControl).IsAssignableFrom(typeof(Vocument.Windows.Views.PdfViewerView)));
        Assert.True(typeof(Microsoft.UI.Xaml.Controls.UserControl).IsAssignableFrom(typeof(Vocument.Windows.Views.WorkbenchView)));
        Assert.True(typeof(Microsoft.UI.Xaml.Controls.UserControl).IsAssignableFrom(typeof(Vocument.Windows.Views.EditorView)));
        Assert.True(typeof(Microsoft.UI.Xaml.Controls.UserControl).IsAssignableFrom(typeof(Vocument.Windows.Views.CompareView)));
    }
#endif

    private static string FindRepoRoot()
    {
        var current = AppContext.BaseDirectory;
        while (!string.IsNullOrEmpty(current))
        {
            if (File.Exists(Path.Combine(current, "Cargo.toml")) &&
                Directory.Exists(Path.Combine(current, "tests", "corpus")))
            {
                return current;
            }

            current = Path.GetDirectoryName(current);
        }

        return Directory.GetCurrentDirectory();
    }

    private static string FindWindowsProjectRoot()
    {
        var current = AppContext.BaseDirectory;
        while (!string.IsNullOrEmpty(current))
        {
            var candidate = Path.Combine(current, "apps", "windows");
            if (Directory.Exists(Path.Combine(candidate, "Views")))
            {
                return candidate;
            }

            current = Path.GetDirectoryName(current);
        }

        throw new DirectoryNotFoundException("The Windows project root was not found.");
    }
}

/// <summary>
/// A file broker that refuses every access. It exists to verify the access check of
/// the shell; the document operations themselves always run against the real core.
/// </summary>
internal sealed class AccessDeniedFileBroker : IWindowsFileBroker
{
    public string GenerateCollisionFreePath(string destinationDirectory, string desiredFileName)
        => Path.Combine(destinationDirectory, desiredFileName);

    public bool VerifyAccessPermission(string filePath, bool writeAccess) => false;

    public byte[] ReadDocumentBytes(string filePath) => throw new UnauthorizedAccessException("Access denied.");

    public void WriteDocumentBytes(string filePath, byte[] data) => throw new UnauthorizedAccessException("Access denied.");
}
