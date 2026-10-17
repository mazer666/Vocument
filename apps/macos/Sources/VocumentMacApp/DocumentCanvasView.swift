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

                Divider().frame(height: 18)

                Button("↺ −90°") {
                    coordinator.rotateCurrentPage(deltaDegrees: -90)
                }
                Button("↻ +90°") {
                    coordinator.rotateCurrentPage(deltaDegrees: 90)
                }
                Text("Rot: \(coordinator.currentPageRotationDegrees)°")
                    .font(.caption)
                    .monospacedDigit()

                Button("Highlight Line") {
                    coordinator.highlightFirstLineOnCurrentPage()
                }

                Divider().frame(height: 18)

                Button("Undo") {
                    coordinator.performUndo()
                }
                .disabled(!(coordinator.transactionHistory?.canUndo ?? false))
                .help(coordinator.transactionHistory?.undoDescription ?? "Nothing to undo")

                Button("Redo") {
                    coordinator.performRedo()
                }
                .disabled(!(coordinator.transactionHistory?.canRedo ?? false))
                .help(coordinator.transactionHistory?.redoDescription ?? "Nothing to redo")

                Divider().frame(height: 18)

                Button("Save") {
                    coordinator.save()
                }
                .disabled(!(coordinator.transactionHistory?.isDirty ?? false))
                .help("Atomically saves changes to disk (Cmd+S)")
                .keyboardShortcut("s", modifiers: .command)

                Button("Save As…") {
                    coordinator.saveAsViaPanel()
                }
                .help("Saves document to a new file location")

                Button("Safe Copy") {
                    coordinator.saveCollisionFreeCopy()
                }
                .help("Saves a non-colliding copy with timestamp/counter")

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

            if let notice = coordinator.saveNotificationMessage {
                Text(notice)
                    .font(.caption)
                    .foregroundStyle(Color.green)
            }

            if let hist = coordinator.transactionHistory {
                Text(
                    "Revision \(hist.currentRevision) (\(hist.isDirty ? "Modified • Unsaved" : "Clean")) • Undo stack: \(hist.undoCount) (\(hist.undoDescription ?? "none")) • Redo stack: \(hist.redoCount) (\(hist.redoDescription ?? "none"))"
                )
                .font(.caption)
                .foregroundStyle(hist.isDirty ? Color.orange : Color.secondary)
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
        .sheet(item: $coordinator.activeConflict) { conflict in
            SaveConflictSheet(coordinator: coordinator, conflict: conflict)
        }
    }
}

/// Conflict resolution dialog presenting safe-copy, explicit overwrite, reload, or cancel (`SAVE-001`, `FILELOCK-004`).
struct SaveConflictSheet: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator
    let conflict: FfiSaveConflict

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(.orange)
                VStack(alignment: .leading, spacing: 4) {
                    Text("External Change Detected")
                        .font(.headline)
                    Text("The file has been modified outside Vocument since it was opened.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Target Path:").font(.caption).bold()
                Text(conflict.targetToken).font(.caption).foregroundStyle(.secondary)

                Text("Comparison:").font(.caption).bold()
                Text(conflict.comparisonSummary).font(.caption).foregroundStyle(.secondary)

                Text("Safe Copy Target:").font(.caption).bold()
                Text(conflict.suggestedSafeCopyPath).font(.caption).foregroundStyle(.secondary)
            }
            .padding(12)
            .background(Color(nsColor: .controlBackgroundColor))
            .cornerRadius(6)

            Divider()

            VStack(spacing: 8) {
                Button("Save Local Changes as Safe Copy (Recommended)") {
                    coordinator.resolveActiveConflict(.saveLocalAsCollisionFreeCopy)
                }
                .buttonStyle(.borderedProminent)
                .frame(maxWidth: .infinity)

                Button("Overwrite External Changes on Disk") {
                    coordinator.resolveActiveConflict(.overwriteExternalExplicitly)
                }
                .foregroundStyle(.red)
                .frame(maxWidth: .infinity)

                Button("Discard Local Changes & Reload External File") {
                    coordinator.resolveActiveConflict(.reloadExternalVersion)
                }
                .frame(maxWidth: .infinity)

                Button("Cancel Save") {
                    coordinator.resolveActiveConflict(.cancel)
                }
                .frame(maxWidth: .infinity)
            }
        }
        .padding(24)
        .frame(width: 480)
    }
}

extension FfiSaveConflict: Identifiable {
    public var id: String { "\(sessionId)-\(targetToken)-\(observedExternalIdentity.sha256Hex)" }
}
