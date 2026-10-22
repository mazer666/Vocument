// The conversion of a rendered tile into a `CGImage`.
//
// This is the only part of the kit that needs CoreGraphics. The file is
// guarded as a whole, so that the kit — the data path of the application —
// can be **compiled and tested** on a platform that does not have
// CoreGraphics (Linux). The behaviour on macOS is unchanged: the same
// functions, the same results.
//
// The functions are members of the same type as before, so that the callers
// do not change: `DocumentCoreClient.makeCGImage(from:)`.

import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
import VocumentFFI

extension DocumentCoreClient {
    /// Converts an immutable `FfiRenderedTile` RGBA8 buffer into a native `CGImage` (`ARCHITECTURE.md` §10).
    public static func makeCGImage(from tile: FfiRenderedTile) -> CGImage? {
        makeCGImageFromRgba8(
            width: Int(tile.widthPx),
            height: Int(tile.heightPx),
            stride: Int(tile.strideBytes),
            rgbaBytes: tile.rgbaBytes
        )
    }

    /// Converts an immutable `FfiRenderedThumbnail` RGBA8 buffer into a native `CGImage`.
    public static func makeCGImage(from thumbnail: FfiRenderedThumbnail) -> CGImage? {
        makeCGImageFromRgba8(
            width: Int(thumbnail.widthPx),
            height: Int(thumbnail.heightPx),
            stride: Int(thumbnail.strideBytes),
            rgbaBytes: thumbnail.rgbaBytes
        )
    }

    private static func makeCGImageFromRgba8(
        width: Int,
        height: Int,
        stride: Int,
        rgbaBytes: Data
    ) -> CGImage? {
        guard width > 0, height > 0, rgbaBytes.count >= stride * height else {
            return nil
        }
        let data = rgbaBytes as CFData
        guard let provider = CGDataProvider(data: data) else {
            return nil
        }
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue)
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: stride,
            space: colorSpace,
            bitmapInfo: bitmapInfo,
            provider: provider,
            decode: nil,
            shouldInterpolate: true,
            intent: .defaultIntent
        )
    }
}
#endif
