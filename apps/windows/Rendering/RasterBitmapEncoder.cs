using System;

namespace Vocument.Windows.Rendering;

/// <summary>
/// Converts the raw RGBA page rasters produced by the Rust core into a bitmap that
/// the Windows imaging stack can decode (32 bit BMP, bottom-up, BGRA).
/// <para>
/// The conversion lives in the shell because it is a presentation concern only: the
/// core owns the rasterisation (MuPDF), the shell owns the pixels on the screen.
/// </para>
/// </summary>
public static class RasterBitmapEncoder
{
    private const int FileHeaderSize = 14;
    private const int InfoHeaderSize = 40;
    private const int PixelDataOffset = FileHeaderSize + InfoHeaderSize;

    /// <summary>
    /// Encodes an RGRA pixel buffer (red, green, blue, alpha; one byte per channel)
    /// into an uncompressed 32 bit BMP file.
    /// </summary>
    public static byte[] EncodeBmp(byte[] rgbaPixels, int width, int height)
    {
        ArgumentNullException.ThrowIfNull(rgbaPixels);
        if (width <= 0 || height <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(width), "The raster size must be positive.");
        }

        var expectedLength = checked(width * height * 4);
        if (rgbaPixels.Length < expectedLength)
        {
            throw new ArgumentException(
                $"The raster buffer holds {rgbaPixels.Length} bytes but {expectedLength} bytes are required for {width}x{height} pixels.",
                nameof(rgbaPixels));
        }

        var pixelDataSize = checked(width * height * 4);
        var fileSize = checked(PixelDataOffset + pixelDataSize);
        var bmp = new byte[fileSize];

        // BITMAPFILEHEADER
        bmp[0] = (byte)'B';
        bmp[1] = (byte)'M';
        WriteInt32(bmp, 2, fileSize);
        WriteInt32(bmp, 10, PixelDataOffset);

        // BITMAPINFOHEADER
        WriteInt32(bmp, 14, InfoHeaderSize);
        WriteInt32(bmp, 18, width);
        WriteInt32(bmp, 22, height);
        WriteInt16(bmp, 26, 1);              // planes
        WriteInt16(bmp, 28, 32);             // bits per pixel
        WriteInt32(bmp, 30, 0);              // BI_RGB, no compression
        WriteInt32(bmp, 34, pixelDataSize);
        WriteInt32(bmp, 38, 2835);           // horizontal pixels per metre
        WriteInt32(bmp, 42, 2835);           // vertical pixels per metre

        // Pixel data, bottom-up, BGRA.
        var rowSize = checked(width * 4);
        for (var row = 0; row < height; row++)
        {
            var sourceRow = (height - 1 - row) * rowSize;
            var targetRow = PixelDataOffset + row * rowSize;
            for (var column = 0; column < width; column++)
            {
                var source = sourceRow + column * 4;
                var target = targetRow + column * 4;
                bmp[target] = rgbaPixels[source + 2];      // blue
                bmp[target + 1] = rgbaPixels[source + 1];  // green
                bmp[target + 2] = rgbaPixels[source];      // red
                bmp[target + 3] = 0xFF;                    // opaque
            }
        }

        return bmp;
    }

    private static void WriteInt32(byte[] buffer, int offset, int value)
    {
        buffer[offset] = (byte)(value & 0xFF);
        buffer[offset + 1] = (byte)((value >> 8) & 0xFF);
        buffer[offset + 2] = (byte)((value >> 16) & 0xFF);
        buffer[offset + 3] = (byte)((value >> 24) & 0xFF);
    }

    private static void WriteInt16(byte[] buffer, int offset, int value)
    {
        buffer[offset] = (byte)(value & 0xFF);
        buffer[offset + 1] = (byte)((value >> 8) & 0xFF);
    }
}
