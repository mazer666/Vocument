import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native SwiftUI sheet for PDF/X (ISO 15930) verification and professional print preflight (`PRINT-003`, `PRINT-004`, `M3.5`).
struct PreflightPdfxSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var pdfxReport: FfiPdfxPreflightReport?
    @State private var printReport: FfiPrintPreflightReport?

    var body: some View {
        VStack(spacing: 16) {
            // Header
            HStack(spacing: 12) {
                Image(systemName: "checkmark.seal")
                    .font(.system(size: 32))
                    .foregroundStyle(Color.accentColor)

                VStack(alignment: .leading, spacing: 4) {
                    Text("PDF/X & Print Preflight Analysis")
                        .font(.headline)
                    Text("ISO 15930 blind-exchange compliance and print readiness audit.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()
            }

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // PDF/X Section
                    pdfxSection

                    Divider()

                    // Print Readiness Section
                    printSection
                }
                .padding(.vertical, 4)
            }

            Divider()

            // Close Button
            HStack {
                Spacer()
                Button("Done", role: .cancel) {
                    coordinator.showPreflightPdfxSheet = false
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 520, height: 460)
        .onAppear {
            pdfxReport = coordinator.workbenchPreflightPdfx()
            printReport = coordinator.workbenchPreflightPrint()
        }
    }

    // MARK: - PDF/X Section

    @ViewBuilder
    private var pdfxSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("PDF/X Eingeschränkte Vorprüfung")
                    .font(.subheadline)
                    .bold()

                Spacer()

                if let report = pdfxReport {
                    if report.isValid {
                        Label("Strukturell konform (\(report.version))", systemImage: "checkmark.circle.fill")
                            .font(.caption)
                            .bold()
                            .foregroundStyle(.green)
                    } else {
                        Label("Abweichungen festgestellt", systemImage: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .bold()
                            .foregroundStyle(.orange)
                    }
                }
            }

            if let report = pdfxReport {
                // Disclaimer Callout
                HStack(alignment: .top, spacing: 8) {
                    Image(systemName: "info.circle.fill")
                        .foregroundStyle(Color.accentColor)
                        .font(.caption)
                    Text(report.disclaimer)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(8)
                .background(Color.accentColor.opacity(0.08))
                .cornerRadius(6)

                VStack(alignment: .leading, spacing: 6) {
                    attributeRow(label: "Output Intent", value: report.hasOutputIntent ? report.outputIntentProfile : "None (Missing)")
                    attributeRow(label: "Trapped Key", value: report.hasTrappedKey ? "True" : "Missing")
                    attributeRow(label: "Bleed / Trim Box", value: report.hasBleedOrTrimBox ? "Present" : "Missing")
                    attributeRow(label: "Schrifteneinbettung", value: report.unembeddedFonts.isEmpty ? "100% eingebettet" : "\(report.unembeddedFonts.count) nicht eingebettet")
                    attributeRow(label: "Verschlüsselung", value: report.isEncrypted ? "Verschlüsselt (Unzulässig in PDF/X)" : "Unverschlüsselt")
                }
                .padding(10)
                .background(Color.secondary.opacity(0.06))
                .cornerRadius(6)

                if !report.unembeddedFonts.isEmpty {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Nicht eingebettete Schriften:")
                            .font(.caption2)
                            .bold()
                            .foregroundStyle(.orange)
                        ForEach(report.unembeddedFonts, id: \.self) { font in
                            Text("• \(font)")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(.horizontal, 4)
                }

                if !report.issues.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Festgestellte Prüfungsbefunde:")
                            .font(.caption)
                            .bold()
                            .foregroundStyle(.secondary)

                        ForEach(report.issues, id: \.self) { issue in
                            HStack(alignment: .top, spacing: 6) {
                                Image(systemName: "xmark.circle")
                                    .foregroundStyle(.red)
                                    .font(.caption2)
                                Text(issue)
                                    .font(.caption)
                            }
                        }
                    }
                }
            }
        }
    }

    // MARK: - Print Section

    @ViewBuilder
    private var printSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Print Readiness Audit")
                    .font(.subheadline)
                    .bold()

                Spacer()

                if let report = printReport {
                    if report.warnings.isEmpty {
                        Label("Ready to Print", systemImage: "checkmark.circle.fill")
                            .font(.caption)
                            .bold()
                            .foregroundStyle(.green)
                    } else {
                        Label("\(report.warnings.count) Warning(s)", systemImage: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .bold()
                            .foregroundStyle(.orange)
                    }
                }
            }

            if let report = printReport {
                VStack(alignment: .leading, spacing: 6) {
                    let sizeDesc = String(format: "%.1f × %.1f pt", report.widthPt, report.heightPt)
                    attributeRow(label: "Uniform Page Size", value: report.isUniformSize ? "Yes (\(sizeDesc))" : "No (Mixed Sizes)")
                    attributeRow(label: "BleedBox", value: report.hasBleedBox ? "Configured" : "None")
                    attributeRow(label: "CropBox", value: report.hasCropBox ? "Configured" : "None")
                }
                .padding(10)
                .background(Color.secondary.opacity(0.06))
                .cornerRadius(6)

                if !report.warnings.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Print Warnings:")
                            .font(.caption)
                            .bold()
                            .foregroundStyle(.secondary)

                        ForEach(report.warnings, id: \.self) { warn in
                            HStack(alignment: .top, spacing: 6) {
                                Image(systemName: "exclamationmark.circle")
                                    .foregroundStyle(.orange)
                                    .font(.caption2)
                                Text(warn)
                                    .font(.caption)
                            }
                        }
                    }
                }
            }
        }
    }

    private func attributeRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.caption)
                .bold()
        }
    }
}
