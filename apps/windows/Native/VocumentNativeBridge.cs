using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;
using Vocument.Windows.Platform;

namespace Vocument.Windows.Native;

public sealed record DocumentCoreHandshake(
    string BaselineVersion,
    string Milestone,
    bool StrictOfflineDefault,
    bool TelemetryEnabled,
    uint TargetFps,
    string DefaultGermanVariant,
    IReadOnlyList<string> BuiltInOcrLanguages
);

public sealed record DocumentSessionSnapshot(
    string SessionId,
    string DocumentId,
    string AccessOrigin,
    ulong SizeBytes,
    uint PageCount,
    string? Title,
    string? Author,
    string EngineIdentifier
);

public sealed record PageGeometry(
    uint PageIndex,
    uint WidthMilliPt,
    uint HeightMilliPt
);

public sealed record ViewportTile(
    uint PageIndex,
    uint WidthPx,
    uint HeightPx,
    byte[] ArgbPixels
);

public sealed record SearchHit(
    uint PageIndex,
    string LogicalPageLabel,
    string Snippet,
    uint MatchCount
);

public sealed record TextSelection(
    uint PageIndex,
    string RawText,
    string CleanedText
);

public sealed record HighlightAnnotation(
    string AnnotationId,
    uint PageIndex,
    string ColorHex,
    string? Author
);

internal static class NativeMethods
{
    private const string LibName = "vocument_ffi";

    private static IntPtr _loadedHandle;

    static NativeMethods()
    {
        NativeLibrary.SetDllImportResolver(typeof(NativeMethods).Assembly, ResolveNativeLibrary);
    }

    /// <summary>
    /// Reports whether the linked core build exports the given C ABI entry point. The
    /// shell uses this to tell apart "the core cannot do this" from "the Windows
    /// client does not implement this entry point yet".
    /// </summary>
    internal static bool TryGetExport(string entryPointName)
    {
        var handle = _loadedHandle;
        if (handle == IntPtr.Zero)
        {
            handle = LoadNativeLibrary();
        }

        return handle != IntPtr.Zero && NativeLibrary.TryGetExport(handle, entryPointName, out _);
    }

    private static IntPtr ResolveNativeLibrary(string libraryName, System.Reflection.Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (libraryName != LibName) return IntPtr.Zero;

        var handle = LoadNativeLibrary();
        if (handle != IntPtr.Zero)
        {
            _loadedHandle = handle;
            return handle;
        }

        if (NativeLibrary.TryLoad(libraryName, assembly, searchPath, out var defaultHandle))
        {
            _loadedHandle = defaultHandle;
            return defaultHandle;
        }

        return IntPtr.Zero;
    }

    private static IntPtr LoadNativeLibrary()
    {
        string extension;
        string prefix = "";
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            extension = ".dll";
        }
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
        {
            prefix = "lib";
            extension = ".dylib";
        }
        else
        {
            prefix = "lib";
            extension = ".so";
        }

        string fileName = $"{prefix}{LibName}{extension}";

        // Search paths:
        // 1. AppContext.BaseDirectory
        // 2. target/debug and target/release relative to repo root
        // 3. Environment library path
        var candidates = new List<string>
        {
            Path.Combine(AppContext.BaseDirectory, fileName)
        };

        var dir = AppContext.BaseDirectory;
        while (!string.IsNullOrEmpty(dir))
        {
            var debugPath = Path.Combine(dir, "target", "debug", fileName);
            if (File.Exists(debugPath)) candidates.Add(debugPath);

            var releasePath = Path.Combine(dir, "target", "release", fileName);
            if (File.Exists(releasePath)) candidates.Add(releasePath);

            if (File.Exists(Path.Combine(dir, "Cargo.toml"))) break;
            dir = Path.GetDirectoryName(dir);
        }

        foreach (var path in candidates)
        {
            if (File.Exists(path) && NativeLibrary.TryLoad(path, out var handle))
            {
                _loadedHandle = handle;
                return handle;
            }
        }

        return IntPtr.Zero;
    }

    [DllImport(LibName, EntryPoint = "vocument_c_handshake", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr VocumentHandshake();

    [DllImport(LibName, EntryPoint = "vocument_c_open_document", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr VocumentOpenDocument([MarshalAs(UnmanagedType.LPUTF8Str)] string path);

    [DllImport(LibName, EntryPoint = "vocument_c_get_page_geometry", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentGetPageGeometry([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, uint pageIndex, out uint widthMilli, out uint heightMilli);

    [DllImport(LibName, EntryPoint = "vocument_c_render_page_tile", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentRenderPageTile([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, uint pageIndex, uint scaleMilli, uint tileX, uint tileY, uint tileW, uint tileH, byte[] outPixels, nuint maxLen);

    [DllImport(LibName, EntryPoint = "vocument_c_search_exact", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr VocumentSearchExact([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, [MarshalAs(UnmanagedType.LPUTF8Str)] string query);

    [DllImport(LibName, EntryPoint = "vocument_c_select_page_text", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr VocumentSelectPageText([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, uint pageIndex);

    [DllImport(LibName, EntryPoint = "vocument_c_add_highlight", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr VocumentAddHighlight([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, uint pageIndex, [MarshalAs(UnmanagedType.LPUTF8Str)] string colorHex);

    [DllImport(LibName, EntryPoint = "vocument_c_rotate_pages", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentRotatePages([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, [In] uint[] pageIndices, nuint numPages, int deltaDegrees);

    [DllImport(LibName, EntryPoint = "vocument_c_undo", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentUndo([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId);

    [DllImport(LibName, EntryPoint = "vocument_c_redo", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentRedo([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId);

    [DllImport(LibName, EntryPoint = "vocument_c_save_session", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr VocumentSaveSession([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId, [MarshalAs(UnmanagedType.LPUTF8Str)] string destinationPath);

    [DllImport(LibName, EntryPoint = "vocument_c_check_recovery", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentCheckRecovery([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId);

    [DllImport(LibName, EntryPoint = "vocument_c_close_session", CallingConvention = CallingConvention.Cdecl)]
    public static extern int VocumentCloseSession([MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId);

    [DllImport(LibName, EntryPoint = "vocument_c_free_string", CallingConvention = CallingConvention.Cdecl)]
    public static extern void VocumentFreeString(IntPtr str);
}

public sealed partial class DocumentCoreClient : IDisposable
{
    private readonly IWindowsFileBroker _fileBroker;
    private readonly IWindowsSettingsStore _settingsStore;
    private readonly IWindowsFontProvider _fontProvider;
    private readonly List<string> _activeSessions = new();

    public DocumentCoreClient(
        IWindowsFileBroker? fileBroker = null,
        IWindowsSettingsStore? settingsStore = null,
        IWindowsFontProvider? fontProvider = null)
    {
        _fileBroker = fileBroker ?? new WindowsFileBroker();
        _settingsStore = settingsStore ?? new WindowsSettingsStore();
        _fontProvider = fontProvider ?? new WindowsFontProvider();
    }

    private static string MarshalAndFree(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return string.Empty;
        try
        {
            return Marshal.PtrToStringUTF8(ptr) ?? string.Empty;
        }
        finally
        {
            NativeMethods.VocumentFreeString(ptr);
        }
    }

    public DocumentCoreHandshake PerformHandshake()
    {
        var jsonStr = MarshalAndFree(NativeMethods.VocumentHandshake());
        if (string.IsNullOrEmpty(jsonStr))
        {
            throw new InvalidOperationException("Failed to invoke vocument_c_handshake native API");
        }

        using var doc = JsonDocument.Parse(jsonStr);
        var root = doc.RootElement;
        if (root.TryGetProperty("error", out var errProp))
        {
            throw new InvalidOperationException($"Core handshake error: {errProp.GetString()}");
        }

        var baseline = root.GetProperty("baseline_version").GetString() ?? "0.1";
        var milestone = root.GetProperty("milestone").GetString() ?? "M3";
        var offline = root.GetProperty("strict_offline_default").GetBoolean();
        var telemetry = root.GetProperty("telemetry_enabled").GetBoolean();
        var fps = root.GetProperty("target_fps").GetUInt32();
        var locale = root.GetProperty("default_german_variant").GetString() ?? "de-AT";

        var langs = new List<string>();
        if (root.TryGetProperty("built_in_ocr_languages", out var langsElem))
        {
            foreach (var item in langsElem.EnumerateArray())
            {
                var s = item.GetString();
                if (s != null) langs.Add(s);
            }
        }

        return new DocumentCoreHandshake(
            BaselineVersion: baseline,
            Milestone: milestone,
            StrictOfflineDefault: offline,
            TelemetryEnabled: telemetry,
            TargetFps: fps,
            DefaultGermanVariant: locale,
            BuiltInOcrLanguages: langs
        );
    }

    public DocumentSessionSnapshot OpenDocument(string filePath)
    {
        ArgumentException.ThrowIfNullOrEmpty(filePath);

        if (!_fileBroker.VerifyAccessPermission(filePath, writeAccess: false))
        {
            throw new UnauthorizedAccessException($"Access to {filePath} not permitted by WindowsFileBroker");
        }

        var jsonStr = MarshalAndFree(NativeMethods.VocumentOpenDocument(filePath));
        if (string.IsNullOrEmpty(jsonStr))
        {
            throw new InvalidOperationException($"Native open call returned null pointer for {filePath}");
        }

        using var doc = JsonDocument.Parse(jsonStr);
        var root = doc.RootElement;
        if (root.TryGetProperty("error", out var errProp))
        {
            throw new InvalidOperationException($"Failed to open document: {errProp.GetString()}");
        }

        var sessionId = root.GetProperty("session_id").GetString()!;
        var documentId = root.GetProperty("document_id").GetString()!;
        var origin = root.GetProperty("access_origin").GetString() ?? "WindowsCommonItemDialog";
        var sizeBytes = root.GetProperty("size_bytes").GetUInt64();
        var pageCount = root.GetProperty("page_count").GetUInt32();
        var title = root.TryGetProperty("title", out var tProp) && tProp.ValueKind != JsonValueKind.Null ? tProp.GetString() : null;
        var author = root.TryGetProperty("author", out var aProp) && aProp.ValueKind != JsonValueKind.Null ? aProp.GetString() : null;
        var engineId = root.GetProperty("engine_identifier").GetString()!;

        _activeSessions.Add(sessionId);

        return new DocumentSessionSnapshot(
            SessionId: sessionId,
            DocumentId: documentId,
            AccessOrigin: origin,
            SizeBytes: sizeBytes,
            PageCount: pageCount,
            Title: title,
            Author: author,
            EngineIdentifier: engineId
        );
    }

    public PageGeometry GetPageGeometry(string sessionId, uint pageIndex)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);

        var rc = NativeMethods.VocumentGetPageGeometry(sessionId, pageIndex, out var widthMilli, out var heightMilli);
        if (rc != 0)
        {
            throw new InvalidOperationException($"Failed to get page geometry for page {pageIndex} (error code: {rc})");
        }

        return new PageGeometry(pageIndex, widthMilli, heightMilli);
    }

    public ViewportTile RenderViewport(string sessionId, uint pageIndex, uint scaleMilli)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);

        var geom = GetPageGeometry(sessionId, pageIndex);
        var widthPt = geom.WidthMilliPt / 1000f;
        var heightPt = geom.HeightMilliPt / 1000f;
        var widthPx = (uint)Math.Max(1, Math.Round(widthPt * (scaleMilli / 1000f)));
        var heightPx = (uint)Math.Max(1, Math.Round(heightPt * (scaleMilli / 1000f)));

        var buffer = new byte[widthPx * heightPx * 4];
        var rc = NativeMethods.VocumentRenderPageTile(sessionId, pageIndex, scaleMilli, 0, 0, widthPx, heightPx, buffer, (nuint)buffer.Length);
        if (rc != 0)
        {
            throw new InvalidOperationException($"Failed to render page viewport tile (error code: {rc})");
        }

        return new ViewportTile(pageIndex, widthPx, heightPx, buffer);
    }

    public IReadOnlyList<SearchHit> SearchExact(string sessionId, string query)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);
        ArgumentException.ThrowIfNullOrEmpty(query);

        var jsonStr = MarshalAndFree(NativeMethods.VocumentSearchExact(sessionId, query));
        var hits = new List<SearchHit>();
        if (string.IsNullOrEmpty(jsonStr)) return hits;

        using var doc = JsonDocument.Parse(jsonStr);
        if (doc.RootElement.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                var pIdx = item.GetProperty("page_index").GetUInt32();
                var label = item.GetProperty("logical_page_label").GetString() ?? "";
                var snippet = item.GetProperty("snippet").GetString() ?? "";
                var count = item.GetProperty("match_count").GetUInt32();
                hits.Add(new SearchHit(pIdx, label, snippet, count));
            }
        }
        return hits;
    }

    public TextSelection SelectPageText(string sessionId, uint pageIndex)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);

        var jsonStr = MarshalAndFree(NativeMethods.VocumentSelectPageText(sessionId, pageIndex));
        if (string.IsNullOrEmpty(jsonStr))
        {
            throw new InvalidOperationException("Failed to select page text");
        }

        using var doc = JsonDocument.Parse(jsonStr);
        var root = doc.RootElement;
        if (root.TryGetProperty("error", out var errProp))
        {
            throw new InvalidOperationException($"Failed to select page text: {errProp.GetString()}");
        }

        var raw = root.GetProperty("raw_text").GetString() ?? "";
        var cleaned = root.GetProperty("cleaned_text").GetString() ?? "";

        return new TextSelection(pageIndex, raw, cleaned);
    }

    public HighlightAnnotation AddHighlight(string sessionId, uint pageIndex, string colorHex)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);

        var jsonStr = MarshalAndFree(NativeMethods.VocumentAddHighlight(sessionId, pageIndex, colorHex));
        if (string.IsNullOrEmpty(jsonStr))
        {
            throw new InvalidOperationException("Failed to add highlight annotation");
        }

        using var doc = JsonDocument.Parse(jsonStr);
        var root = doc.RootElement;
        if (root.TryGetProperty("error", out var errProp))
        {
            throw new InvalidOperationException($"Failed to add highlight: {errProp.GetString()}");
        }

        var annotId = root.GetProperty("annotation_id").GetString()!;
        var color = root.GetProperty("color_hex").GetString()!;
        var author = root.TryGetProperty("author", out var aProp) ? aProp.GetString() : null;

        return new HighlightAnnotation(annotId, pageIndex, color, author);
    }

    public uint RotatePages(string sessionId, IReadOnlyList<uint> pages, int rotationDeltaDegrees)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);
        var arr = new uint[pages.Count];
        for (int i = 0; i < pages.Count; i++) arr[i] = pages[i];

        var rc = NativeMethods.VocumentRotatePages(sessionId, arr, (nuint)arr.Length, rotationDeltaDegrees);
        if (rc < 0)
        {
            throw new InvalidOperationException($"Failed to rotate pages (error code: {rc})");
        }
        return (uint)rc;
    }

    public bool Undo(string sessionId)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);
        return NativeMethods.VocumentUndo(sessionId) == 1;
    }

    public bool Redo(string sessionId)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);
        return NativeMethods.VocumentRedo(sessionId) == 1;
    }

    public void SaveSession(string sessionId, string destinationPath)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);
        ArgumentException.ThrowIfNullOrEmpty(destinationPath);

        var jsonStr = MarshalAndFree(NativeMethods.VocumentSaveSession(sessionId, destinationPath));
        if (string.IsNullOrEmpty(jsonStr))
        {
            throw new InvalidOperationException("Failed to execute save session");
        }

        using var doc = JsonDocument.Parse(jsonStr);
        var root = doc.RootElement;
        if (root.TryGetProperty("error", out var errProp))
        {
            throw new InvalidOperationException($"Save failed: {errProp.GetString()}");
        }
    }

    public bool CheckRecoveryCheckpoint(string sessionId)
    {
        ArgumentException.ThrowIfNullOrEmpty(sessionId);
        return NativeMethods.VocumentCheckRecovery(sessionId) == 1;
    }

    public void CloseSession(string sessionId)
    {
        if (string.IsNullOrEmpty(sessionId)) return;
        NativeMethods.VocumentCloseSession(sessionId);
        _activeSessions.Remove(sessionId);
    }

    public void Dispose()
    {
        foreach (var session in _activeSessions.ToArray())
        {
            CloseSession(session);
        }
    }
}
