using System;
using System.Collections.Generic;
using System.IO;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;

namespace Vocument.Windows.Views;

/// <summary>
/// The user actions of the PDF work surface, independent of the WinUI 3 widget tree.
/// <para>
/// The WinUI view (PdfViewerView) is a thin shell over this workflow: every button
/// of the toolbar performs exactly the operation implemented here, so the same
/// behaviour can be verified by automated end-to-end tests on every host that can
/// load the native core.
/// </para>
/// </summary>
public sealed class ViewerWorkflow
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;

    private DocumentSessionSnapshot? _session;

    public ViewerWorkflow(DocumentCoreClient client, IWindowsFileBroker fileBroker)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _fileBroker = fileBroker ?? throw new ArgumentNullException(nameof(fileBroker));
    }

    public DocumentSessionSnapshot? Session => _session;

    public uint CurrentPageIndex { get; private set; }

    public uint ZoomMilli { get; private set; } = 1000;

    public uint PageCount => _session?.PageCount ?? 0;

    /// <summary>
    /// Opens a document through the file broker and the core. An unknown path is
    /// rejected with <see cref="FileNotFoundException"/>, a path the broker refuses
    /// with <see cref="UnauthorizedAccessException"/>.
    /// </summary>
    public DocumentSessionSnapshot OpenDocument(string filePath)
    {
        if (string.IsNullOrWhiteSpace(filePath) || !File.Exists(filePath))
        {
            throw new FileNotFoundException("Cannot open non-existent PDF file", filePath);
        }

        if (!_fileBroker.VerifyAccessPermission(filePath, writeAccess: false))
        {
            throw new UnauthorizedAccessException($"Access denied to document file {filePath}");
        }

        CloseDocument();

        var session = _client.OpenDocument(filePath);
        _session = session;
        CurrentPageIndex = 0;
        ZoomMilli = 1000;
        return session;
    }

    /// <summary>Closes the open session, if there is one.</summary>
    public void CloseDocument()
    {
        if (_session is null)
        {
            return;
        }

        _client.CloseSession(_session.SessionId);
        _session = null;
        CurrentPageIndex = 0;
    }

    /// <summary>Saves the open session through the save pipeline of the core.</summary>
    public void SaveTo(string destinationPath)
    {
        EnsureSession();
        _client.SaveSession(_session!.SessionId, destinationPath);
    }

    public void NavigateNext()
    {
        EnsureSession();
        if (CurrentPageIndex + 1 < _session!.PageCount)
        {
            CurrentPageIndex++;
        }
    }

    public void NavigatePrevious()
    {
        EnsureSession();
        if (CurrentPageIndex > 0)
        {
            CurrentPageIndex--;
        }
    }

    public void SetZoom(uint zoomMilli) => ZoomMilli = Math.Clamp(zoomMilli, 250u, 4000u);

    public void ZoomIn() => SetZoom(ZoomMilli + 250);

    public void ZoomOut() => SetZoom(ZoomMilli > 250 ? ZoomMilli - 250 : 250);

    /// <summary>Renders the current page with the engine of the core.</summary>
    public ViewportTile RenderCurrentPage()
    {
        EnsureSession();
        return _client.RenderViewport(_session!.SessionId, CurrentPageIndex, ZoomMilli);
    }

    /// <summary>Extracts the text of the current page with the core.</summary>
    public TextSelection SelectCurrentPageText()
    {
        EnsureSession();
        return _client.SelectPageText(_session!.SessionId, CurrentPageIndex);
    }

    /// <summary>Runs the exact search of the core over the open document.</summary>
    public IReadOnlyList<SearchHit> Search(string query)
    {
        EnsureSession();
        if (string.IsNullOrWhiteSpace(query))
        {
            return Array.Empty<SearchHit>();
        }

        return _client.SearchExact(_session!.SessionId, query);
    }

    /// <summary>Adds a highlight annotation to the current page.</summary>
    public HighlightAnnotation HighlightCurrentPage(string colorHex = "#FFD54F")
    {
        EnsureSession();
        return _client.AddHighlight(_session!.SessionId, CurrentPageIndex, colorHex);
    }

    /// <summary>Rotates the current page through the transaction manager.</summary>
    public uint RotateCurrentPage(int deltaDegrees)
    {
        EnsureSession();
        return _client.RotatePages(_session!.SessionId, new[] { CurrentPageIndex }, deltaDegrees);
    }

    public bool Undo()
    {
        EnsureSession();
        return _client.Undo(_session!.SessionId);
    }

    public bool Redo()
    {
        EnsureSession();
        return _client.Redo(_session!.SessionId);
    }

    /// <summary>Reports whether the session has a recovery checkpoint.</summary>
    public bool HasRecoveryCheckpoint()
    {
        EnsureSession();
        return _client.CheckRecoveryCheckpoint(_session!.SessionId);
    }

    private void EnsureSession()
    {
        if (_session is null)
        {
            throw new InvalidOperationException("No document is open in the viewer workflow.");
        }
    }
}
