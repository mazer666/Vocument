import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Native SwiftUI dialog for hierarchical bookmark (outline) editing (`ROADMAP.md` M3.4).
struct BookmarkEditorSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var items: [FfiBookmarkItem] = []
    @State private var selectedIndex: Int?
    @State private var newTitle: String = ""
    @State private var newPage: Int = 1

    var body: some View {
        VStack(spacing: 16) {
            Text("Document Bookmarks & Outline")
                .font(.headline)

            Text("Hierarchical navigation tree for readers and tables of contents")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            // Outline tree list
            List(selection: $selectedIndex) {
                if items.isEmpty {
                    Text("No bookmarks in document. Add one below.")
                        .foregroundStyle(.secondary)
                        .italic()
                } else {
                    ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                        BookmarkRowView(item: item, depth: 0)
                            .tag(index)
                    }
                }
            }
            .listStyle(.inset)
            .frame(height: 220)

            // Edit controls for selected or new
            HStack(spacing: 8) {
                TextField("Bookmark Title", text: $newTitle)

                HStack {
                    Text("Page:")
                    TextField("Page", value: $newPage, format: .number)
                        .frame(width: 45)
                }
                .font(.callout)

                Button {
                    let pageIdx = UInt32(max(0, newPage - 1))
                    let newItem = FfiBookmarkItem(
                        title: newTitle.isEmpty ? "Bookmark \(items.count + 1)" : newTitle,
                        targetPage: pageIdx,
                        children: []
                    )
                    items.append(newItem)
                    newTitle = ""
                } label: {
                    Label("Add", systemImage: "plus")
                }

                if let sel = selectedIndex, sel < items.count {
                    Button(role: .destructive) {
                        items.remove(at: sel)
                        selectedIndex = nil
                    } label: {
                        Image(systemName: "trash")
                    }
                    .help("Delete selected bookmark")
                }
            }
            .padding(.horizontal, 8)

            Divider()

            HStack {
                Button("Cancel") {
                    coordinator.showBookmarksSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Save Bookmarks") {
                    coordinator.showBookmarksSheet = false
                    coordinator.workbenchSetBookmarks(bookmarks: items)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 500, height: 420)
        .onAppear {
            items = coordinator.workbenchLoadBookmarks()
            newPage = Int(coordinator.currentPageIndex + 1)
        }
    }
}

/// Recursive row view for outline hierarchy.
struct BookmarkRowView: View {
    let item: FfiBookmarkItem
    let depth: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                if depth > 0 {
                    Spacer().frame(width: CGFloat(depth * 16))
                }
                Image(systemName: "bookmark.fill")
                    .foregroundStyle(.blue)
                    .font(.caption)
                Text(item.title)
                    .fontWeight(.medium)
                Spacer()
                Text("Page \(item.targetPage + 1)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            ForEach(Array(item.children.enumerated()), id: \.offset) { _, child in
                BookmarkRowView(item: child, depth: depth + 1)
            }
        }
    }
}
