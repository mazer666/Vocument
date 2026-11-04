# Vocument Windows Application Shell (`apps/windows/`)

Windows native application shell (.NET 8 / C# WinUI 3) implementing Track 3 (`CR-0003`, `PLATFORM-002`, `PLATFORM-003`).

## Platform Abstraction (`PLATFORM-003`)

The Windows shell implements the required platform traits:
- `PlatformFileBroker` (`WindowsFileBroker.cs`) — Windows Storage / Common Item Dialog scoped access.
- `PlatformSettingsStore` (`WindowsSettingsStore.cs`) — Local AppData settings storage.
- `PlatformAppLifecycle` (`WindowsAppLifecycle.cs`) — Process lifecycle coordinator.
- `PlatformFontProvider` (`WindowsFontProvider.cs`) — DirectWrite / Windows Font provider.

## Building and Testing

```powershell
# Restore dependencies and build
dotnet build apps/windows/VocumentApp.csproj

# Run unit and acceptance tests
dotnet test apps/windows/VocumentApp.csproj

# Run handshake CLI directly
dotnet run --project apps/windows/VocumentApp.csproj
```

## Shared Acceptance Scenario

The Windows acceptance test (`WindowsAcceptanceScenarioTests.cs`) verifies:
`Open → search → select text → highlight → rotate → undo → redo → save → recover → reopen`.
