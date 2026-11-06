using System;
using System.Runtime.InteropServices;
#if WINUI_DESKTOP
using Microsoft.Windows.AppLifecycle;
using WinRT;
#endif

namespace Vocument.Windows;

/// <summary>
/// Entry point of the Vocument Windows shell.
/// <para>
/// On Windows the process is a real WinUI 3 desktop application: the Windows App SDK
/// runtime is started through <c>Microsoft.UI.Xaml.Application.Start</c> after the
/// process requirements (DPI awareness, supported OS build, single instance) have been
/// verified. On other hosts the same project keeps the headless native acceptance
/// scenario runnable, because the Windows SDK targets do not exist there.
/// </para>
/// </summary>
public static class Program
{
#if WINUI_DESKTOP
    /// <summary>Process entry point of the unpackaged WinUI 3 desktop application.</summary>
    [STAThread]
    public static void Main(string[] args)
    {
        // CsWinRT wrappers must be initialised before any WinRT object is created.
        ComWrappersSupport.InitializeComWrappers();

        XamlCheckProcessRequirements();

        Microsoft.UI.Xaml.Application.Start((p) => new App());
    }

    /// <summary>
    /// Verifies the process requirements of the WinUI 3 XAML runtime before the
    /// application object is started. Every check that cannot be satisfied aborts the
    /// start-up with an explicit message instead of letting the XAML runtime fail with
    /// an opaque COM error.
    /// </summary>
    private static void XamlCheckProcessRequirements()
    {
        EnsurePerMonitorV2DpiAwareness();
        EnsureSupportedWindowsBuild();
        EnsureSingleInstanceActivation();
    }

    /// <summary>
    /// The WinUI 3 compositor requires per-monitor DPI awareness v2. The process is
    /// started without an application manifest, so the awareness context is requested
    /// explicitly. A failure with <c>ERROR_ACCESS_DENIED</c> means the awareness was
    /// already set (for example by the Windows App SDK bootstrap) and is not an error.
    /// </summary>
    private static void EnsurePerMonitorV2DpiAwareness()
    {
        const int errorAccessDenied = 5;
        var perMonitorAwareV2 = new IntPtr(-4);

        if (SetProcessDpiAwarenessContext(perMonitorAwareV2))
        {
            return;
        }

        var lastError = Marshal.GetLastWin32Error();
        if (lastError != errorAccessDenied)
        {
            throw new InvalidOperationException(
                $"Per-monitor DPI awareness v2 could not be enabled (Win32 error {lastError}).");
        }
    }

    /// <summary>
    /// The Windows App SDK 1.6 requires Windows 10 build 17763 or newer.
    /// </summary>
    private static void EnsureSupportedWindowsBuild()
    {
        var version = Environment.OSVersion.Version;
        var minimum = new Version(10, 0, 17763);
        if (version < minimum)
        {
            throw new PlatformNotSupportedException(
                $"Vocument requires Windows 10 build {minimum.Build} or newer; this host reports {version}.");
        }
    }

    /// <summary>
    /// An unpackaged WinUI 3 application may be started more than once. The first
    /// process owns the main key, every further start is redirected to it and exits.
    /// </summary>
    private static void EnsureSingleInstanceActivation()
    {
        var mainInstance = AppInstance.FindOrRegisterForKey("Vocument.Windows.MainWindow");
        if (mainInstance.IsCurrent)
        {
            return;
        }

        var activationArgs = AppInstance.GetCurrent().GetActivatedEventArgs();
        mainInstance.RedirectActivationToAsync(activationArgs).AsTask().GetAwaiter().GetResult();
        Environment.Exit(0);
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetProcessDpiAwarenessContext(IntPtr value);
#else
    /// <summary>
    /// Headless entry point for non-Windows hosts: the native acceptance scenario is
    /// executed against the real Rust core so that the shared gate stays verifiable
    /// on every platform that can build the native library.
    /// </summary>
    public static int Main(string[] args)
    {
        Console.WriteLine("==> Executing Vocument Windows Handshake & Acceptance Scenario...");
        try
        {
            var tests = new Tests.WindowsAcceptanceScenarioTests();
            tests.TestSharedAcceptanceScenario_Windows();
            Console.WriteLine("✓ Windows Acceptance Scenario succeeded: Open → search → select → highlight → rotate → undo → redo → save → recover → reopen");
            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"ERROR: Windows Acceptance Scenario failed: {ex.Message}");
            Console.Error.WriteLine(ex.StackTrace);
            return 1;
        }
    }
#endif
}
