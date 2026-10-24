import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// User choice for optimization preset (`ROADMAP.md` M3.5).
enum OptimizePresetChoice: String, CaseIterable, Identifiable {
    case lossless = "Lossless"
    case balanced = "Balanced"
    case maximum = "Maximum"

    var id: String { rawValue }

    var ffiPreset: FfiOptimizationPreset {
        switch self {
        case .lossless: return .lossless
        case .balanced: return .balanced
        case .maximum: return .maximum
        }
    }

    var description: String {
        switch self {
        case .lossless:
            return "Deduplicates objects, re-compresses streams with Flate, and removes orphaned PDF structures without altering any pixel."
        case .balanced:
            return "Re-compresses embedded images and compacts internal structures while maintaining sharp visual quality."
        case .maximum:
            return "Aggressively downsizes large graphics and compacts all internal object streams for the smallest file size."
        }
    }
}

/// Native SwiftUI dialog for PDF optimization, object deduplication, and stream compression (`ROADMAP.md` M3.5).
struct OptimizeSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var selectedPreset: OptimizePresetChoice = .lossless

    var body: some View {
        VStack(spacing: 20) {
            // Header
            HStack(spacing: 12) {
                Image(systemName: "gauge.with.dots.needle.bottom.50percent")
                    .font(.system(size: 32))
                    .foregroundStyle(Color.accentColor)

                VStack(alignment: .leading, spacing: 4) {
                    Text("Optimize & Compress Document")
                        .font(.headline)
                    Text("Reduce file size, remove orphaned objects, and compact cross-reference streams.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()
            }

            Divider()

            // Preset Selection
            VStack(alignment: .leading, spacing: 10) {
                Text("Compression Level")
                    .font(.subheadline)
                    .bold()

                Picker("Preset", selection: $selectedPreset) {
                    ForEach(OptimizePresetChoice.allCases) { preset in
                        Text(preset.rawValue).tag(preset)
                    }
                }
                .pickerStyle(.segmented)

                Text(selectedPreset.description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.vertical, 4)
            }

            Divider()

            // Object Streams & Web Byte-Range Access
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Image(systemName: "network")
                        .font(.subheadline)
                        .foregroundStyle(Color.accentColor)
                    Text("Object Streams & Byte-Range Delivery (/ObjStm)")
                        .font(.subheadline)
                        .bold()
                }

                Text("All presets compact internal PDF structures into ISO 32000-1 object streams. This enables fast HTTP range-request delivery (RFC 7233) in modern viewers and web browsers with zero loss of document appearance, forms, or metadata.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer()

            Divider()

            // Action Buttons
            HStack {
                Button("Cancel", role: .cancel) {
                    coordinator.showOptimizeSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Optimize Document") {
                    let options = FfiOptimizationOptions(
                        preset: selectedPreset.ffiPreset
                    )
                    coordinator.showOptimizeSheet = false
                    coordinator.workbenchOptimizeDocument(options: options)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 480, height: 380)
    }
}
