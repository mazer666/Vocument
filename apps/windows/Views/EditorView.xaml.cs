using System;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Documents;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;

namespace Vocument.Windows.Views;

/// <summary>
/// WinUI 3 flow document surface (M4.0-M4.5, DOC-001).
/// <para>
/// The document content is read from the core (text extraction of the opened
/// session) and the authoring commands operate on the WinUI rich text surface. The
/// tagged PDF export of the Rust layout engine is not exported by the Windows C ABI
/// yet; the view asks the client for the capability and names the missing core
/// entry point instead of writing a file that is not a PDF/UA document (QA-003).
/// </para>
/// </summary>
public sealed partial class EditorView : Microsoft.UI.Xaml.Controls.UserControl
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsDocumentPicker _documentPicker;
    private readonly Func<IntPtr> _windowHandleProvider;

    private DocumentSessionSnapshot? _session;
    private readonly List<string> _trackedChanges = new();
    private bool _trackChanges;
    private bool _isLoading;

    public EditorView(
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
    }

    private async void OnOpenDocumentClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            var path = await _documentPicker.PickOpenDocumentAsync(_windowHandleProvider(), new[] { ".pdf", ".pdoc" });
            if (path is null)
            {
                return;
            }

            if (!_fileBroker.VerifyAccessPermission(path, writeAccess: false))
            {
                EditorStatusText.Text = $"Access to {path} is not permitted by the file broker.";
                return;
            }

            if (path.EndsWith(".pdoc", StringComparison.OrdinalIgnoreCase))
            {
                _client.EnsureOperationAvailable(CoreOperation.FlowDocumentOpen);
                return;
            }

            if (_session is not null)
            {
                _client.CloseSession(_session.SessionId);
            }

            _session = _client.OpenDocument(path);
            _trackedChanges.Clear();
            _isLoading = true;
            FlowDocumentEditorBox.Document.SetText(CoreTextExtractor.ExtractDocumentText(_client, _session));
            _isLoading = false;

            EditorStatusText.Text = $"{_session.Title ?? _session.DocumentId} — {_session.PageCount} pages loaded from the core";
        }
        catch (CoreCapabilityUnavailableException ex)
        {
            EditorStatusText.Text = ex.Message;
        }
        catch (Exception ex)
        {
            EditorStatusText.Text = ex.Message;
        }
    }

    private async void OnSaveDocumentClicked(object sender, RoutedEventArgs e)
    {
        if (_session is null)
        {
            EditorStatusText.Text = "No document open.";
            return;
        }

        try
        {
            var path = await _documentPicker.PickSaveDocumentAsync(_windowHandleProvider(), "document.pdf", new[] { ".pdf" });
            if (path is null)
            {
                return;
            }

            var directory = System.IO.Path.GetDirectoryName(path) ?? ".";
            var target = _fileBroker.GenerateCollisionFreePath(directory, System.IO.Path.GetFileName(path));
            _client.SaveSession(_session.SessionId, target);
            EditorStatusText.Text = $"Saved: {target}";
        }
        catch (Exception ex)
        {
            EditorStatusText.Text = ex.Message;
        }
    }

    private void OnStyleSelectionChanged(object sender, Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs e)
    {
        if (FlowDocumentEditorBox.Document is not { } document)
        {
            return;
        }

        var selection = document.Selection;
        var start = selection.Start;
        var length = Math.Max(0, selection.End - start);
        var level = StyleComboBox.SelectedIndex switch
        {
            1 => 1,
            2 => 2,
            3 => 3,
            _ => 0
        };

        document.SetParagraphStyle(start, length, new ParagraphStyle
        {
            HeadingLevel = level,
            IsCodeBlock = StyleComboBox.SelectedIndex == 4
        });
    }

    private void OnBoldClicked(object sender, RoutedEventArgs e) => ApplyInlineStyle(new InlineStyle { Bold = true });

    private void OnItalicClicked(object sender, RoutedEventArgs e) => ApplyInlineStyle(new InlineStyle { Italic = true });

    private void OnUnderlineClicked(object sender, RoutedEventArgs e) => ApplyInlineStyle(new InlineStyle { Underline = true });

    private void OnInsertTableClicked(object sender, RoutedEventArgs e)
    {
        FlowDocumentEditorBox.Document.InsertTextAtCaret("| Column A | Column B |\n| --- | --- |\n| value | value |");
        EditorStatusText.Text = "Table skeleton inserted";
    }

    private void OnInsertFigureClicked(object sender, RoutedEventArgs e)
    {
        FlowDocumentEditorBox.Document.InsertTextAtCaret("[Figure: describe the image for the PDF/UA alt text]");
        EditorStatusText.Text = "Figure placeholder inserted (alt text required for PDF/UA)";
    }

    private void OnTrackChangesToggled(object sender, Microsoft.UI.Xaml.Controls.ToggledEventArgs e)
    {
        _trackChanges = e.IsOn;
        EditorStatusText.Text = _trackChanges
            ? $"Track changes is on ({_trackedChanges.Count} recorded change(s))"
            : "Track changes is off";
    }

    private void OnEditorTextChanged(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (_isLoading || !_trackChanges)
        {
            return;
        }

        _trackedChanges.Add($"{DateTimeOffset.Now:HH:mm:ss} — text changed ({FlowDocumentEditorBox.Document.GetText().Length} characters)");
        EditorStatusText.Text = $"Track changes: {_trackedChanges.Count} change(s) recorded";
    }

    private async void OnExportTextClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            var path = await _documentPicker.PickSaveDocumentAsync(_windowHandleProvider(), "document.html", new[] { ".html", ".txt" });
            if (path is null)
            {
                return;
            }

            var content = FlowDocumentEditorBox.Document.GetText();
            var payload = path.EndsWith(".txt", StringComparison.OrdinalIgnoreCase)
                ? content
                : $"<html><body><pre>{System.Net.WebUtility.HtmlEncode(content)}</pre></body></html>";

            _fileBroker.WriteDocumentBytes(path, Encoding.UTF8.GetBytes(payload));
            EditorStatusText.Text = $"Exported the edited text: {path}";
        }
        catch (Exception ex)
        {
            EditorStatusText.Text = ex.Message;
        }
    }

    private void OnExportPdfClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            _client.EnsureOperationAvailable(CoreOperation.FlowPdfExport);
            EditorStatusText.Text = "Tagged PDF export is available.";
        }
        catch (CoreCapabilityUnavailableException ex)
        {
            EditorStatusText.Text = ex.Message;
        }
    }

    private void ApplyInlineStyle(InlineStyle style)
    {
        if (FlowDocumentEditorBox.Document is not { } document)
        {
            return;
        }

        var selection = document.Selection;
        var start = selection.Start;
        var length = Math.Max(0, selection.End - start);
        document.ApplyInlineStyle(start, length, style);
        EditorStatusText.Text = $"Style applied to {length} character(s)";
    }
}

/// <summary>Inline character formatting of the editor surface.</summary>
public sealed record InlineStyle(bool Bold = false, bool Italic = false, bool Underline = false);

/// <summary>Paragraph formatting of the editor surface.</summary>
public sealed record ParagraphStyle(int HeadingLevel = 0, bool IsCodeBlock = false);

/// <summary>
/// Extensions of the WinUI rich text document that apply the formatting of the
/// editor surface to the current selection.
/// </summary>
public static class RichTextDocumentExtensions
{
    public static void ApplyInlineStyle(this RichTextDocument document, int start, int length, InlineStyle style)
    {
        if (length <= 0)
        {
            return;
        }

        if (style.Bold) document.SetBold(start, length, true);
        if (style.Italic) document.SetItalic(start, length, true);
        if (style.Underline) document.SetUnderline(start, length, true);
    }

    public static void SetParagraphStyle(this RichTextDocument document, int start, int length, ParagraphStyle style)
    {
        if (length <= 0)
        {
            return;
        }

        if (style.HeadingLevel > 0)
        {
            document.SetParagraphStyle(start, length, new RichTextBlockFormat { HeadingLevel = style.HeadingLevel });
        }
    }
}

/// <summary>
/// Reads the text of a document session from the core.
/// </summary>
public static class CoreTextExtractor
{
    public static string ExtractDocumentText(DocumentCoreClient client, DocumentSessionSnapshot session)
    {
        var builder = new StringBuilder();
        for (uint pageIndex = 0; pageIndex < session.PageCount; pageIndex++)
        {
            var selection = client.SelectPageText(session.SessionId, pageIndex);
            builder.AppendLine(selection.CleanedText);
        }

        return builder.ToString();
    }
}
