using System;

namespace Vocument.Windows.Platform;

/// <summary>
/// Windows application lifecycle state (PLATFORM-003).
/// </summary>
public enum AppLifecycleState
{
    Active,
    Suspended,
    Terminating
}

public interface IWindowsAppLifecycle
{
    AppLifecycleState CurrentState { get; }
    void NotifyForeground();
    void NotifyBackground();
    void NotifyShutdown();
    event Action<AppLifecycleState>? StateChanged;
}

public sealed class WindowsAppLifecycle : IWindowsAppLifecycle
{
    public AppLifecycleState CurrentState { get; private set; } = AppLifecycleState.Active;
    public event Action<AppLifecycleState>? StateChanged;

    public void NotifyForeground()
    {
        CurrentState = AppLifecycleState.Active;
        StateChanged?.Invoke(CurrentState);
    }

    public void NotifyBackground()
    {
        CurrentState = AppLifecycleState.Suspended;
        StateChanged?.Invoke(CurrentState);
    }

    public void NotifyShutdown()
    {
        CurrentState = AppLifecycleState.Terminating;
        StateChanged?.Invoke(CurrentState);
    }
}
