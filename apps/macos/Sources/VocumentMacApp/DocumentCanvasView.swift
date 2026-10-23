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
                    PageImageView(coordinator: coordinator, image: cgImage)
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
        .sheet(item: $coordinator.pendingRecovery) { recovery in
            RecoveryPromptSheet(coordinator: coordinator, recovery: recovery)
        }
    }
}

/// The page, with the place the user marks for a redaction (`REDACT-001`).
///
/// The image is drawn with one point per pixel, so a rectangle in the
/// coordinate space of the view is a rectangle in the coordinate space of the
/// page. The core then decides which text lies inside it — the shell does not
/// guess.
struct PageImageView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator
    let image: CGImage

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                Image(decorative: image, scale: 1.0, orientation: .up)
                    .background(Color.white)
                if coordinator.canvasDragRect.width > 1, coordinator.canvasDragRect.height > 1 {
                    Rectangle()
                        .fill(Color.red.opacity(0.25))
                        .frame(width: coordinator.canvasDragRect.width, height: coordinator.canvasDragRect.height)
                        .offset(x: coordinator.canvasDragRect.minX, y: coordinator.canvasDragRect.minY)
                        .allowsHitTesting(false)
                }
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 4)
                    .onChanged { value in
                        if coordinator.canvasDragStart == nil {
                            coordinator.canvasDragStart = value.startLocation
                        }
                        coordinator.canvasDragRect = Self.rect(from: coordinator.canvasDragStart ?? value.startLocation, to: value.location)
                    }
                    .onEnded { value in
                        let rect = Self.rect(from: coordinator.canvasDragStart ?? value.startLocation, to: value.location)
                        coordinator.canvasDragStart = nil
                        coordinator.canvasDragRect = .zero
                        guard let pageRect = coordinator.pageRect(fromViewRect: rect) else {
                            return
                        }
                        coordinator.applyRedactionSelection(pageRect)
                    }
            )
        }
        .frame(width: CGFloat(image.width), height: CGFloat(image.height))
        .clipShape(RoundedRectangle(cornerRadius: 4))
        .shadow(radius: 3)
    }

    /// The rectangle between two points, however the drag ran.
    private static func rect(from first: CGPoint, to second: CGPoint) -> CGRect {
        CGRect(
            x: min(first.x, second.x),
            y: min(first.y, second.y),
            width: abs(second.x - first.x),
            height: abs(second.y - first.y)
        )
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

/// Crash recovery dialog offering Restore, Preview, Discard, or Open as New (`ROADMAP.md` M0.6, `RECOVERY-001` Preset).
struct RecoveryPromptSheet: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator
    let recovery: FfiRecoverableDocumentInfo

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                Image(systemName: "arrow.counterclockwise.circle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(.blue)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Unsaved Changes Recovered")
                        .font(.headline)
                    Text("Vocument detected uncommitted edits after an unexpected shutdown.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Document:").font(.caption).bold()
                Text(recovery.displayName).font(.caption).foregroundStyle(.primary)

                Text("Location:").font(.caption).bold()
                Text(recovery.sourceToken).font(.caption).foregroundStyle(.secondary)

                Text("Uncommitted Revisions:").font(.caption).bold()
                Text("\(recovery.uncommittedRevisions)").font(.caption).foregroundStyle(.secondary)
            }
            .padding(12)
            .background(Color(nsColor: .controlBackgroundColor))
            .cornerRadius(6)

            Text("Original file on disk remains completely intact until you explicitly save.")
                .font(.caption2)
                .foregroundStyle(.secondary)

            Divider()

            VStack(spacing: 8) {
                Button("Restore Unsaved Changes (Recommended)") {
                    coordinator.executeRecovery(action: .restore)
                }
                .buttonStyle(.borderedProminent)
                .frame(maxWidth: .infinity)

                Button("Preview Recovered Version") {
                    coordinator.executeRecovery(action: .preview)
                }
                .frame(maxWidth: .infinity)

                Button("Open Recovered Version as New Document") {
                    coordinator.executeRecovery(action: .openAsNew)
                }
                .frame(maxWidth: .infinity)

                Button("Discard Recovery & Open Original") {
                    coordinator.executeRecovery(action: .discard)
                }
                .foregroundStyle(.red)
                .frame(maxWidth: .infinity)
            }
        }
        .padding(24)
        .frame(width: 480)
    }
}

extension FfiRecoverableDocumentInfo: Identifiable {
    public var id: String { documentId }
}

