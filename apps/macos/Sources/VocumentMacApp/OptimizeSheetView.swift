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

/// Native SwiftUI dialog for PDF optimization, compression, and Fast Web View linearization (`ROADMAP.md` M3.5).
struct OptimizeSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var selectedPreset: OptimizePresetChoice = .lossless
    @State private var enableFastWebView: Bool = true

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
                    Text("Reduce file size, remove duplicates, and linearize for Fast Web View.")
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

            // Linearization / Fast Web View
            VStack(alignment: .leading, spacing: 8) {
                Text("Web & Streaming")
                    .font(.subheadline)
                    .bold()

                Toggle("Fast Web View (Linearize document)", isOn: $enableFastWebView)
                    .font(.body)

                Text("Organizes PDF objects so page 1 renders immediately over HTTP connections before downloading the full file.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
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
                        preset: selectedPreset.ffiPreset,
                        doLinearize: enableFastWebView
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
