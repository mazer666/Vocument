using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Windows.Storage.Pickers;

namespace Vocument.Windows.Platform;

/// <summary>
/// Opens the native file dialogs of Windows. The shell never touches the file
/// system for user chosen documents without going through this broker (FILE-001).
/// </summary>
public interface IWindowsDocumentPicker
{
    /// <summary>
    /// Shows the open dialog and returns the selected path, or <c>null</c> when the
    /// user cancelled the dialog.
    /// </summary>
    Task<string?> PickOpenDocumentAsync(IntPtr ownerWindowHandle, IReadOnlyList<string> fileTypeFilters);

    /// <summary>
    /// Shows the save dialog and returns the chosen target path, or <c>null</c> when
    /// the user cancelled the dialog.
    /// </summary>
    Task<string?> PickSaveDocumentAsync(
        IntPtr ownerWindowHandle,
        string suggestedFileName,
        IReadOnlyList<string> fileTypeFilters);
}

/// <summary>
/// Implementation on top of the Windows Common Item Dialog (FileOpenPicker and
/// FileSavePicker of the Windows App SDK). The pickers are bound to the window of
/// the shell, because an unpackaged WinUI 3 application has no implicit owner.
/// </summary>
public sealed class WindowsDocumentPicker : IWindowsDocumentPicker
{
    public async Task<string?> PickOpenDocumentAsync(IntPtr ownerWindowHandle, IReadOnlyList<string> fileTypeFilters)
    {
        var picker = new FileOpenPicker();
        foreach (var filter in fileTypeFilters)
        {
            picker.FileTypeFilter.Add(filter);
        }

        WinRT.Interop.InitializeWithWindow.Initialize(picker, ownerWindowHandle);

        var file = await picker.PickSingleFileAsync();
        return file?.Path;
    }

    public async Task<string?> PickSaveDocumentAsync(
        IntPtr ownerWindowHandle,
        string suggestedFileName,
        IReadOnlyList<string> fileTypeFilters)
    {
        var picker = new FileSavePicker();
        foreach (var filter in fileTypeFilters)
        {
            picker.FileTypeFilter.Add(filter);
        }

        picker.SuggestedFileName = suggestedFileName;
        WinRT.Interop.InitializeWithWindow.Initialize(picker, ownerWindowHandle);

        var file = await picker.PickSaveFileAsync();
        return file?.Path;
    }
}
