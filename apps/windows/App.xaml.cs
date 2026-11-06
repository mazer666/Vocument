using Vocument.Windows.Native;
using Vocument.Windows.Platform;

namespace Vocument.Windows;

/// <summary>
/// WinUI 3 application object of the Vocument Windows shell (CR-0003, PLATFORM-002).
/// <para>
/// The application owns the single <see cref="DocumentCoreClient"/> (the P/Invoke
/// bridge into the Rust core), the platform services (file broker, settings store,
/// lifecycle) and hands them to the <see cref="MainWindow"/>. Offline-first defaults
/// (NET-001) and the absence of telemetry (LOG-001) are properties of the core and
/// are not changed by the shell.
/// </para>
/// </summary>
public partial class App : Microsoft.UI.Xaml.Application
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsSettingsStore _settingsStore;
    private readonly IWindowsAppLifecycle _lifecycle;
    private MainWindow? _mainWindow;

    public App()
    {
        _fileBroker = new WindowsFileBroker();
        _settingsStore = new WindowsSettingsStore();
        _client = new DocumentCoreClient(_fileBroker, _settingsStore);
        _lifecycle = new WindowsAppLifecycle();

        InitializeComponent();
    }

    /// <summary>
    /// Creates the main window, hands the core client and the platform services to it
    /// and shows it. The window instance is kept so that the shell owns exactly one
    /// window for the whole lifetime of the process.
    /// </summary>
    protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
    {
        _mainWindow = new MainWindow(_client, _fileBroker, _settingsStore, _lifecycle);
        _lifecycle.NotifyForeground();
        _mainWindow.Activate();
    }
}
