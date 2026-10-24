import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Preset page boundary choices for native page cropping (`CROP-001`, `CROP-002`, `CROP-003`).
enum CropPreset: String, CaseIterable, Identifiable {
    case trimMargins = "Trim Margins (Auto-Crop)"
    case a4 = "A4 (210 × 297 mm)"
    case usLetter = "US Letter (8.5 × 11 in)"
    case custom = "Custom Margins"

    var id: String { rawValue }
}

/// Execution mode for page crop mutations (`CROP-001`).
enum CropMode: String, CaseIterable, Identifiable {
    case display = "Non-destructive Display Crop (/CropBox)"
    case hard = "Destructive Hard Crop (Clipping Path & Prune)"

    var id: String { rawValue }
}

/// Native SwiftUI dialog for millimeter-accurate page boundary cropping (`CROP-001`–`CROP-003`).
struct CropSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var selectedPreset: CropPreset = .trimMargins
    @State private var selectedMode: CropMode = .display
    @State private var safetyMarginMm: Double = 5.0
    @State private var customTopMm: Double = 10.0
    @State private var customBottomMm: Double = 10.0
    @State private var customLeftMm: Double = 10.0
    @State private var customRightMm: Double = 10.0

    var body: some View {
        VStack(spacing: 20) {
            Text("Crop Selected Pages")
                .font(.headline)

            Text("\(coordinator.selectedWorkbenchPages.count) page(s) selected")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                Picker("Preset:", selection: $selectedPreset) {
                    ForEach(CropPreset.allCases) { preset in
                        Text(preset.rawValue).tag(preset)
                    }
                }
                .pickerStyle(.menu)

                if selectedPreset == .trimMargins {
                    HStack {
                        Text("Safety Margin:")
                        TextField("Margin", value: $safetyMarginMm, format: .number)
                            .frame(width: 60)
                        Text("mm")
                    }
                    .font(.callout)
                } else if selectedPreset == .custom {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Margins from Page Edge:")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        HStack {
                            Text("Top:")
                            TextField("Top", value: $customTopMm, format: .number)
                                .frame(width: 50)
                            Text("mm")
                            Spacer()
                            Text("Bottom:")
                            TextField("Bottom", value: $customBottomMm, format: .number)
                                .frame(width: 50)
                            Text("mm")
                        }
                        HStack {
                            Text("Left:")
                            TextField("Left", value: $customLeftMm, format: .number)
                                .frame(width: 50)
                            Text("mm")
                            Spacer()
                            Text("Right:")
                            TextField("Right", value: $customRightMm, format: .number)
                                .frame(width: 50)
                            Text("mm")
                        }
                    }
                }

                Divider()

                Picker("Mode:", selection: $selectedMode) {
                    ForEach(CropMode.allCases) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.radioGroup)

                Text(selectedMode == .display
                     ? "Non-destructive: Modifies /CropBox display boundary. Original page contents are preserved."
                     : "Destructive: Injects clipping path (q ... W n ... Q) into content stream and prunes outside annotations.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding()
            .background(Color(NSColor.controlBackgroundColor))
            .cornerRadius(8)

            HStack {
                Button("Cancel") {
                    coordinator.showCropSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Apply Crop") {
                    applyCrop()
                    coordinator.showCropSheet = false
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 440)
    }

    private func applyCrop() {
        if selectedPreset == .trimMargins {
            let safetyMilliPt = Int32((safetyMarginMm * 72.0 / 25.4 * 1000.0).rounded())
            coordinator.workbenchAutoCropSelectedPages(safetyMarginMilliPt: safetyMilliPt)
            return
        }

        // Calculate rect for custom or preset
        guard let session = coordinator.activeSession else { return }
        let primaryPage = coordinator.selectedWorkbenchPages.sorted().first ?? coordinator.currentPageIndex
        guard let boxes = try? coordinator.coreClient.getPageBoxes(sessionId: session.sessionId, pageIndex: primaryPage) else { return }

        let mediaWidthPt = Double(boxes.mediaBox.x1MilliPt - boxes.mediaBox.x0MilliPt) / 1000.0
        let mediaHeightPt = Double(boxes.mediaBox.y1MilliPt - boxes.mediaBox.y0MilliPt) / 1000.0

        let topMilliPt: Int32
        let bottomMilliPt: Int32
        let leftMilliPt: Int32
        let rightMilliPt: Int32

        switch selectedPreset {
        case .a4:
            let targetWPt = 595.276
            let targetHPt = 841.89
            let dxPt = max(0.0, (mediaWidthPt - targetWPt) / 2.0)
            let dyPt = max(0.0, (mediaHeightPt - targetHPt) / 2.0)
            leftMilliPt = Int32((dxPt * 1000.0).rounded())
            topMilliPt = Int32((dyPt * 1000.0).rounded())
            rightMilliPt = Int32(((mediaWidthPt - dxPt) * 1000.0).rounded())
            bottomMilliPt = Int32(((mediaHeightPt - dyPt) * 1000.0).rounded())
        case .usLetter:
            let targetWPt = 612.0
            let targetHPt = 792.0
            let dxPt = max(0.0, (mediaWidthPt - targetWPt) / 2.0)
            let dyPt = max(0.0, (mediaHeightPt - targetHPt) / 2.0)
            leftMilliPt = Int32((dxPt * 1000.0).rounded())
            topMilliPt = Int32((dyPt * 1000.0).rounded())
            rightMilliPt = Int32(((mediaWidthPt - dxPt) * 1000.0).rounded())
            bottomMilliPt = Int32(((mediaHeightPt - dyPt) * 1000.0).rounded())
        case .custom:
            let leftPt = customLeftMm * 72.0 / 25.4
            let rightPt = customRightMm * 72.0 / 25.4
            let topPt = customTopMm * 72.0 / 25.4
            let bottomPt = customBottomMm * 72.0 / 25.4
            leftMilliPt = Int32((leftPt * 1000.0).rounded())
            topMilliPt = Int32((topPt * 1000.0).rounded())
            rightMilliPt = Int32(((mediaWidthPt - rightPt) * 1000.0).rounded())
            bottomMilliPt = Int32(((mediaHeightPt - bottomPt) * 1000.0).rounded())
        case .trimMargins:
            return
        }

        let cropRect = FfiPdfRectMilliPt(
            x0MilliPt: leftMilliPt,
            y0MilliPt: topMilliPt,
            x1MilliPt: rightMilliPt,
            y1MilliPt: bottomMilliPt
        )

        if selectedMode == .hard {
            coordinator.workbenchApplyHardCrop(cropRect: cropRect)
        } else {
            coordinator.workbenchApplyDisplayCrop(cropRect: cropRect)
        }
    }
}
