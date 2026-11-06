using System;
using System.Runtime.InteropServices;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;
using Vocument.Windows.Views;

namespace Vocument.Windows;

/// <summary>
/// Main WinUI 3 window of the Vocument Windows shell (CR-0003, PLATFORM-002).
/// Hosts the four work surfaces of the shell: PDF viewer, page workbench, flow editor
/// and document comparison. Exactly one of them is visible at a time.
/// </summary>
public sealed partial class MainWindow : Microsoft.UI.Xaml.Window
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsSettingsStore _settingsStore;
    private readonly IWindowsAppLifecycle _lifecycle;
    private bool _viewsInitialized;

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

        var documentPicker = new WindowsDocumentPicker();
        Func<IntPtr> windowHandle = () => WindowHandle;

        ViewerView = new PdfViewerView(_client, _fileBroker, documentPicker, windowHandle);
        WorkbenchView = new WorkbenchView(_client, _fileBroker, documentPicker, windowHandle);
        EditorView = new EditorView(_client, _fileBroker, documentPicker, windowHandle);
        CompareView = new CompareView(_client, _fileBroker, documentPicker, windowHandle);

        InitializeComponent();

        // The Checked handlers of the navigation radio buttons run while the XAML tree
        // is created; the content host is only switched once the tree is complete.
        _viewsInitialized = true;
        MainContentHost.Content = ViewerView;

        ApplyWindowChrome();
        Activated += OnWindowActivated;
    }

    private IntPtr WindowHandle => WinRT.Interop.WindowNative.GetWindowHandle(this);

    private void ShowView(object view)
    {
        if (!_viewsInitialized)
        {
            return;
        }

        MainContentHost.Content = view;
    }

    private void OnViewerTabChecked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) => ShowView(ViewerView);

    private void OnWorkbenchTabChecked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) => ShowView(WorkbenchView);

    private void OnEditorTabChecked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) => ShowView(EditorView);

    private void OnCompareTabChecked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) => ShowView(CompareView);

    /// <summary>
    /// Shows the effective configuration of the shell. The values are read from the
    /// persisted settings store and from the live handshake with the Rust core, so
    /// the dialog never shows a hard-coded state.
    /// </summary>
    private async void OnSettingsClicked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var offlineDefault = _settingsStore.GetBooleanSetting("offline_default", true);
        var telemetry = _settingsStore.GetBooleanSetting("telemetry_enabled", false);
        var locale = _settingsStore.GetSetting("locale") ?? "de-AT";

        var coreLine = "Core handshake unavailable.";
        try
        {
            var handshake = _client.PerformHandshake();
            coreLine = $"Core {handshake.BaselineVersion} ({handshake.Milestone}), engine offline default {handshake.StrictOfflineDefault}, telemetry {handshake.TelemetryEnabled}, OCR languages {string.Join(", ", handshake.BuiltInOcrLanguages)}.";
        }
        catch (Exception ex)
        {
            coreLine = $"Core handshake failed: {ex.Message}";
        }

        var dialog = new Microsoft.UI.Xaml.Controls.ContentDialog
        {
            XamlRoot = MainContentHost.XamlRoot,
            Title = "Settings",
            CloseButtonText = "Close",
            Content = $"Offline-first: {offlineDefault}\nTelemetry: {telemetry}\nLocale: {locale}\n\n{coreLine}"
        };

        await dialog.ShowAsync();
    }

    private void OnWindowActivated(object sender, Microsoft.UI.Xaml.WindowActivatedEventArgs args)
    {
        ApplyWindowChrome();
    }

    /// <summary>
    /// The unpackaged WinUI 3 window has no XAML title, so the caption and the
    /// initial size are applied through the window handle of the real window.
    /// </summary>
    private void ApplyWindowChrome()
    {
        var handle = WindowHandle;
        if (handle == IntPtr.Zero)
        {
            return;
        }

        SetWindowText(handle, "Vocument");
        MoveWindow(handle, 0, 0, 1440, 900, true);
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetWindowText(IntPtr windowHandle, string text);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool MoveWindow(IntPtr windowHandle, int x, int y, int width, int height, bool repaint);
}
