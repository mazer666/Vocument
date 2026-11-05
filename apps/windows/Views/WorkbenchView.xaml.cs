using System;
using System.Collections.Generic;
using Vocument.Windows.Native;

namespace Vocument.Windows.Views;

/// <summary>
/// Page Workbench view model and presenter (M3.0-M3.6, PLATFORM-002).
/// Provides structural page mutations, rotations, crops, and sanitization backed by Rust core.
/// </summary>
public sealed class WorkbenchView
{
    private readonly DocumentCoreClient _client;
    private DocumentSessionSnapshot? _activeSession;
    private readonly HashSet<uint> _selectedPages = new();

    public WorkbenchView(DocumentCoreClient client)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
    }

    public DocumentSessionSnapshot? ActiveSession => _activeSession;
    public IReadOnlyCollection<uint> SelectedPages => _selectedPages;

    public void AttachSession(DocumentSessionSnapshot session)
    {
        _activeSession = session ?? throw new ArgumentNullException(nameof(session));
        _selectedPages.Clear();
    }

    public void SelectPage(uint pageIndex)
    {
        if (_activeSession != null && pageIndex < _activeSession.PageCount)
        {
            _selectedPages.Add(pageIndex);
        }
    }

    public void DeselectPage(uint pageIndex)
    {
        _selectedPages.Remove(pageIndex);
    }

    public void ClearSelection()
    {
        _selectedPages.Clear();
    }

    public uint RotateSelectedPages(int deltaDegrees)
    {
        if (_activeSession == null || _selectedPages.Count == 0)
        {
            return 0;
        }

        var pages = new List<uint>(_selectedPages);
        pages.Sort();
        return _client.RotatePages(_activeSession.SessionId, pages, deltaDegrees);
    }

    public bool Undo()
    {
        return _activeSession != null && _client.Undo(_activeSession.SessionId);
    }

    public bool Redo()
    {
        return _activeSession != null && _client.Redo(_activeSession.SessionId);
    }
}
