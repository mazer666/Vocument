import SwiftUI
import AppKit
import CoreGraphics
import VocumentMacKit
import VocumentFFI

/// Native macOS sidebar for M0.3 Thumbnails, Logical Page Labels, Go-To Navigation,
/// Progressive SQLite FTS5 Search, and Cleaned Plain-Text Copy (`PAGEVIEW-002`, `PAGELABEL-001`..`003`, `SEARCH-002`, `OCRCOPY-002`).
struct NavigationAndSearchSidebarView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupBox("Jump to Page / Label") {
                HStack(spacing: 6) {
                    TextField("e.g. ii, 1, or #2", text: $coordinator.navigationInput)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit {
                            coordinator.jumpToQuery()
                        }
                    Button("Go") {
                        coordinator.jumpToQuery()
                    }
                }
                .padding(4)
            }

            GroupBox("Classic Search (SQLite FTS5)") {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        TextField("Exact search…", text: $coordinator.searchQuery)
                            .textFieldStyle(.roundedBorder)
                            .onSubmit {
                                coordinator.performSearch()
                            }
                        Button("Find") {
                            coordinator.performSearch()
                        }
                    }

                    if let progress = coordinator.searchProgress {
                        HStack {
                            Text(
                                "Indexed \(progress.indexedPages)/\(progress.totalPages) pages" +
                                    (progress.isComplete ? " (Complete)" : " (Progressive…)")
                            )
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            Spacer()
                            if !progress.isComplete {
                                Button("Index More") {
                                    coordinator.indexNextBatch()
                                }
                                .font(.caption2)
                            }
                        }
                    }

                    if !coordinator.searchHits.isEmpty {
                        ScrollView {
                            VStack(alignment: .leading, spacing: 4) {
                                ForEach(Array(coordinator.searchHits.enumerated()), id: \.offset) { _, hit in
                                    Button {
                                        coordinator.jumpToPageIndex(hit.pageIndex)
                                    } label: {
                                        VStack(alignment: .leading, spacing: 2) {
                                            Text("Label \(hit.logicalPageLabel) (Page #\(hit.physicalPageNumber)) • \(hit.matchCount)x")
                                                .font(.caption)
                                                .bold()
                                            Text(hit.snippet)
                                                .font(.caption2)
                                                .lineLimit(2)
                                                .foregroundStyle(.secondary)
                                        }
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                    }
                                    .buttonStyle(.plain)
                                    Divider()
                                }
                            }
                        }
                        .frame(maxHeight: 130)
                    }
                }
                .padding(4)
            }

            GroupBox("Text Selection & Copy (Page \(coordinator.currentLogicalLabel))") {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Button("Copy Cleaned Text") {
                            coordinator.copyCurrentPageText(cleaned: true)
                        }
                        Button("Copy Raw Text") {
                            coordinator.copyCurrentPageText(cleaned: false)
                        }
                    }
                    if let preview = coordinator.selectedTextPreview, !preview.isEmpty {
                        Text(preview)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .lineLimit(4)
                    }
                }
                .padding(4)
            }

            Text("Thumbnails")
                .font(.headline)

            ScrollView {
                LazyVStack(spacing: 10) {
                    ForEach(coordinator.pageDescriptors, id: \.pageIndex) { desc in
                        Button {
                            coordinator.jumpToPageIndex(desc.pageIndex)
                        } label: {
                            VStack(spacing: 4) {
                                if let thumbImg = coordinator.thumbnailImages[desc.pageIndex] {
                                    Image(decorative: thumbImg, scale: 1.0, orientation: .up)
                                        .background(Color.white)
                                        .overlay(
                                            RoundedRectangle(cornerRadius: 4)
                                                .stroke(
                                                    desc.pageIndex == coordinator.currentPageIndex
                                                        ? Color.accentColor
                                                        : Color.secondary.opacity(0.3),
                                                    lineWidth: desc.pageIndex == coordinator.currentPageIndex ? 2 : 1
                                                )
                                        )
                                }
                                Text("\(desc.logicalLabel) (#\(desc.physicalPageNumber))")
                                    .font(.caption)
                                    .monospacedDigit()
                            }
                            .padding(4)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .frame(width: 260)
    }
}
