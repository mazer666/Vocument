using System;
using System.Collections.Generic;
using Vocument.Windows.Comparison;
using Vocument.Windows.Native;

namespace Vocument.Windows.Views;

/// <summary>A single difference that the comparison found.</summary>
public sealed record ComparisonEntry(string Page, string Kind, string Detail);

/// <summary>The outcome of a comparison of two documents.</summary>
public sealed record ComparisonResult(
    IReadOnlyList<ComparisonEntry> Entries,
    int RemovedWords,
    int InsertedWords,
    int ChangedPixels,
    double ChangedPixelRatio,
    int Width,
    int Height,
    byte[] OverlayRgba);

/// <summary>
/// The comparison actions of the document compare surface, independent of the WinUI 3
/// widget tree. The comparison runs on data that the core produced: the text of every
/// page is extracted by the core and the page rasters are rendered by the engine of
/// the core. The shell only compares those results.
/// </summary>
public sealed class CompareWorkflow
{
    private const uint DefaultZoomMilli = 700;

    private readonly DocumentCoreClient _client;

    public CompareWorkflow(DocumentCoreClient client)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
    }

    /// <summary>
    /// Compares two sessions page by page and returns the detected differences
    /// together with the diff overlay of the first page.
    /// </summary>
    public ComparisonResult Compare(
        DocumentSessionSnapshot original,
        DocumentSessionSnapshot modified,
        uint zoomMilli = DefaultZoomMilli)
    {
        ArgumentNullException.ThrowIfNull(original);
        ArgumentNullException.ThrowIfNull(modified);

        var entries = new List<ComparisonEntry>();
        var removedWords = 0;
        var insertedWords = 0;
        var changedPixels = 0;
        var changedPixelRatio = 0d;
        var overlay = Array.Empty<byte>();
        var overlayWidth = 0;
        var overlayHeight = 0;

        if (original.PageCount != modified.PageCount)
        {
            entries.Add(new ComparisonEntry(
                "Document",
                "Geometry",
                $"Page counts differ ({original.PageCount} against {modified.PageCount})"));
        }

        var pageCount = Math.Min(original.PageCount, modified.PageCount);
        for (uint pageIndex = 0; pageIndex < pageCount; pageIndex++)
        {
            var originalText = _client.SelectPageText(original.SessionId, pageIndex);
            var modifiedText = _client.SelectPageText(modified.SessionId, pageIndex);

            var (removed, inserted) = TextDiffEngine.CountChanges(originalText.CleanedText, modifiedText.CleanedText);
            removedWords += removed;
            insertedWords += inserted;

            if (removed > 0 || inserted > 0)
            {
                entries.Add(new ComparisonEntry(
                    $"Page {pageIndex + 1}",
                    "Text",
                    $"{removed} word(s) removed, {inserted} word(s) inserted"));
            }

            var originalRaster = _client.RenderViewport(original.SessionId, pageIndex, zoomMilli);
            var modifiedRaster = _client.RenderViewport(modified.SessionId, pageIndex, zoomMilli);

            if (originalRaster.WidthPx != modifiedRaster.WidthPx || originalRaster.HeightPx != modifiedRaster.HeightPx)
            {
                entries.Add(new ComparisonEntry(
                    $"Page {pageIndex + 1}",
                    "Geometry",
                    $"Page sizes differ ({originalRaster.WidthPx}x{originalRaster.HeightPx} against {modifiedRaster.WidthPx}x{modifiedRaster.HeightPx})"));
                continue;
            }

            var pixelDiff = PixelDiffEngine.Compare(
                originalRaster.ArgbPixels,
                modifiedRaster.ArgbPixels,
                (int)originalRaster.WidthPx,
                (int)originalRaster.HeightPx);

            if (pixelDiff.ChangedPixels > 0)
            {
                changedPixels += pixelDiff.ChangedPixels;
                changedPixelRatio = Math.Max(changedPixelRatio, pixelDiff.ChangedRatio);

                entries.Add(new ComparisonEntry(
                    $"Page {pageIndex + 1}",
                    "Visual",
                    $"{pixelDiff.ChangedPixels} pixel(s) differ ({pixelDiff.ChangedRatio:P2}) in the region [{pixelDiff.MinX},{pixelDiff.MinY}]–[{pixelDiff.MaxX},{pixelDiff.MaxY}]"));

                if (overlay.Length == 0)
                {
                    overlay = pixelDiff.OverlayRgba;
                    overlayWidth = (int)originalRaster.WidthPx;
                    overlayHeight = (int)originalRaster.HeightPx;
                }
            }
        }

        return new ComparisonResult(
            entries,
            removedWords,
            insertedWords,
            changedPixels,
            changedPixelRatio,
            overlayWidth,
            overlayHeight,
            overlay);
    }
}
