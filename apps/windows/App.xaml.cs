using System;
using Vocument.Windows.Native;
using Vocument.Windows.Platform;

namespace Vocument.Windows;

/// <summary>
/// WinUI 3 Application entry and lifecycle manager (CR-0003, PLATFORM-002, PLATFORM-003).
/// Enforces offline-first defaults (NET-001) and no telemetry (LOG-001).
/// </summary>
public sealed class App : IDisposable
{
    private readonly DocumentCoreClient _client;
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsSettingsStore _settingsStore;
    private readonly IWindowsAppLifecycle _lifecycle;
    private MainWindow? _mainWindow;

    public App()
    {
        _client = new DocumentCoreClient();
        _fileBroker = new WindowsFileBroker();
        _settingsStore = new WindowsSettingsStore();
        _lifecycle = new WindowsAppLifecycle();
    }

    public MainWindow LaunchMainWindow()
    {
        _mainWindow = new MainWindow(_client, _fileBroker, _settingsStore, _lifecycle);
        _lifecycle.NotifyForeground();
        return _mainWindow;
    }

    public void Dispose()
    {
        _lifecycle.NotifyShutdown();
        _client.Dispose();
    }
}
