import SwiftUI
import CoreGraphics
import VocumentMacKit
import VocumentFFI

/// Native macOS `DocumentCanvas` view rendering RGBA8 tiles from `DocumentCore` (`ARCHITECTURE.md` §10, `ROADMAP.md` M0.2–M0.3).
/// Displays both logical page label and physical page number (`PAGELABEL-002`).
struct DocumentCanvasView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Button("◀ Prev Page") {
                    coordinator.stepPage(by: -1)
                }
                .disabled(!coordinator.canStepPage(by: -1))

                Text("Label \(coordinator.currentLogicalLabel) (Page \(coordinator.currentPageIndex + 1) / \(coordinator.pageCount))")
                    .monospacedDigit()
                    .bold()

                Button("Next Page ▶") {
                    coordinator.stepPage(by: 1)
                }
                .disabled(!coordinator.canStepPage(by: 1))

                Divider().frame(height: 18)

                Button("Zoom −") {
                    coordinator.adjustZoom(deltaMilli: -250)
                }
                .disabled(coordinator.scaleMilli <= 500)

                Text("\(Int(coordinator.scaleMilli / 10))%")
                    .monospacedDigit()
                    .frame(width: 48, alignment: .center)

                Button("Zoom +") {
                    coordinator.adjustZoom(deltaMilli: 250)
                }
                .disabled(coordinator.scaleMilli >= 3000)

                Spacer()

                Menu("Memory Pressure") {
                    Button("Normal (100% Budget)") {
                        coordinator.applyMemoryPressure(.normal)
                    }
                    Button("Warning (50% Trim)") {
                        coordinator.applyMemoryPressure(.warning)
                    }
                    Button("Critical (Purge Cache)") {
                        coordinator.applyMemoryPressure(.critical)
                    }
                }
            }

            if let stats = coordinator.cacheStats {
                Text(
                    "TileCache: \(stats.cachedTileCount) tiles (\(stats.currentUsedBytes / 1024) KB / \(stats.effectiveMaxBytes / (1024 * 1024)) MB) • Hits: \(stats.hitCount) • Misses: \(stats.missCount) • Evictions: \(stats.evictionCount) • Prefetched: \(coordinator.prefetchedPagesSummary)"
                )
                .font(.caption)
                .foregroundStyle(.secondary)
            }

            ScrollView([.horizontal, .vertical], showsIndicators: true) {
                if let cgImage = coordinator.renderedPageImage {
                    Image(decorative: cgImage, scale: 1.0, orientation: .up)
                        .background(Color.white)
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                        .shadow(radius: 3)
                        .padding(16)
                } else {
                    Text("Rendering viewport via DocumentActor…")
                        .foregroundStyle(.secondary)
                        .padding(32)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: .underPageBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 6))
        }
    }
}
