using System;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Vocument.Windows.Views;

namespace Vocument.Windows;

/// <summary>
/// Main Application Window for Vocument on Windows 11 (PLATFORM-002, CR-0003).
/// Hosts the primary navigation tabs: PDF Viewer, Page Workbench, Flow Editor, and Document Compare.
/// </summary>
public sealed class MainWindow
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsSettingsStore _settingsStore;
    private readonly IWindowsAppLifecycle _lifecycle;

    public PdfViewerView ViewerView { get; }
    public WorkbenchView WorkbenchView { get; }
    public EditorView EditorView { get; }
    public CompareView CompareView { get; }

    public MainWindow(
        DocumentCoreClient client,
        IWindowsFileBroker fileBroker,
        IWindowsSettingsStore settingsStore,
        IWindowsAppLifecycle lifecycle)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _fileBroker = fileBroker ?? throw new ArgumentNullException(nameof(fileBroker));
        _settingsStore = settingsStore ?? throw new ArgumentNullException(nameof(settingsStore));
        _lifecycle = lifecycle ?? throw new ArgumentNullException(nameof(lifecycle));

        ViewerView = new PdfViewerView(_client, _fileBroker);
        WorkbenchView = new WorkbenchView(_client);
        EditorView = new EditorView(_client);
        CompareView = new CompareView(_client);
    }
}
