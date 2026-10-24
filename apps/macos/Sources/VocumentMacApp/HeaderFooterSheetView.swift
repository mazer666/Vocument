import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native SwiftUI dialog for multi-zone header and footer configuration (`ROADMAP.md` M3.4).
struct HeaderFooterSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var headerLeft: String = ""
    @State private var headerCenter: String = "{filename}"
    @State private var headerRight: String = "{date}"
    @State private var footerLeft: String = "Confidential"
    @State private var footerCenter: String = "Page {page} of {total}"
    @State private var footerRight: String = ""
    @State private var fontSizePt: Int = 9
    @State private var marginPt: Int = 36
    @State private var docTitleOverride: String = ""
    @State private var dateOverride: String = ""

    var body: some View {
        VStack(spacing: 18) {
            Text("Headers & Footers")
                .font(.headline)

            Text("\(coordinator.selectedWorkbenchPages.count) page(s) selected")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 14) {
                // Macro tags helper
                HStack(spacing: 8) {
                    Text("Macros:")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    macroTag("{page}", label: "Current Page")
                    macroTag("{total}", label: "Total Pages")
                    macroTag("{date}", label: "Current Date")
                    macroTag("{filename}", label: "Doc Title")
                }

                Divider()

                // Header section
                VStack(alignment: .leading, spacing: 6) {
                    Text("Header Zones (Top)")
                        .font(.subheadline)
                        .fontWeight(.semibold)

                    HStack(spacing: 8) {
                        TextField("Left Header", text: $headerLeft)
                        TextField("Center Header", text: $headerCenter)
                        TextField("Right Header", text: $headerRight)
                    }
                }

                // Footer section
                VStack(alignment: .leading, spacing: 6) {
                    Text("Footer Zones (Bottom)")
                        .font(.subheadline)
                        .fontWeight(.semibold)

                    HStack(spacing: 8) {
                        TextField("Left Footer", text: $footerLeft)
                        TextField("Center Footer", text: $footerCenter)
                        TextField("Right Footer", text: $footerRight)
                    }
                }

                Divider()

                // Styling & Overrides
                HStack(spacing: 16) {
                    HStack {
                        Text("Font Size:")
                        TextField("Size", value: $fontSizePt, format: .number)
                            .frame(width: 45)
                        Text("pt")
                    }
                    .font(.callout)

                    HStack {
                        Text("Margin:")
                        TextField("Margin", value: $marginPt, format: .number)
                            .frame(width: 45)
                        Text("pt")
                    }
                    .font(.callout)

                    Spacer()

                    TextField("Date override", text: $dateOverride)
                        .frame(width: 120)
                }
            }
            .padding(.horizontal, 8)

            Divider()

            HStack {
                Button("Cancel") {
                    coordinator.showHeaderFooterSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Apply Headers & Footers") {
                    let options = FfiHeaderFooterOptions(
                        headerLeft: headerLeft.isEmpty ? nil : headerLeft,
                        headerCenter: headerCenter.isEmpty ? nil : headerCenter,
                        headerRight: headerRight.isEmpty ? nil : headerRight,
                        footerLeft: footerLeft.isEmpty ? nil : footerLeft,
                        footerCenter: footerCenter.isEmpty ? nil : footerCenter,
                        footerRight: footerRight.isEmpty ? nil : footerRight,
                        fontSizePt: UInt32(max(6, fontSizePt)),
                        marginPt: UInt32(max(0, marginPt))
                    )
                    let titleParam = docTitleOverride.isEmpty ? nil : docTitleOverride
                    let dateParam = dateOverride.isEmpty ? nil : dateOverride

                    coordinator.showHeaderFooterSheet = false
                    coordinator.workbenchApplyHeaderFooter(
                        options: options,
                        docTitle: titleParam,
                        dateStr: dateParam
                    )
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 520)
    }

    @ViewBuilder
    private func macroTag(_ macro: String, label: String) -> some View {
        Text(macro)
            .font(.caption)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(Color.blue.opacity(0.12))
            .foregroundStyle(.blue)
            .cornerRadius(4)
            .help(label)
    }
}
