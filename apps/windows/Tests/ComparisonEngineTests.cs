using System;
using System.Linq;
using Vocument.Windows.Comparison;
using Vocument.Windows.Rendering;
using Xunit;

namespace Vocument.Windows.Tests;

/// <summary>
/// Tests for the comparison engines and the raster encoder of the Windows shell.
/// The engines are platform independent, so these tests run on every host that can
/// build the project, not only on Windows.
/// </summary>
public class ComparisonEngineTests
{
    [Fact]
    public void TextDiff_IdentifiesRemovedAndInsertedWords()
    {
        var segments = TextDiffEngine.Compute("alpha beta gamma", "alpha delta gamma");

        Assert.Contains(segments, segment => segment.Kind == TextDiffKind.Removed && segment.Text.Contains("beta"));
        Assert.Contains(segments, segment => segment.Kind == TextDiffKind.Inserted && segment.Text.Contains("delta"));
        Assert.Contains(segments, segment => segment.Kind == TextDiffKind.Equal && segment.Text.Contains("alpha"));
    }

    [Fact]
    public void TextDiff_OfIdenticalTextsHasNoChanges()
    {
        var (removed, inserted) = TextDiffEngine.CountChanges("same words here", "same words here");

        Assert.Equal(0, removed);
        Assert.Equal(0, inserted);
    }

    [Fact]
    public void TextDiff_CountsChangedWords()
    {
        var (removed, inserted) = TextDiffEngine.CountChanges("alpha beta gamma", "alpha delta gamma");

        Assert.Equal(1, removed);
        Assert.Equal(1, inserted);
    }

    [Fact]
    public void TextDiff_OfEmptyTextsIsEmpty()
    {
        var segments = TextDiffEngine.Compute(string.Empty, "   ");

        Assert.Empty(segments);
    }

    [Fact]
    public void PixelDiff_ReportsChangedPixelsAndBoundingBox()
    {
        var width = 4;
        var height = 4;
        var left = new byte[width * height * 4];
        var right = new byte[width * height * 4];

        for (var i = 0; i < left.Length; i += 4)
        {
            left[i] = 255;
            left[i + 1] = 255;
            left[i + 2] = 255;
            left[i + 3] = 255;

            right[i] = 255;
            right[i + 1] = 255;
            right[i + 2] = 255;
            right[i + 3] = 255;
        }

        // Change one pixel at (1, 2).
        var offset = (2 * width + 1) * 4;
        right[offset] = 0;
        right[offset + 1] = 0;
        right[offset + 2] = 0;

        var result = PixelDiffEngine.Compare(left, right, width, height);

        Assert.Equal(1, result.ChangedPixels);
        Assert.Equal(16, result.TotalPixels);
        Assert.Equal(1, result.MinX);
        Assert.Equal(2, result.MinY);
        Assert.Equal(1, result.MaxX);
        Assert.Equal(2, result.MaxY);
        Assert.Equal(left.Length, result.OverlayRgba.Length);
    }

    [Fact]
    public void PixelDiff_OfIdenticalRastersHasNoChanges()
    {
        var pixels = new byte[2 * 2 * 4];
        Array.Fill(pixels, (byte)200);

        var result = PixelDiffEngine.Compare(pixels, pixels, 2, 2);

        Assert.Equal(0, result.ChangedPixels);
        Assert.Equal(0d, result.ChangedRatio);
    }

    [Fact]
    public void RasterEncoder_ProducesAValidBmpHeader()
    {
        var pixels = new byte[2 * 2 * 4];
        Array.Fill(pixels, (byte)128);

        var bmp = RasterBitmapEncoder.EncodeBmp(pixels, 2, 2);

        Assert.Equal((byte)'B', bmp[0]);
        Assert.Equal((byte)'M', bmp[1]);
        Assert.Equal(bmp.Length, BitConverter.ToInt32(bmp, 2));
        Assert.Equal(54, BitConverter.ToInt32(bmp, 10));
        Assert.Equal(40, BitConverter.ToInt32(bmp, 14));
        Assert.Equal(2, BitConverter.ToInt32(bmp, 18));
        Assert.Equal(2, BitConverter.ToInt32(bmp, 22));
        Assert.Equal(32, BitConverter.ToInt16(bmp, 28));
    }

    [Fact]
    public void RasterEncoder_ConvertsRgbaToBgraAndFlipsRows()
    {
        var pixels = new byte[]
        {
            10, 20, 30, 255,   // top left pixel (red, green, blue, alpha)
            40, 50, 60, 255,   // top right pixel
            70, 80, 90, 255,   // bottom left pixel
            100, 110, 120, 255 // bottom right pixel
        };

        var bmp = RasterBitmapEncoder.EncodeBmp(pixels, 2, 2);
        var firstPixel = 54;

        // BMP rows are stored bottom-up and each pixel is stored as blue, green, red,
        // so the last source row comes first and its channels are reversed.
        Assert.Equal(90, bmp[firstPixel]);
        Assert.Equal(80, bmp[firstPixel + 1]);
        Assert.Equal(70, bmp[firstPixel + 2]);
        Assert.Equal(255, bmp[firstPixel + 3]);
        Assert.Equal(120, bmp[firstPixel + 4]);
        Assert.Equal(30, bmp[firstPixel + 8]);
        Assert.Equal(20, bmp[firstPixel + 9]);
        Assert.Equal(10, bmp[firstPixel + 10]);
    }

    [Fact]
    public void RasterEncoder_RejectsATooSmallBuffer()
    {
        Assert.Throws<ArgumentException>(() => RasterBitmapEncoder.EncodeBmp(new byte[8], 4, 4));
    }
}
