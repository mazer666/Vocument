import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native SwiftUI dialog for discovering and exporting embedded raster images (`ROADMAP.md` M3.5).
struct ImageExtractionSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var images: [FfiExtractedImageInfo] = []
    @State private var selectedIndex: Int?
    @State private var statusMessage: String?

    var body: some View {
        VStack(spacing: 16) {
            // Header
            HStack(spacing: 12) {
                Image(systemName: "photo.on.rectangle.angled")
                    .font(.system(size: 32))
                    .foregroundStyle(Color.accentColor)

                VStack(alignment: .leading, spacing: 4) {
                    Text("Embedded Raster Images")
                        .font(.headline)
                    Text("Discover and extract full-resolution raster images preserved in original fidelity.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()
            }

            Divider()

            if images.isEmpty {
                VStack(spacing: 12) {
                    Spacer()
                    Image(systemName: "photo")
                        .font(.system(size: 44))
                        .foregroundStyle(.secondary)
                    Text("No embedded raster images found in this document.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Discovered \(images.count) image(s):")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    List(selection: $selectedIndex) {
                        ForEach(0..<images.count, id: \.self) { idx in
                            let img = images[idx]
                            HStack {
                                Image(systemName: "photo")
                                    .foregroundStyle(.secondary)
                                    .frame(width: 20)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text("Page \(img.pageIndex + 1) — Image #\(img.imageIndex + 1)")
                                        .font(.subheadline)
                                        .bold()
                                    Text("\(img.width) × \(img.height) px • \(img.bits) bits • \(img.colorspace)")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }

                                Spacer()

                                Text(img.format.lowercased() == "jpeg" ? "JPEG (Raw)" : (img.format.lowercased() == "jp2" ? "JP2 (Raw)" : "PNG (Rendered)"))
                                    .font(.caption2)
                                    .bold()
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(img.format.lowercased() == "jpeg" ? Color.green.opacity(0.15) : Color.secondary.opacity(0.15))
                                    .cornerRadius(4)
                            }
                            .padding(.vertical, 2)
                            .tag(idx)
                        }
                    }
                    .listStyle(.bordered)
                    .frame(height: 220)
                }
            }

            if let msg = statusMessage {
                Text(msg)
                    .font(.caption)
                    .foregroundStyle(Color.accentColor)
            }

            Divider()

            // Actions
            HStack {
                Button("Done", role: .cancel) {
                    coordinator.showImageExtractionSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Export Selected…") {
                    exportSelectedImage()
                }
                .disabled(selectedIndex == nil || images.isEmpty)

                Button("Export All…") {
                    exportAllImages()
                }
                .disabled(images.isEmpty)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 520, height: 440)
        .onAppear {
            images = coordinator.workbenchGetDocumentImages()
            if !images.isEmpty {
                selectedIndex = 0
            }
        }
    }

    private func fileExtension(for format: String) -> String {
        switch format.lowercased() {
        case "jpeg", "jpg": return "jpg"
        case "jp2", "jpx": return "jp2"
        default: return "png"
        }
    }

    private func exportSelectedImage() {
        guard let idx = selectedIndex, idx < images.count else { return }
        let img = images[idx]
        guard let data = coordinator.workbenchExtractImageBytes(
            pageIndex: img.pageIndex,
            imageIndex: img.imageIndex
        ) else {
            statusMessage = "Failed to extract image bytes."
            return
        }

        let ext = fileExtension(for: data.format)
        let panel = NSSavePanel()
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = "image_page\(img.pageIndex + 1)_\(img.imageIndex + 1).\(ext)"

        if panel.runModal() == .OK, let url = panel.url {
            do {
                try data.data.write(to: url, options: .atomic)
                let typeStr = data.isRawStream ? "Lossless original stream" : "Converted PNG"
                statusMessage = "\(typeStr) exported to \(url.lastPathComponent)"
            } catch {
                statusMessage = "Export failed: \(error.localizedDescription)"
            }
        }
    }

    private func exportAllImages() {
        guard !images.isEmpty else { return }
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "Choose Folder"

        if panel.runModal() == .OK, let folderUrl = panel.url {
            var count = 0
            for img in images {
                if let data = coordinator.workbenchExtractImageBytes(
                    pageIndex: img.pageIndex,
                    imageIndex: img.imageIndex
                ) {
                    let ext = fileExtension(for: data.format)
                    let fileUrl = folderUrl.appendingPathComponent("image_page\(img.pageIndex + 1)_\(img.imageIndex + 1).\(ext)")
                    try? data.data.write(to: fileUrl, options: .atomic)
                    count += 1
                }
            }
            statusMessage = "Exported \(count) image(s) to \(folderUrl.lastPathComponent)"
        }
    }
}
