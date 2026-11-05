using System;
using System.IO;
using Vocument.Windows.Native;

namespace Vocument.Windows.Views;

/// <summary>
/// Side-by-side synchronized document comparison view (M3.7, CR-0005, PLATFORM-002).
/// Displays dual viewports, visual diff rasterization, structural text changes, and report export.
/// </summary>
public sealed class CompareView
{
    private readonly DocumentCoreClient _client;
    private string? _documentAPath;
    private string? _documentBPath;
    private DocumentSessionSnapshot? _sessionA;
    private DocumentSessionSnapshot? _sessionB;

    public CompareView(DocumentCoreClient client)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
    }

    public string? DocumentAPath => _documentAPath;
    public string? DocumentBPath => _documentBPath;

    public void LoadDocumentA(string path)
    {
        if (string.IsNullOrEmpty(path) || !File.Exists(path))
        {
            throw new FileNotFoundException("Document A not found", path);
        }
        _documentAPath = path;
        _sessionA = _client.OpenDocument(path);
    }

    public void LoadDocumentB(string path)
    {
        if (string.IsNullOrEmpty(path) || !File.Exists(path))
        {
            throw new FileNotFoundException("Document B not found", path);
        }
        _documentBPath = path;
        _sessionB = _client.OpenDocument(path);
    }

    public bool CanCompare => _sessionA != null && _sessionB != null;

    public void CloseDocuments()
    {
        if (_sessionA != null)
        {
            _client.CloseSession(_sessionA.SessionId);
            _sessionA = null;
        }
        if (_sessionB != null)
        {
            _client.CloseSession(_sessionB.SessionId);
            _sessionB = null;
        }
        _documentAPath = null;
        _documentBPath = null;
    }
}
