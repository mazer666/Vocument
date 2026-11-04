using System;
using System.IO;

namespace Vocument.Windows.Platform;

/// <summary>
/// Scoped Windows file access broker (PLATFORM-003, FILE-001).
/// Interfaces with Windows Storage and Common Item Dialog APIs.
/// </summary>
public interface IWindowsFileBroker
{
    string GenerateCollisionFreePath(string destinationDirectory, string desiredFileName);
    bool VerifyAccessPermission(string filePath, bool writeAccess);
    byte[] ReadDocumentBytes(string filePath);
    void WriteDocumentBytes(string filePath, byte[] data);
}

public sealed class WindowsFileBroker : IWindowsFileBroker
{
    public string GenerateCollisionFreePath(string destinationDirectory, string desiredFileName)
    {
        ArgumentException.ThrowIfNullOrEmpty(destinationDirectory);
        ArgumentException.ThrowIfNullOrEmpty(desiredFileName);

        var candidate = Path.Combine(destinationDirectory, desiredFileName);
        if (!File.Exists(candidate))
        {
            return candidate;
        }

        var stem = Path.GetFileNameWithoutExtension(desiredFileName);
        var ext = Path.GetExtension(desiredFileName);

        for (var index = 1; index <= 10000; index++)
        {
            var nextCandidate = Path.Combine(destinationDirectory, $"{stem} ({index}){ext}");
            if (!File.Exists(nextCandidate))
            {
                return nextCandidate;
            }
        }

        throw new IOException($"Could not determine unique collision-free file path in {destinationDirectory}");
    }

    public bool VerifyAccessPermission(string filePath, bool writeAccess)
    {
        if (string.IsNullOrEmpty(filePath))
        {
            return false;
        }

        try
        {
            if (File.Exists(filePath))
            {
                var access = writeAccess ? FileAccess.ReadWrite : FileAccess.Read;
                using var stream = new FileStream(filePath, FileMode.Open, access, FileShare.ReadWrite);
                return true;
            }

            var dir = Path.GetDirectoryName(filePath);
            return !string.IsNullOrEmpty(dir) && Directory.Exists(dir);
        }
        catch
        {
            return false;
        }
    }

    public byte[] ReadDocumentBytes(string filePath)
    {
        ArgumentException.ThrowIfNullOrEmpty(filePath);
        return File.ReadAllBytes(filePath);
    }

    public void WriteDocumentBytes(string filePath, byte[] data)
    {
        ArgumentException.ThrowIfNullOrEmpty(filePath);
        ArgumentNullException.ThrowIfNull(data);

        var directory = Path.GetDirectoryName(filePath);
        if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
        {
            Directory.CreateDirectory(directory);
        }

        // Atomic write via temp file
        var tempFile = Path.Combine(directory ?? ".", $"{Path.GetFileName(filePath)}.{Guid.NewGuid():N}.tmp");
        File.WriteAllBytes(tempFile, data);
        File.Move(tempFile, filePath, overwrite: true);
    }
}
