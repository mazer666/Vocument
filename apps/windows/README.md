# Vocument Windows Application Shell (`apps/windows/`)

Windows native application shell (.NET 8 / C# / **WinUI 3 / Windows App SDK**) implementing Track 3 (`CR-0003`, `PLATFORM-002`, `PLATFORM-003`).

The shell is a real WinUI 3 desktop application: `App.xaml` is a `Microsoft.UI.Xaml.Application`,
`MainWindow.xaml` is a `Microsoft.UI.Xaml.Window` and the four work surfaces are
`Microsoft.UI.Xaml.Controls.UserControl` instances with XAML code-behind.

## Architecture

| Layer | Files | Responsibility |
|---|---|---|
| WinUI shell | `App.xaml(.cs)`, `MainWindow.xaml(.cs)`, `Views/*.xaml(.cs)` | Window lifecycle, navigation, command wiring |
| Native bridge | `Native/VocumentNativeBridge.cs`, `Native/DocumentCoreOperations.cs`, `Native/DocumentCoreCapabilities.cs` | P/Invoke into `vocument_ffi`, capability handling |
| Platform services | `Platform/WindowsFileBroker.cs`, `WindowsSettingsStore.cs`, `WindowsAppLifecycle.cs`, `WindowsFontProvider.cs`, `WindowsDocumentPicker.cs` | Scoped file access, settings, lifecycle, Common Item Dialog |
| Engines | `Comparison/DocumentComparisonEngine.cs`, `Rendering/RasterBitmapEncoder.cs` | Text diff, pixel diff, RGBA → BMP for the XAML image pipeline |

The shell owns no document logic: every document operation is executed by the shared Rust
core through `DocumentCoreClient` (C ABI of `platform/vocument-ffi`).

## Core capability handling (`QA-003`)

The Windows C ABI currently exports the document session operations of the PDF workbench
(open, geometry, render, search, text selection, highlight, rotate, undo, redo, save,
recovery checkpoint). Operations that exist in the core but are **not** exported by the C
ABI are never emulated in the shell. `DocumentCoreClient.EnsureOperationAvailable()`
probes the linked library with `NativeLibrary.TryGetExport` and reports the exact missing
entry point:

| Operation | Required core entry point |
|---|---|
| Move pages | `vocument_c_reorder_pages` |
| Delete pages | `vocument_c_delete_pages` |
| Extract pages | `vocument_c_extract_pages` |
| Stamp pages | `vocument_c_apply_stamp` |
| Open `.pdoc` | `vocument_c_open_flow_document` |
| Tagged PDF export | `vocument_c_export_flow_pdf` |
| Core comparison | `vocument_c_compare_documents` |

The comparison surface of the shell does not depend on the last entry point: the text and
the visual diff are computed from core data (extracted page text and MuPDF rasters).

## Building and Testing

```powershell
# Windows: build the WinUI 3 shell (requires the Windows SDK and the Windows App SDK)
dotnet build apps/windows/VocumentApp.csproj

# Windows and macOS: run the acceptance scenario and the engine tests
dotnet test apps/windows/VocumentApp.csproj

# macOS / Linux: headless run of the shared acceptance scenario
dotnet run --project apps/windows/VocumentApp.csproj
```

On non-Windows hosts the project targets `net8.0` and excludes the WinUI sources
(`App.xaml.cs`, `MainWindow.xaml.cs`, `Views/**`, `Platform/WindowsDocumentPicker.cs`);
the native bridge, the platform layer, the comparison engines and the tests stay
platform independent.

## Shared Acceptance Scenario

`Tests/WindowsAcceptanceScenarioTests.cs` verifies:
`Open → search → select text → highlight → rotate → undo → redo → save → recover → reopen`.

`Tests/ComparisonEngineTests.cs` verifies the text diff, the pixel diff and the raster
encoder that the shell uses to display the core output.

## GUI end-to-end gate

`Tests/WindowsGuiEndToEndTests.cs` drives the user actions of the shell against the real
core and the real MuPDF engine:

* the actions of the viewer (`Views/ViewerWorkflow.cs`) — open, navigate, zoom, select
  text, search, highlight, rotate, undo, redo, save and reopen — including the proof
  that the saved state survives the round trip through the file system;
* the error handling — an unknown path raises `FileNotFoundException`, a path the file
  broker refuses raises `UnauthorizedAccessException`;
* the actions of the comparison (`Views/CompareWorkflow.cs`) — the text and the visual
  difference of two real corpus documents;
* on Windows additionally the XAML contract of the views: every handler that the XAML
  declares must exist on the code-behind with a routed event argument, and the shell
  types must derive from the Windows App SDK base types.

The workflow classes hold the actions of the views, so the gate verifies exactly the
behaviour of the WinUI 3 shell while staying runnable on macOS and Linux.

## Continuous integration (`.github/workflows/ci-windows.yml`)

1. build the core (`cargo build -p vocument-ffi`) and the MuPDF runtime for Windows;
2. run the native acceptance scenario (`dotnet test apps/windows/VocumentApp.csproj`);
3. publish the unpackaged application
   (`dotnet publish … -c Release -r win-x64 -p:Platform=x64 --self-contained false`);
4. assemble the internal distribution with `vocument_ffi.dll` and the MuPDF DLLs and
   upload `Vocument-Windows-x64-internal.zip` as the workflow artifact
   `vocument-windows-x64-internal`;
5. run the GUI end-to-end gate and confirm it with `WINUI_GATE=PASS`.
