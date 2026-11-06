using System;
using System.Collections.Generic;
using System.Text;

namespace Vocument.Windows.Comparison;

/// <summary>Kind of a segment of a text comparison.</summary>
public enum TextDiffKind
{
    Equal,
    Inserted,
    Removed
}

/// <summary>A contiguous run of words with its comparison kind.</summary>
public sealed record TextDiffSegment(TextDiffKind Kind, string Text);

/// <summary>
/// Word level comparison of two texts. The engine is deterministic and independent
/// of the UI so that it can be verified by unit tests.
/// </summary>
public static class TextDiffEngine
{
    private const int MaxComparedWords = 4000;

    /// <summary>
    /// Computes the differences between two texts on word level. Words that exist in
    /// both texts are reported as <see cref="TextDiffKind.Equal"/>, words that only
    /// exist in the left text as <see cref="TextDiffKind.Removed"/> and words that only
    /// exist in the right text as <see cref="TextDiffKind.Inserted"/>.
    /// </summary>
    public static IReadOnlyList<TextDiffSegment> Compute(string leftText, string rightText)
    {
        var leftWords = SplitWords(leftText);
        var rightWords = SplitWords(rightText);

        if (leftWords.Length > MaxComparedWords || rightWords.Length > MaxComparedWords)
        {
            return ComputeByBlocks(leftWords, rightWords);
        }

        var table = BuildLongestCommonSubsequenceTable(leftWords, rightWords);
        return ExtractSegments(leftWords, rightWords, table);
    }

    /// <summary>
    /// Counts the words that were added or removed between two texts.
    /// </summary>
    public static (int Removed, int Inserted) CountChanges(string leftText, string rightText)
    {
        var removed = 0;
        var inserted = 0;

        foreach (var segment in Compute(leftText, rightText))
        {
            var wordCount = segment.Text.Split(' ', StringSplitOptions.RemoveEmptyEntries).Length;
            if (segment.Kind == TextDiffKind.Removed) removed += wordCount;
            if (segment.Kind == TextDiffKind.Inserted) inserted += wordCount;
        }

        return (removed, inserted);
    }

    private static string[] SplitWords(string? text)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            return Array.Empty<string>();
        }

        return text.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
    }

    private static int[,] BuildLongestCommonSubsequenceTable(IReadOnlyList<string> left, IReadOnlyList<string> right)
    {
        var table = new int[left.Count + 1, right.Count + 1];

        for (var i = left.Count - 1; i >= 0; i--)
        {
            for (var j = right.Count - 1; j >= 0; j--)
            {
                table[i, j] = string.Equals(left[i], right[j], StringComparison.Ordinal)
                    ? table[i + 1, j + 1] + 1
                    : Math.Max(table[i + 1, j], table[i, j + 1]);
            }
        }

        return table;
    }

    private static List<TextDiffSegment> ExtractSegments(
        IReadOnlyList<string> left,
        IReadOnlyList<string> right,
        int[,] table)
    {
        var segments = new List<TextDiffSegment>();
        var i = 0;
        var j = 0;

        while (i < left.Count && j < right.Count)
        {
            if (string.Equals(left[i], right[j], StringComparison.Ordinal))
            {
                Append(segments, TextDiffKind.Equal, left[i]);
                i++;
                j++;
            }
            else if (table[i + 1, j] >= table[i, j + 1])
            {
                Append(segments, TextDiffKind.Removed, left[i]);
                i++;
            }
            else
            {
                Append(segments, TextDiffKind.Inserted, right[j]);
                j++;
            }
        }

        while (i < left.Count)
        {
            Append(segments, TextDiffKind.Removed, left[i]);
            i++;
        }

        while (j < right.Count)
        {
            Append(segments, TextDiffKind.Inserted, right[j]);
            j++;
        }

        return segments;
    }

    private static List<TextDiffSegment> ComputeByBlocks(string[] left, string[] right)
    {
        var segments = new List<TextDiffSegment>();
        var commonPrefix = 0;
        while (commonPrefix < left.Length && commonPrefix < right.Length &&
               string.Equals(left[commonPrefix], right[commonPrefix], StringComparison.Ordinal))
        {
            commonPrefix++;
        }

        var commonSuffix = 0;
        while (commonSuffix < left.Length - commonPrefix &&
               commonSuffix < right.Length - commonPrefix &&
               string.Equals(left[left.Length - 1 - commonSuffix], right[right.Length - 1 - commonSuffix], StringComparison.Ordinal))
        {
            commonSuffix++;
        }

        if (commonPrefix > 0)
        {
            segments.Add(new TextDiffSegment(TextDiffKind.Equal, string.Join(' ', left[..commonPrefix])));
        }

        if (left.Length > commonPrefix + commonSuffix)
        {
            segments.Add(new TextDiffSegment(TextDiffKind.Removed, string.Join(' ', left[commonPrefix..(left.Length - commonSuffix)])));
        }

        if (right.Length > commonPrefix + commonSuffix)
        {
            segments.Add(new TextDiffSegment(TextDiffKind.Inserted, string.Join(' ', right[commonPrefix..(right.Length - commonSuffix)])));
        }

        if (commonSuffix > 0)
        {
            segments.Add(new TextDiffSegment(TextDiffKind.Equal, string.Join(' ', left[(left.Length - commonSuffix)..])));
        }

        return segments;
    }

    private static void Append(List<TextDiffSegment> segments, TextDiffKind kind, string word)
    {
        if (segments.Count > 0 && segments[^1].Kind == kind)
        {
            segments[^1] = new TextDiffSegment(kind, segments[^1].Text + " " + word);
            return;
        }

        segments.Add(new TextDiffSegment(kind, word));
    }
}

/// <summary>Result of a pixel comparison of two rendered pages.</summary>
public sealed record PixelDiffResult(
    int ChangedPixels,
    int TotalPixels,
    double ChangedRatio,
    int MinX,
    int MinY,
    int MaxX,
    int MaxY,
    byte[] OverlayRgba);

/// <summary>
/// Compares two rendered page rasters. Both buffers come from the real MuPDF
/// rasteriser of the Rust core; the engine only compares them.
/// </summary>
public static class PixelDiffEngine
{
    /// <summary>
    /// Compares two RGBA buffers of identical size. Pixels whose channel values differ
    /// by more than <paramref name="channelTolerance"/> count as changed. The overlay
    /// keeps the unchanged content dimmed and marks changed pixels.
    /// </summary>
    public static PixelDiffResult Compare(
        byte[] leftRgba,
        byte[] rightRgba,
        int width,
        int height,
        int channelTolerance = 12)
    {
        ArgumentNullException.ThrowIfNull(leftRgba);
        ArgumentNullException.ThrowIfNull(rightRgba);

        var expected = checked(width * height * 4);
        if (leftRgba.Length < expected || rightRgba.Length < expected)
        {
            throw new ArgumentException("The raster buffers do not match the given size.");
        }

        var overlay = new byte[expected];
        var changedPixels = 0;
        var minX = width;
        var minY = height;
        var maxX = -1;
        var maxY = -1;

        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                var offset = (y * width + x) * 4;
                var isChanged =
                    Math.Abs(leftRgba[offset] - rightRgba[offset]) > channelTolerance ||
                    Math.Abs(leftRgba[offset + 1] - rightRgba[offset + 1]) > channelTolerance ||
                    Math.Abs(leftRgba[offset + 2] - rightRgba[offset + 2]) > channelTolerance;

                if (isChanged)
                {
                    changedPixels++;
                    if (x < minX) minX = x;
                    if (y < minY) minY = y;
                    if (x > maxX) maxX = x;
                    if (y > maxY) maxY = y;

                    overlay[offset] = 0xFF;
                    overlay[offset + 1] = 0x00;
                    overlay[offset + 2] = 0xC8;
                    overlay[offset + 3] = 0xFF;
                }
                else
                {
                    var luminance = (byte)((leftRgba[offset] * 299 + leftRgba[offset + 1] * 587 + leftRgba[offset + 2] * 114) / 1000);
                    overlay[offset] = (byte)((luminance * 3) / 4);
                    overlay[offset + 1] = (byte)((luminance * 3) / 4);
                    overlay[offset + 2] = (byte)((luminance * 3) / 4);
                    overlay[offset + 3] = 0xFF;
                }
            }
        }

        var totalPixels = checked(width * height);
        var ratio = totalPixels == 0 ? 0d : (double)changedPixels / totalPixels;

        return new PixelDiffResult(
            changedPixels,
            totalPixels,
            ratio,
            minX,
            minY,
            maxX,
            maxY,
            overlay);
    }
}
