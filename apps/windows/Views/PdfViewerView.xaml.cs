using System;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Vocument.Windows.Rendering;
using Windows.Storage.Streams;

namespace Vocument.Windows.Views;

/// <summary>
/// WinUI 3 PDF work surface (M0.1-M0.7, PLATFORM-002).
/// <para>
/// Every command of this view is executed by the shared Rust core through
/// <see cref="ViewerWorkflow"/>: opening, rendering, searching, text selection,
/// highlighting, rotation, undo, redo, saving and the recovery checkpoint. The
/// workflow is the tested action layer of the surface, so the end-to-end tests of
/// the shell verify exactly the behaviour of this view (QA-003).
/// </para>
/// </summary>
public sealed partial class PdfViewerView : Microsoft.UI.Xaml.Controls.UserControl
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsDocumentPicker _documentPicker;
    private readonly Func<IntPtr> _windowHandleProvider;
    private readonly ViewerWorkflow _workflow;

    public PdfViewerView(
        DocumentCoreClient client,
        IWindowsFileBroker fileBroker,
        IWindowsDocumentPicker documentPicker,
        Func<IntPtr> windowHandleProvider)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _fileBroker = fileBroker ?? throw new ArgumentNullException(nameof(fileBroker));
        _documentPicker = documentPicker ?? throw new ArgumentNullException(nameof(documentPicker));
        _windowHandleProvider = windowHandleProvider ?? throw new ArgumentNullException(nameof(windowHandleProvider));
        _workflow = new ViewerWorkflow(_client, _fileBroker);

        InitializeComponent();
    }

    private async void OnOpenFileClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            var path = await _documentPicker.PickOpenDocumentAsync(_windowHandleProvider(), new[] { ".pdf" });
            if (path is null)
            {
                return;
            }

            await OpenDocumentAsync(path);
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async void OnSaveClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var session = _workflow.Session!;
            var suggested = session.Title is { Length: > 0 } title
                ? $"{title}.pdf"
                : "document.pdf";

            var path = await _documentPicker.PickSaveDocumentAsync(_windowHandleProvider(), suggested, new[] { ".pdf" });
            if (path is null)
            {
                return;
            }

            var directory = System.IO.Path.GetDirectoryName(path) ?? ".";
            var target = _fileBroker.GenerateCollisionFreePath(directory, System.IO.Path.GetFileName(path));

            _workflow.SaveTo(target);
            DocumentStatusText.Text = $"Saved: {target}";
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async void OnZoomInClicked(object sender, RoutedEventArgs e)
    {
        await ChangeZoomAsync(zoomIn: true);
    }

    private async void OnZoomOutClicked(object sender, RoutedEventArgs e)
    {
        await ChangeZoomAsync(zoomIn: false);
    }

    private async void OnRotateLeftClicked(object sender, RoutedEventArgs e)
    {
        await RotateAsync(-90);
    }

    private async void OnRotateRightClicked(object sender, RoutedEventArgs e)
    {
        await RotateAsync(90);
    }

    private async void OnHighlightClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var annotation = _workflow.HighlightCurrentPage();
            DocumentStatusText.Text = $"Highlight {annotation.AnnotationId} added on page {_workflow.CurrentPageIndex + 1}";
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async void OnExtractTextClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var selection = _workflow.SelectCurrentPageText();
            SelectedTextBox.Text = selection.CleanedText;
            DocumentStatusText.Text = $"Text of page {_workflow.CurrentPageIndex + 1} extracted ({selection.RawText.Length} raw characters)";
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async void OnSearchClicked(object sender, RoutedEventArgs e)
    {
        await RunSearchAsync();
    }

    private async void OnSearchKeyDown(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (e.Key != Windows.System.VirtualKey.Enter)
        {
            return;
        }

        e.Handled = true;
        await RunSearchAsync();
    }

    private async void OnPrevPageClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession() || _workflow.CurrentPageIndex == 0)
        {
            return;
        }

        _workflow.NavigatePrevious();
        await RenderCurrentPageAsync();
    }

    private async void OnNextPageClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession() || _workflow.CurrentPageIndex + 1 >= _workflow.PageCount)
        {
            return;
        }

        _workflow.NavigateNext();
        await RenderCurrentPageAsync();
    }

    private async void OnUndoClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var undone = _workflow.Undo();
            DocumentStatusText.Text = undone ? "Undo applied" : "Nothing to undo";
            await RenderCurrentPageAsync();
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async void OnRedoClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var redone = _workflow.Redo();
            DocumentStatusText.Text = redone ? "Redo applied" : "Nothing to redo";
            await RenderCurrentPageAsync();
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async void OnCheckRecoveryClicked(object sender, RoutedEventArgs e)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var hasRecovery = _workflow.HasRecoveryCheckpoint();
            DocumentStatusText.Text = hasRecovery
                ? "Recovery checkpoint available for this session"
                : "No recovery checkpoint for this session";
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async Task OpenDocumentAsync(string path)
    {
        var session = _workflow.OpenDocument(path);
        ZoomText.Text = "100%";
        PageIndicatorText.Text = $"Page 1 of {session.PageCount}";
        DocumentStatusText.Text = $"{session.Title ?? session.DocumentId} — {session.EngineIdentifier} — {session.PageCount} pages";

        await RenderCurrentPageAsync();
    }

    private async Task ChangeZoomAsync(bool zoomIn)
    {
        if (!EnsureSession())
        {
            return;
        }

        if (zoomIn)
        {
            _workflow.ZoomIn();
        }
        else
        {
            _workflow.ZoomOut();
        }

        ZoomText.Text = $"{_workflow.ZoomMilli / 10f:0}%";
        await RenderCurrentPageAsync();
    }

    private async Task RotateAsync(int deltaDegrees)
    {
        if (!EnsureSession())
        {
            return;
        }

        try
        {
            var rotation = _workflow.RotateCurrentPage(deltaDegrees);
            DocumentStatusText.Text = $"Page {_workflow.CurrentPageIndex + 1} rotated to {rotation}°";
            await RenderCurrentPageAsync();
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async Task RunSearchAsync()
    {
        if (!EnsureSession())
        {
            return;
        }

        var query = SearchBox.Text;
        if (string.IsNullOrWhiteSpace(query))
        {
            SearchResultsSummary.Text = "";
            return;
        }

        try
        {
            var hits = _workflow.Search(query);
            if (hits.Count == 0)
            {
                SearchResultsSummary.Text = $"No hit for '{query}'";
                return;
            }

            var first = hits[0];
            SearchResultsSummary.Text = $"{hits.Count} hit(s) for '{query}'. First: page {first.PageIndex + 1} ({first.LogicalPageLabel}) — {first.Snippet}";
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private async Task RenderCurrentPageAsync()
    {
        if (_workflow.Session is null)
        {
            return;
        }

        try
        {
            var tile = _workflow.RenderCurrentPage();
            var bitmapBytes = RasterBitmapEncoder.EncodeBmp(tile.ArgbPixels, (int)tile.WidthPx, (int)tile.HeightPx);

            using var stream = new InMemoryRandomAccessStream();
            using (var writer = new DataWriter(stream.GetOutputStreamAt(0)))
            {
                writer.WriteBytes(bitmapBytes);
                await writer.StoreAsync();
            }

            stream.Seek(0);
            var image = new BitmapImage();
            await image.SetSourceAsync(stream);
            PageViewportImage.Source = image;

            PageIndicatorText.Text = $"Page {_workflow.CurrentPageIndex + 1} of {_workflow.PageCount}";
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
    }

    private bool EnsureSession()
    {
        if (_workflow.Session is not null)
        {
            return true;
        }

        ShowError("No document is open. Use 'Open PDF...' first.");
        return false;
    }

    private void ShowError(string message)
    {
        DocumentStatusText.Text = message;
    }
}
