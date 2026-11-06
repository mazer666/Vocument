using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Text;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Vocument.Windows.Comparison;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Vocument.Windows.Rendering;
using Windows.Storage.Streams;

namespace Vocument.Windows.Views;

/// <summary>A detected difference between the two compared documents.</summary>
public sealed record DocumentDifference(string Page, string Kind, string Detail);

/// <summary>
/// WinUI 3 comparison surface (M3.7, CR-0005, PLATFORM-002).
/// <para>
/// The comparison runs on data that the shared Rust core produced: both documents
/// are opened as core sessions, the text of every page is extracted by the core and
/// the page rasters are rendered by the MuPDF engine of the core. The shell compares
/// those results and renders the diff overlay.
/// </para>
/// </summary>
public sealed partial class CompareView : Microsoft.UI.Xaml.Controls.UserControl
{
    private const uint CompareZoomMilli = 700;

    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsDocumentPicker _documentPicker;
    private readonly Func<IntPtr> _windowHandleProvider;
    private readonly CompareWorkflow _compareWorkflow;

    private string? _documentAPath;
    private string? _documentBPath;
    private DocumentSessionSnapshot? _sessionA;
    private DocumentSessionSnapshot? _sessionB;

    public ObservableCollection<DocumentDifference> Differences { get; } = new();

    public CompareView(
        DocumentCoreClient client,
        IWindowsFileBroker fileBroker,
        IWindowsDocumentPicker documentPicker,
        Func<IntPtr> windowHandleProvider)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _fileBroker = fileBroker ?? throw new ArgumentNullException(nameof(fileBroker));
        _documentPicker = documentPicker ?? throw new ArgumentNullException(nameof(documentPicker));
        _windowHandleProvider = windowHandleProvider ?? throw new ArgumentNullException(nameof(windowHandleProvider));
        _compareWorkflow = new CompareWorkflow(_client);

        InitializeComponent();
        DifferencesListView.ItemsSource = Differences;
    }

    private async void OnSelectDocAClicked(object sender, RoutedEventArgs e)
    {
        await LoadDocumentAsync(isOriginal: true);
    }

    private async void OnSelectDocBClicked(object sender, RoutedEventArgs e)
    {
        await LoadDocumentAsync(isOriginal: false);
    }

    private async void OnRunCompareClicked(object sender, RoutedEventArgs e)
    {
        if (_sessionA is null || _sessionB is null)
        {
            CompareStatusSummary.Text = "Choose document A and document B first.";
            return;
        }

        try
        {
            Differences.Clear();

            var result = _compareWorkflow.Compare(_sessionA, _sessionB, CompareZoomMilli);

            foreach (var entry in result.Entries)
            {
                Differences.Add(new DocumentDifference(entry.Page, entry.Kind, entry.Detail));
            }

            if (result.OverlayRgba.Length > 0)
            {
                var originalRaster = _client.RenderViewport(_sessionA.SessionId, 0, CompareZoomMilli);
                await ShowRasterAsync(DocAImage, originalRaster.ArgbPixels, (int)originalRaster.WidthPx, (int)originalRaster.HeightPx);
                await ShowRasterAsync(DocBImage, result.OverlayRgba, result.Width, result.Height);
            }

            CompareStatusSummary.Text = Differences.Count == 0
                ? "No differences detected between the two documents."
                : $"{Differences.Count} difference(s) detected between the two documents.";
        }
        catch (Exception ex)
        {
            CompareStatusSummary.Text = ex.Message;
        }
    }

    private async void OnExportReportClicked(object sender, RoutedEventArgs e)
    {
        if (_sessionA is null || _sessionB is null)
        {
            CompareStatusSummary.Text = "Choose document A and document B first.";
            return;
        }

        try
        {
            var path = await _documentPicker.PickSaveDocumentAsync(_windowHandleProvider(), "comparison-report.html", new[] { ".html" });
            if (path is null)
            {
                return;
            }

            var report = new StringBuilder();
            report.AppendLine("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Vocument comparison report</title></head><body>");
            report.AppendLine($"<h1>Vocument comparison report</h1>");
            report.AppendLine($"<p>Original: {System.Net.WebUtility.HtmlEncode(_documentAPath ?? "")}</p>");
            report.AppendLine($"<p>Modified: {System.Net.WebUtility.HtmlEncode(_documentBPath ?? "")}</p>");
            report.AppendLine("<table border=\"1\" cellspacing=\"0\" cellpadding=\"4\"><tr><th>Page</th><th>Kind</th><th>Detail</th></tr>");

            foreach (var difference in Differences)
            {
                report.AppendLine(
                    $"<tr><td>{System.Net.WebUtility.HtmlEncode(difference.Page)}</td>" +
                    $"<td>{System.Net.WebUtility.HtmlEncode(difference.Kind)}</td>" +
                    $"<td>{System.Net.WebUtility.HtmlEncode(difference.Detail)}</td></tr>");
            }

            report.AppendLine("</table></body></html>");

            _fileBroker.WriteDocumentBytes(path, Encoding.UTF8.GetBytes(report.ToString()));
            CompareStatusSummary.Text = $"Comparison report exported: {path}";
        }
        catch (Exception ex)
        {
            CompareStatusSummary.Text = ex.Message;
        }
    }

    private async void OnExportDocumentBClicked(object sender, RoutedEventArgs e)
    {
        if (_sessionB is null)
        {
            CompareStatusSummary.Text = "Choose document B first.";
            return;
        }

        try
        {
            var path = await _documentPicker.PickSaveDocumentAsync(_windowHandleProvider(), "document-b.pdf", new[] { ".pdf" });
            if (path is null)
            {
                return;
            }

            var directory = System.IO.Path.GetDirectoryName(path) ?? ".";
            var target = _fileBroker.GenerateCollisionFreePath(directory, System.IO.Path.GetFileName(path));
            _client.SaveSession(_sessionB.SessionId, target);
            CompareStatusSummary.Text = $"Document B exported through the core save pipeline: {target}";
        }
        catch (Exception ex)
        {
            CompareStatusSummary.Text = ex.Message;
        }
    }

    private async Task LoadDocumentAsync(bool isOriginal)
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
                CompareStatusSummary.Text = $"Access to {path} is not permitted by the file broker.";
                return;
            }

            if (isOriginal)
            {
                if (_sessionA is not null) _client.CloseSession(_sessionA.SessionId);
                _sessionA = _client.OpenDocument(path);
                _documentAPath = path;
                DocAPathText.Text = path;
            }
            else
            {
                if (_sessionB is not null) _client.CloseSession(_sessionB.SessionId);
                _sessionB = _client.OpenDocument(path);
                _documentBPath = path;
                DocBPathText.Text = path;
            }

            CompareStatusSummary.Text = isOriginal
                ? $"Original loaded: {path}"
                : $"Modified loaded: {path}";
        }
        catch (Exception ex)
        {
            CompareStatusSummary.Text = ex.Message;
        }
    }

    private static async Task ShowRasterAsync(Microsoft.UI.Xaml.Controls.Image target, byte[] rgba, int width, int height)
    {
        var bitmapBytes = RasterBitmapEncoder.EncodeBmp(rgba, width, height);

        using var stream = new InMemoryRandomAccessStream();
        using (var writer = new DataWriter(stream.GetOutputStreamAt(0)))
        {
            writer.WriteBytes(bitmapBytes);
            await writer.StoreAsync();
        }

        stream.Seek(0);
        var image = new BitmapImage();
        await image.SetSourceAsync(stream);
        target.Source = image;
    }
}
