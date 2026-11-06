using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Vocument.Windows.Rendering;
using Windows.Storage.Streams;

namespace Vocument.Windows.Views;

/// <summary>A page thumbnail rendered by the core.</summary>
public sealed record WorkbenchThumbnail(uint PageIndex, string Label, BitmapImage Thumbnail);

/// <summary>
/// WinUI 3 page workbench (M3.0-M3.6, PLATFORM-002).
/// <para>
/// Thumbnails, selection and rotation are executed by the shared Rust core. The
/// structural page operations (move, extract, delete, stamp) exist in the core but
/// are not exported by the Windows C ABI yet; the view asks the client for the
/// capability and reports the missing core entry point instead of pretending that
/// the operation succeeded (QA-003).
/// </para>
/// </summary>
public sealed partial class WorkbenchView : Microsoft.UI.Xaml.Controls.UserControl
{
    private const uint ThumbnailZoomMilli = 300;

    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsDocumentPicker _documentPicker;
    private readonly Func<IntPtr> _windowHandleProvider;

    private DocumentSessionSnapshot? _session;
    private readonly List<uint> _selectedPages = new();

    public ObservableCollection<WorkbenchThumbnail> Thumbnails { get; } = new();

    public WorkbenchView(
        DocumentCoreClient client,
        IWindowsFileBroker fileBroker,
        IWindowsDocumentPicker documentPicker,
        Func<IntPtr> windowHandleProvider)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _fileBroker = fileBroker ?? throw new ArgumentNullException(nameof(fileBroker));
        _documentPicker = documentPicker ?? throw new ArgumentNullException(nameof(documentPicker));
        _windowHandleProvider = windowHandleProvider ?? throw new ArgumentNullException(nameof(windowHandleProvider));

        InitializeComponent();
        PageThumbnailsList.ItemsSource = Thumbnails;
    }

    private async void OnOpenDocumentClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            var path = await _documentPicker.PickOpenDocumentAsync(_windowHandleProvider(), new[] { ".pdf" });
            if (path is null)
            {
                return;
            }

            if (!_fileBroker.VerifyAccessPermission(path, writeAccess: false))
            {
                WorkbenchStatusText.Text = $"Access to {path} is not permitted by the file broker.";
                return;
            }

            if (_session is not null)
            {
                _client.CloseSession(_session.SessionId);
            }

            _session = _client.OpenDocument(path);
            _selectedPages.Clear();
            WorkbenchStatusText.Text = $"{_session.Title ?? _session.DocumentId} — {_session.PageCount} pages";

            await BuildThumbnailsAsync();
        }
        catch (Exception ex)
        {
            WorkbenchStatusText.Text = ex.Message;
        }
    }

    private async void OnRefreshThumbnailsClicked(object sender, RoutedEventArgs e)
    {
        await BuildThumbnailsAsync();
    }

    private async void OnPageSelectionChanged(object sender, Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs e)
    {
        _selectedPages.Clear();
        foreach (var item in PageThumbnailsList.SelectedItems)
        {
            if (item is WorkbenchThumbnail thumbnail)
            {
                _selectedPages.Add(thumbnail.PageIndex);
            }
        }

        _selectedPages.Sort();
        WorkbenchStatusText.Text = _selectedPages.Count == 0
            ? "No page selected"
            : $"{_selectedPages.Count} page(s) selected: {string.Join(", ", _selectedPages.Select(page => page + 1))}";
    }

    private async void OnRotateSelectedLeftClicked(object sender, RoutedEventArgs e)
    {
        await RotateSelectedAsync(-90);
    }

    private async void OnRotateSelectedRightClicked(object sender, RoutedEventArgs e)
    {
        await RotateSelectedAsync(90);
    }

    private void OnMovePagesClicked(object sender, RoutedEventArgs e) => ReportUnavailable(CoreOperation.PageReorder);

    private void OnExtractPagesClicked(object sender, RoutedEventArgs e) => ReportUnavailable(CoreOperation.PageExtract);

    private void OnDeletePagesClicked(object sender, RoutedEventArgs e) => ReportUnavailable(CoreOperation.PageDelete);

    private void OnStampClicked(object sender, RoutedEventArgs e) => ReportUnavailable(CoreOperation.PageStamp);

    private void OnUndoClicked(object sender, RoutedEventArgs e)
    {
        if (_session is null)
        {
            WorkbenchStatusText.Text = "No document open.";
            return;
        }

        try
        {
            WorkbenchStatusText.Text = _client.Undo(_session.SessionId) ? "Undo applied" : "Nothing to undo";
        }
        catch (Exception ex)
        {
            WorkbenchStatusText.Text = ex.Message;
        }
    }

    private void OnRedoClicked(object sender, RoutedEventArgs e)
    {
        if (_session is null)
        {
            WorkbenchStatusText.Text = "No document open.";
            return;
        }

        try
        {
            WorkbenchStatusText.Text = _client.Redo(_session.SessionId) ? "Redo applied" : "Nothing to redo";
        }
        catch (Exception ex)
        {
            WorkbenchStatusText.Text = ex.Message;
        }
    }

    private async Task RotateSelectedAsync(int deltaDegrees)
    {
        if (_session is null || _selectedPages.Count == 0)
        {
            WorkbenchStatusText.Text = "Select at least one page first.";
            return;
        }

        try
        {
            var rotation = _client.RotatePages(_session.SessionId, _selectedPages, deltaDegrees);
            WorkbenchStatusText.Text = $"Rotation of {rotation}° applied to {_selectedPages.Count} page(s)";
            await BuildThumbnailsAsync();
        }
        catch (Exception ex)
        {
            WorkbenchStatusText.Text = ex.Message;
        }
    }

    private async Task BuildThumbnailsAsync()
    {
        if (_session is null)
        {
            return;
        }

        Thumbnails.Clear();

        for (uint pageIndex = 0; pageIndex < _session.PageCount; pageIndex++)
        {
            try
            {
                var tile = _client.RenderViewport(_session.SessionId, pageIndex, ThumbnailZoomMilli);
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

                Thumbnails.Add(new WorkbenchThumbnail(pageIndex, $"Page {pageIndex + 1}", image));
            }
            catch (Exception ex)
            {
                WorkbenchStatusText.Text = $"Page {pageIndex + 1} could not be rendered: {ex.Message}";
                break;
            }
        }
    }

    /// <summary>
    /// Reports a core operation that the linked core build does not expose through the
    /// Windows C ABI. The shell never emulates the operation.
    /// </summary>
    private void ReportUnavailable(CoreOperation operation)
    {
        try
        {
            _client.EnsureOperationAvailable(operation);
            WorkbenchStatusText.Text = $"{operation} is available.";
        }
        catch (CoreCapabilityUnavailableException ex)
        {
            WorkbenchStatusText.Text = ex.Message;
        }
    }
}
