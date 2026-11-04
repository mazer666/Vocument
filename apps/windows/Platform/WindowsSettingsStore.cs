using System;
using System.Collections.Concurrent;
using System.IO;
using System.Text.Json;

namespace Vocument.Windows.Platform;

/// <summary>
/// Persistent Windows settings store (PLATFORM-003, CONF-001).
/// Backed by AppData Local JSON configuration.
/// </summary>
public interface IWindowsSettingsStore
{
    string? GetSetting(string key);
    void SetSetting(string key, string value);
    bool GetBooleanSetting(string key, bool defaultValue);
    void SetBooleanSetting(string key, bool value);
    void Save();
}

public sealed class WindowsSettingsStore : IWindowsSettingsStore
{
    private readonly string _settingsFilePath;
    private readonly ConcurrentDictionary<string, string> _values = new();

    public WindowsSettingsStore(string? customPath = null)
    {
        if (!string.IsNullOrEmpty(customPath))
        {
            _settingsFilePath = customPath;
        }
        else
        {
            var appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            var vocumentDir = Path.Combine(appData, "Vocument");
            _settingsFilePath = Path.Combine(vocumentDir, "settings.json");
        }

        Load();
    }

    private void Load()
    {
        if (!File.Exists(_settingsFilePath))
        {
            // Defaults (NET-001 offline, LOG-001 telemetry disabled, de-AT locale)
            _values["offline_default"] = "true";
            _values["telemetry_enabled"] = "false";
            _values["locale"] = "de-AT";
            return;
        }

        try
        {
            var json = File.ReadAllText(_settingsFilePath);
            var dict = JsonSerializer.Deserialize<Dictionary<string, string>>(json);
            if (dict != null)
            {
                foreach (var (k, v) in dict)
                {
                    _values[k] = v;
                }
            }
        }
        catch
        {
            // Fall back to defaults on corrupt file
            _values["offline_default"] = "true";
            _values["telemetry_enabled"] = "false";
            _values["locale"] = "de-AT";
        }
    }

    public void Save()
    {
        var dir = Path.GetDirectoryName(_settingsFilePath);
        if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
        {
            Directory.CreateDirectory(dir);
        }

        var json = JsonSerializer.Serialize(_values, new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(_settingsFilePath, json);
    }

    public string? GetSetting(string key)
    {
        return _values.TryGetValue(key, out var val) ? val : null;
    }

    public void SetSetting(string key, string value)
    {
        _values[key] = value;
    }

    public bool GetBooleanSetting(string key, bool defaultValue)
    {
        if (_values.TryGetValue(key, out var val) && bool.TryParse(val, out var parsed))
        {
            return parsed;
        }
        return defaultValue;
    }

    public void SetBooleanSetting(string key, bool value)
    {
        _values[key] = value ? "true" : "false";
    }
}
