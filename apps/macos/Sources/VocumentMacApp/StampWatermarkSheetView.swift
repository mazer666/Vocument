import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Preset choices for rubber stamps and watermarks (`WATER-001`–`003`, `M3.4`).
enum StampPresetChoice: String, CaseIterable, Identifiable {
    case draft = "DRAFT"
    case confidential = "CONFIDENTIAL"
    case approved = "APPROVED"
    case custom = "Custom…"

    var id: String { rawValue }
}

/// Color presets for rubber stamps and watermarks.
enum StampColorChoice: String, CaseIterable, Identifiable {
    case red = "Red"
    case gray = "Gray"
    case blue = "Blue"
    case green = "Green"

    var id: String { rawValue }

    var rgb: (u8: UInt8, g8: UInt8, b8: UInt8) {
        switch self {
        case .red: return (220, 30, 30)
        case .gray: return (128, 128, 128)
        case .blue: return (30, 80, 220)
        case .green: return (30, 160, 50)
        }
    }

    var swiftColor: Color {
        let (r, g, b) = rgb
        return Color(red: Double(r) / 255.0, green: Double(g) / 255.0, blue: Double(b) / 255.0)
    }
}

/// Native SwiftUI dialog for rubber stamp and watermark configuration (`WATER-001`–`003`, `M3.4`).
struct StampWatermarkSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var selectedPreset: StampPresetChoice = .confidential
    @State private var customText: String = "CONFIDENTIAL"
    @State private var customColor: StampColorChoice = .red
    @State private var customRotation: Double = -45.0
    @State private var opacityPercent: Double = 50.0
    @State private var isBackground: Bool = false

    private var effectiveText: String {
        switch selectedPreset {
        case .draft: return "DRAFT"
        case .confidential: return "CONFIDENTIAL"
        case .approved: return "APPROVED"
        case .custom: return customText.isEmpty ? "STAMP" : customText
        }
    }

    private var effectiveColor: StampColorChoice {
        switch selectedPreset {
        case .draft: return .gray
        case .confidential: return .red
        case .approved: return .green
        case .custom: return customColor
        }
    }

    private var effectiveRotation: Double {
        switch selectedPreset {
        case .draft: return -45.0
        case .confidential: return -45.0
        case .approved: return 0.0
        case .custom: return customRotation
        }
    }

    var body: some View {
        VStack(spacing: 18) {
            Text("Stamp & Watermark")
                .font(.headline)

            Text("\(coordinator.selectedWorkbenchPages.count) page(s) selected")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 14) {
                // Preset picker
                Picker("Preset:", selection: $selectedPreset) {
                    ForEach(StampPresetChoice.allCases) { preset in
                        Text(preset.rawValue).tag(preset)
                    }
                }
                .pickerStyle(.segmented)

                if selectedPreset == .custom {
                    TextField("Custom Text", text: $customText)

                    HStack(spacing: 16) {
                        Picker("Color:", selection: $customColor) {
                            ForEach(StampColorChoice.allCases) { color in
                                Text(color.rawValue).tag(color)
                            }
                        }
                        .frame(width: 140)

                        Picker("Rotation:", selection: $customRotation) {
                            Text("-45°").tag(-45.0)
                            Text("0°").tag(0.0)
                            Text("45°").tag(45.0)
                        }
                        .pickerStyle(.segmented)
                    }
                }

                // Layering toggle
                Toggle("Behind page content (watermark)", isOn: $isBackground)
                    .font(.callout)

                // Opacity slider
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Opacity:")
                        Spacer()
                        Text("\(Int(opacityPercent))%")
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                    }
                    .font(.callout)

                    Slider(value: $opacityPercent, in: 10...100, step: 5)
                }

                Divider()

                // Live Preview
                VStack(spacing: 4) {
                    Text("Live Preview:")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    ZStack {
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color.white)
                            .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.3), lineWidth: 1))
                            .frame(height: 100)

                        Text(effectiveText)
                            .font(.system(size: 26, weight: .heavy, design: .rounded))
                            .foregroundStyle(effectiveColor.swiftColor)
                            .opacity(opacityPercent / 100.0)
                            .rotationEffect(.degrees(effectiveRotation))
                    }
                }
            }
            .padding(.horizontal, 8)

            Divider()

            HStack {
                Button("Cancel") {
                    coordinator.showStampSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Apply Stamp") {
                    let rgb = effectiveColor.rgb
                    let options = FfiStampOrWatermarkOptions(
                        text: effectiveText,
                        opacityMilli: UInt32(max(10, min(1000, Int(opacityPercent * 10.0)))),
                        rotationDegrees: Int32(effectiveRotation),
                        r: rgb.u8,
                        g: rgb.g8,
                        b: rgb.b8,
                        isBackground: isBackground,
                        rect: nil
                    )
                    coordinator.showStampSheet = false
                    coordinator.workbenchApplyStamp(options: options)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 480)
    }
}
