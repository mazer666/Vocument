import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native SwiftUI dialog for Bates numbering configuration (`ROADMAP.md` M3.4).
struct BatesStampSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var prefix: String = "CONF-"
    @State private var startNumber: Int = 1
    @State private var digitPadding: Int = 6
    @State private var suffix: String = ""
    @State private var selectedPosition: FfiBatesPosition = .bottomRight
    @State private var fontSizePt: Int = 10
    @State private var marginPt: Int = 36

    private var samplePreview: String {
        let numStr = String(format: "%0*d", max(1, digitPadding), max(0, startNumber))
        return "\(prefix)\(numStr)\(suffix)"
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Apply Bates Numbering")
                .font(.headline)

            Text("\(coordinator.selectedWorkbenchPages.count) page(s) selected")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 14) {
                // Formatting row
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Prefix:")
                            .font(.callout)
                        TextField("e.g. CONF-", text: $prefix)
                            .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("Start #:")
                            .font(.callout)
                        TextField("Start", value: $startNumber, format: .number)
                            .frame(width: 70)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("Digits:")
                            .font(.callout)
                        TextField("Padding", value: $digitPadding, format: .number)
                            .frame(width: 60)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("Suffix:")
                            .font(.callout)
                        TextField("e.g. -A", text: $suffix)
                            .frame(width: 80)
                    }
                }

                // Position picker
                Picker("Position:", selection: $selectedPosition) {
                    Text("Top Left").tag(FfiBatesPosition.topLeft)
                    Text("Top Center").tag(FfiBatesPosition.topCenter)
                    Text("Top Right").tag(FfiBatesPosition.topRight)
                    Text("Bottom Left").tag(FfiBatesPosition.bottomLeft)
                    Text("Bottom Center").tag(FfiBatesPosition.bottomCenter)
                    Text("Bottom Right").tag(FfiBatesPosition.bottomRight)
                }
                .pickerStyle(.menu)

                // Font & margin
                HStack(spacing: 16) {
                    HStack {
                        Text("Font Size:")
                        TextField("Size", value: $fontSizePt, format: .number)
                            .frame(width: 50)
                        Text("pt")
                    }
                    .font(.callout)

                    HStack {
                        Text("Margin:")
                        TextField("Margin", value: $marginPt, format: .number)
                            .frame(width: 50)
                        Text("pt")
                    }
                    .font(.callout)
                }

                Divider()

                // Preview box
                VStack(alignment: .leading, spacing: 6) {
                    Text("Live Preview:")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    HStack {
                        Spacer()
                        Text(samplePreview)
                            .font(.system(size: CGFloat(fontSizePt), weight: .medium, design: .monospaced))
                            .padding(.horizontal, 16)
                            .padding(.vertical, 8)
                            .background(Color.secondary.opacity(0.12))
                            .cornerRadius(6)
                        Spacer()
                    }
                }
            }
            .padding(.horizontal, 8)

            Divider()

            HStack {
                Button("Cancel") {
                    coordinator.showBatesSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Apply Bates Numbers") {
                    let options = FfiBatesNumberingOptions(
                        prefix: prefix,
                        startNumber: UInt32(max(0, startNumber)),
                        digitPadding: UInt32(max(1, digitPadding)),
                        suffix: suffix,
                        position: selectedPosition,
                        fontSizePt: UInt32(max(6, fontSizePt)),
                        marginPt: UInt32(max(0, marginPt))
                    )
                    coordinator.showBatesSheet = false
                    coordinator.workbenchApplyBates(options: options)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 440)
    }
}
