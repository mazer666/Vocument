using System;
using System.Collections.Generic;
using System.IO;

namespace Vocument.Windows.Platform;

/// <summary>
/// Font provider for Windows (PLATFORM-003, FONT-001).
/// </summary>
public interface IWindowsFontProvider
{
    IReadOnlyList<string> EnumerateAvailableFontFamilies();
    string? ResolveFontPath(string familyName, bool isBold = false, bool isItalic = false);
}

public sealed class WindowsFontProvider : IWindowsFontProvider
{
    private readonly string _fontsDirectory;

    public WindowsFontProvider(string? customFontsDirectory = null)
    {
        if (!string.IsNullOrEmpty(customFontsDirectory))
        {
            _fontsDirectory = customFontsDirectory;
        }
        else
        {
            var winDir = Environment.GetFolderPath(Environment.SpecialFolder.Windows);
            _fontsDirectory = Path.Combine(winDir, "Fonts");
        }
    }

    public IReadOnlyList<string> EnumerateAvailableFontFamilies()
    {
        var result = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
        {
            "Segoe UI",
            "Arial",
            "Calibri",
            "Times New Roman",
            "Consolas",
            "Droid Sans Fallback",
            "Noto Naskh Arabic"
        };

        if (Directory.Exists(_fontsDirectory))
        {
            try
            {
                foreach (var file in Directory.EnumerateFiles(_fontsDirectory, "*.*"))
                {
                    var ext = Path.GetExtension(file).ToLowerInvariant();
                    if (ext is ".ttf" or ".otf" or ".ttc")
                    {
                        var name = Path.GetFileNameWithoutExtension(file);
                        result.Add(name);
                    }
                }
            }
            catch
            {
                // Fall back to well-known list
            }
        }

        return new List<string>(result);
    }

    public string? ResolveFontPath(string familyName, bool isBold = false, bool isItalic = false)
    {
        if (Directory.Exists(_fontsDirectory))
        {
            var candidate = Path.Combine(_fontsDirectory, $"{familyName}.ttf");
            if (File.Exists(candidate))
            {
                return candidate;
            }

            candidate = Path.Combine(_fontsDirectory, $"{familyName}.otf");
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        return null;
    }
}
