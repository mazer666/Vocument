using System;
using System.IO;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;

namespace Vocument.Windows.Views;

/// <summary>
/// Native WinUI 3 Document Viewport View (M0.1-M0.7, PLATFORM-002).
/// Displays rendered MuPDF raster tiles with zoom, navigation, selection, and transaction support.
/// </summary>
public sealed class PdfViewerView
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private DocumentSessionSnapshot? _activeSession;
    private uint _currentPageIndex;
    private uint _currentZoomMilli = 1000;

    public PdfViewerView(DocumentCoreClient client, IWindowsFileBroker fileBroker)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _fileBroker = fileBroker ?? throw new ArgumentNullException(nameof(fileBroker));
    }

    public DocumentSessionSnapshot? ActiveSession => _activeSession;
    public uint CurrentPageIndex => _currentPageIndex;
    public uint CurrentZoomMilli => _currentZoomMilli;

    public void OpenDocument(string filePath)
    {
        if (string.IsNullOrEmpty(filePath) || !File.Exists(filePath))
        {
            throw new FileNotFoundException("Cannot open non-existent PDF file", filePath);
        }

        if (!_fileBroker.VerifyAccessPermission(filePath, writeAccess: false))
        {
            throw new UnauthorizedAccessException($"Access denied to document file {filePath}");
        }

        if (_activeSession != null)
        {
            _client.CloseSession(_activeSession.SessionId);
            _activeSession = null;
        }

        _activeSession = _client.OpenDocument(filePath);
        _currentPageIndex = 0;
        _currentZoomMilli = 1000;
    }

    public PageGeometry GetCurrentPageGeometry()
    {
        EnsureActiveSession();
        return _client.GetPageGeometry(_activeSession!.SessionId, _currentPageIndex);
    }

    public ViewportTile RenderCurrentPage()
    {
        EnsureActiveSession();
        return _client.RenderViewport(_activeSession!.SessionId, _currentPageIndex, _currentZoomMilli);
    }

    public void NavigateNextPage()
    {
        EnsureActiveSession();
        if (_currentPageIndex + 1 < _activeSession!.PageCount)
        {
            _currentPageIndex++;
        }
    }

    public void NavigatePreviousPage()
    {
        EnsureActiveSession();
        if (_currentPageIndex > 0)
        {
            _currentPageIndex--;
        }
    }

    public void SetZoom(uint zoomMilli)
    {
        _currentZoomMilli = Math.Clamp(zoomMilli, 250, 4000);
    }

    public void ZoomIn()
    {
        SetZoom(_currentZoomMilli + 250);
    }

    public void ZoomOut()
    {
        SetZoom(_currentZoomMilli > 250 ? _currentZoomMilli - 250 : 250);
    }

    public IReadOnlyList<SearchHit> Search(string query)
    {
        EnsureActiveSession();
        if (string.IsNullOrWhiteSpace(query))
        {
            return Array.Empty<SearchHit>();
        }

        return _client.SearchExact(_activeSession!.SessionId, query);
    }

    public TextSelection SelectCurrentPageText()
    {
        EnsureActiveSession();
        return _client.SelectPageText(_activeSession!.SessionId, _currentPageIndex);
    }

    public HighlightAnnotation HighlightCurrentPage(string colorHex = "#FFFF00")
    {
        EnsureActiveSession();
        return _client.AddHighlight(_activeSession!.SessionId, _currentPageIndex, colorHex);
    }

    public uint RotateCurrentPage(int deltaDegrees)
    {
        EnsureActiveSession();
        return _client.RotatePages(_activeSession!.SessionId, new[] { _currentPageIndex }, deltaDegrees);
    }

    public bool Undo()
    {
        EnsureActiveSession();
        return _client.Undo(_activeSession!.SessionId);
    }

    public bool Redo()
    {
        EnsureActiveSession();
        return _client.Redo(_activeSession!.SessionId);
    }

    public void Save(string destinationPath)
    {
        EnsureActiveSession();
        _client.SaveSession(_activeSession!.SessionId, destinationPath);
    }

    private void EnsureActiveSession()
    {
        if (_activeSession == null)
        {
            throw new InvalidOperationException("No PDF document currently opened in PdfViewerView");
        }
    }
}
