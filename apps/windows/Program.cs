using System;
using Vocument.Windows.Tests;

namespace Vocument.Windows;

public static class Program
{
    public static int Main(string[] args)
    {
        Console.WriteLine("==> Executing Vocument Windows Handshake & Acceptance Scenario...");
        try
        {
            var tests = new WindowsAcceptanceScenarioTests();
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
}
