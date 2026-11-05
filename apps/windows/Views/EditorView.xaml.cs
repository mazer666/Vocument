using System;
using Vocument.Windows.Native;

namespace Vocument.Windows.Views;

/// <summary>
/// Flow Document &amp; Office Editor View (M4.0-M4.5, DOC-001).
/// Interfaces with Rust FlowDocument layout engine and deterministic PDF/A-2b / UA-1 exporter.
/// </summary>
public sealed class EditorView
{
    private readonly DocumentCoreClient _client;
    private string _currentDocumentContent = string.Empty;
    private bool _isDirty;

    public EditorView(DocumentCoreClient client)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
    }

    public string CurrentDocumentContent => _currentDocumentContent;
    public bool IsDirty => _isDirty;

    public void LoadContent(string content)
    {
        _currentDocumentContent = content ?? string.Empty;
        _isDirty = false;
    }

    public void UpdateContent(string updatedContent)
    {
        _currentDocumentContent = updatedContent ?? string.Empty;
        _isDirty = true;
    }
}
