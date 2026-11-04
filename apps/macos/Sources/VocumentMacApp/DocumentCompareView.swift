import SwiftUI
import AppKit
import VocumentFFI
import VocumentMacKit

/// Dual-pane synchronized document comparison view for macOS (`COMPARE-001`–`COMPARE-004`).
struct DocumentCompareView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    var body: some View {
        VStack(spacing: 0) {
            // 1. Top Summary and Controls Bar
            compareToolbar

            Divider()

            // 2. Main Content: Changes List Sidebar + Dual Viewports
            HStack(alignment: .top, spacing: 0) {
                // Changes list sidebar
                changesSidebar
                    .frame(minWidth: 280, idealWidth: 320, maxWidth: 400)

                Divider()

                // Dual synchronized viewports
                dualViewportArea
                    .frame(minWidth: 600)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(NSColor.windowBackgroundColor))
    }

    // MARK: - Toolbar View
    private var compareToolbar: some View {
        HStack(spacing: 16) {
            if let report = coordinator.compareReport {
                HStack(spacing: 12) {
                    Label("\(report.summary.totalChanges) Änderungen", systemImage: "arrow.triangle.swap")
                        .font(.headline)

                    HStack(spacing: 8) {
                        Text("+\(report.summary.insertionsCount)")
                            .font(.subheadline)
                            .foregroundColor(.green)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.green.opacity(0.15))
                            .cornerRadius(4)

                        Text("-\(report.summary.deletionsCount)")
                            .font(.subheadline)
                            .foregroundColor(.red)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.red.opacity(0.15))
                            .cornerRadius(4)

                        Text("~\(report.summary.replacementsCount)")
                            .font(.subheadline)
                            .foregroundColor(.orange)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.orange.opacity(0.15))
                            .cornerRadius(4)

                        Text("⇄\(report.summary.movesCount)")
                            .font(.subheadline)
                            .foregroundColor(.blue)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.blue.opacity(0.15))
                            .cornerRadius(4)
                    }

                    Text("Übereinstimmung: \(Int(report.summary.similarityRatio * 100))%")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
            } else {
                Text("Kein aktiver Vergleich. Wählen Sie Dokument B.")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }

            Spacer()

            // Sync Toggle
            Toggle("Synchroner Bildlauf", isOn: $coordinator.compareSyncScroll)
                .toggleStyle(.checkbox)

            // Visual Diff Overlay Toggle
            Toggle("Visueller Diff", isOn: $coordinator.compareShowVisualDiffOverlay)
                .toggleStyle(.checkbox)

            // Select Document B
            Button("Dokument B wählen…") {
                coordinator.openSecondDocumentForComparisonViaPanel()
            }
            .buttonStyle(.bordered)

            // Export Menu
            Menu("Bericht exportieren") {
                Button("HTML-Bericht (.html)") {
                    coordinator.exportComparisonReportToFile(format: "html")
                }
                Button("Markdown-Bericht (.md)") {
                    coordinator.exportComparisonReportToFile(format: "md")
                }
                Button("JSON-Rohdaten (.json)") {
                    coordinator.exportComparisonReportToFile(format: "json")
                }
            }
            .disabled(coordinator.compareReport == nil)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Color(NSColor.controlBackgroundColor))
    }

    // MARK: - Changes Sidebar
    private var changesSidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Filter header
            HStack {
                Text("Änderungsliste")
                    .font(.headline)
                Spacer()
                if let count = coordinator.compareReport?.changes.count {
                    Text("\(count)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            .padding(12)

            Divider()

            if let report = coordinator.compareReport, !report.changes.isEmpty {
                ScrollView {
                    LazyVStack(spacing: 4) {
                        ForEach(report.changes, id: \.id) { change in
                            ChangeRowView(
                                change: change,
                                isSelected: coordinator.selectedChangeId == change.id,
                                onSelect: {
                                    coordinator.jumpToChange(item: change)
                                }
                            )
                        }
                    }
                    .padding(8)
                }
            } else {
                VStack(spacing: 8) {
                    Spacer()
                    Image(systemName: "checkmark.circle")
                        .font(.system(size: 32))
                        .foregroundColor(.secondary)
                    Text("Keine Änderungen oder Dokumente identisch.")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity)
            }
        }
        .background(Color(NSColor.controlBackgroundColor).opacity(0.5))
    }

    // MARK: - Dual Viewport Area
    private var dualViewportArea: some View {
        HStack(spacing: 2) {
            // Document A (Original)
            singleDocumentPane(
                title: coordinator.activeSession?.fileIdentity.displayName ?? "Dokument A",
                pageIndex: coordinator.comparePageA,
                pageCount: coordinator.activeSession?.pdfSummary.pageCount ?? 0,
                renderedImage: coordinator.compareRenderedImageA,
                isDocumentA: true,
                onPreviousPage: {
                    if coordinator.comparePageA > 0 {
                        coordinator.comparePageA -= 1
                        if coordinator.compareSyncScroll && coordinator.comparePageB > 0 {
                            coordinator.comparePageB -= 1
                        }
                        coordinator.renderComparisonViewports()
                    }
                },
                onNextPage: {
                    if let maxPages = coordinator.activeSession?.pdfSummary.pageCount, coordinator.comparePageA + 1 < maxPages {
                        coordinator.comparePageA += 1
                        if coordinator.compareSyncScroll, let maxB = coordinator.compareSecondSession?.pdfSummary.pageCount, coordinator.comparePageB + 1 < maxB {
                            coordinator.comparePageB += 1
                        }
                        coordinator.renderComparisonViewports()
                    }
                }
            )

            Divider()

            // Document B (Modified)
            singleDocumentPane(
                title: coordinator.compareSecondSession?.fileIdentity.displayName ?? "Dokument B",
                pageIndex: coordinator.comparePageB,
                pageCount: coordinator.compareSecondSession?.pdfSummary.pageCount ?? 0,
                renderedImage: coordinator.compareRenderedImageB,
                isDocumentA: false,
                onPreviousPage: {
                    if coordinator.comparePageB > 0 {
                        coordinator.comparePageB -= 1
                        coordinator.renderComparisonViewports()
                    }
                },
                onNextPage: {
                    if let maxPages = coordinator.compareSecondSession?.pdfSummary.pageCount, coordinator.comparePageB + 1 < maxPages {
                        coordinator.comparePageB += 1
                        coordinator.renderComparisonViewports()
                    }
                }
            )
        }
    }

    // MARK: - Single Document Pane
    private func singleDocumentPane(
        title: String,
        pageIndex: UInt32,
        pageCount: UInt32,
        renderedImage: CGImage?,
        isDocumentA: Bool,
        onPreviousPage: @escaping () -> Void,
        onNextPage: @escaping () -> Void
    ) -> some View {
        VStack(spacing: 0) {
            // Pane header
            HStack {
                Text(title)
                    .font(.headline)
                    .lineLimit(1)
                Spacer()
                Text("Seite \(pageIndex + 1) / \(max(1, pageCount))")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Button(action: onPreviousPage) {
                    Image(systemName: "chevron.left")
                }
                .disabled(pageIndex == 0)
                Button(action: onNextPage) {
                    Image(systemName: "chevron.right")
                }
                .disabled(pageIndex + 1 >= pageCount)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color(NSColor.controlBackgroundColor))

            Divider()

            // Page Canvas
            ScrollView([.horizontal, .vertical]) {
                ZStack {
                    if let image = renderedImage {
                        Image(image, scale: 1.0, label: Text("Seite \(pageIndex + 1)"))
                            .interpolation(.high)
                            .shadow(radius: 2)
                    } else {
                        Rectangle()
                            .fill(Color.white)
                            .frame(width: 595, height: 842)
                            .overlay(Text("Keine Vorschau verfügbar").foregroundColor(.secondary))
                    }

                    // Visual Diff Overlay on Document B
                    if !isDocumentA && coordinator.compareShowVisualDiffOverlay, let diff = coordinator.compareVisualDiff {
                        ForEach(diff.differenceBoundingBoxes.indices, id: \.self) { idx in
                            let box = diff.differenceBoundingBoxes[idx]
                            let x0 = CGFloat(box.x0MilliPt) / 1000.0
                            let y0 = CGFloat(box.y0MilliPt) / 1000.0
                            let w = CGFloat(box.x1MilliPt - box.x0MilliPt) / 1000.0
                            let h = CGFloat(box.y1MilliPt - box.y0MilliPt) / 1000.0

                            Rectangle()
                                .stroke(Color.red.opacity(0.8), lineWidth: 2)
                                .background(Color.red.opacity(0.15))
                                .frame(width: max(4, w), height: max(4, h))
                                .position(x: x0 + w / 2, y: y0 + h / 2)
                        }
                    }
                }
                .padding(16)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Row view for an individual change in the changes sidebar.
struct ChangeRowView: View {
    let change: FfiChangeItem
    let isSelected: Bool
    let onSelect: () -> Void

    var body: some View {
        Button(action: onSelect) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    operationBadge
                    if let clause = change.clauseHint {
                        Text(clause)
                            .font(.caption)
                            .bold()
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    if let pa = change.pageA {
                        Text("S. \(pa + 1)")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }

                if let textA = change.textA, !textA.isEmpty {
                    Text(textA)
                        .font(.caption)
                        .strikethrough()
                        .foregroundColor(.red)
                        .lineLimit(2)
                }

                if let textB = change.textB, !textB.isEmpty {
                    Text(textB)
                        .font(.caption)
                        .underline()
                        .foregroundColor(.green)
                        .lineLimit(2)
                }
            }
            .padding(8)
            .background(isSelected ? Color.accentColor.opacity(0.15) : Color.clear)
            .cornerRadius(6)
        }
        .buttonStyle(.plain)
    }

    private var operationBadge: some View {
        switch change.operation {
        case .insert:
            return Text("Einfügung")
                .font(.caption2)
                .bold()
                .foregroundColor(.green)
        case .delete:
            return Text("Löschung")
                .font(.caption2)
                .bold()
                .foregroundColor(.red)
        case .replace:
            return Text("Ersetzung")
                .font(.caption2)
                .bold()
                .foregroundColor(.orange)
        case .move:
            return Text("Verschoben")
                .font(.caption2)
                .bold()
                .foregroundColor(.blue)
        case .unchanged:
            return Text("Identisch")
                .font(.caption2)
                .bold()
                .foregroundColor(.secondary)
        }
    }
}
