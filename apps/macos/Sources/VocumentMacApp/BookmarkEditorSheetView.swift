import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// In-memory node representation for hierarchical bookmark tree editing (`ROADMAP.md` M3.4).
@MainActor
struct BookmarkNode: Identifiable, Equatable {
    private static var nextId: Int = 1
    static func allocateId() -> Int {
        let val = nextId
        nextId += 1
        return val
    }

    let id: Int
    var title: String
    var targetPage: Int
    var children: [BookmarkNode]

    init(id: Int, title: String, targetPage: Int, children: [BookmarkNode]) {
        self.id = id
        self.title = title
        self.targetPage = targetPage
        self.children = children
    }

    static func from(ffi: FfiBookmarkItem) -> BookmarkNode {
        BookmarkNode(
            id: BookmarkNode.allocateId(),
            title: ffi.title,
            targetPage: Int(ffi.targetPage + 1),
            children: ffi.children.map { BookmarkNode.from(ffi: $0) }
        )
    }

    func toFfi() -> FfiBookmarkItem {
        FfiBookmarkItem(
            title: title.isEmpty ? "Bookmark" : title,
            targetPage: UInt32(max(0, targetPage - 1)),
            children: children.map { $0.toFfi() }
        )
    }
}

/// Helper structure for linear display of tree nodes with hierarchy depth.
struct BookmarkFlatItem: Identifiable {
    let id: Int
    let node: BookmarkNode
    let depth: Int
}

/// Native SwiftUI dialog for hierarchical bookmark (outline) editing (`ROADMAP.md` M3.4).
struct BookmarkEditorSheetView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    @State private var nodes: [BookmarkNode] = []
    @State private var selectedId: Int?
    @State private var editTitle: String = ""
    @State private var editPage: Int = 1

    var body: some View {
        VStack(spacing: 16) {
            Text("Document Bookmarks & Outline")
                .font(.headline)

            Text("Hierarchical navigation tree for readers and tables of contents")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            // Outline tree list
            List(selection: $selectedId) {
                if nodes.isEmpty {
                    Text("No bookmarks in document. Click 'Add Sibling' below.")
                        .foregroundStyle(.secondary)
                        .italic()
                } else {
                    ForEach(flattened(nodes: nodes, depth: 0)) { item in
                        BookmarkTreeRowView(
                            title: item.node.title,
                            page: item.node.targetPage,
                            depth: item.depth,
                            isSelected: selectedId == item.node.id
                        )
                        .tag(item.node.id)
                    }
                }
            }
            .listStyle(.inset)
            .frame(height: 220)

            // Hierarchy and reorder toolbar
            HStack(spacing: 8) {
                Button {
                    indentSelected()
                } label: {
                    Label("Indent", systemImage: "increase.indent")
                }
                .disabled(selectedId == nil || !canIndent(selectedId))
                .help("Make selected bookmark a child of preceding bookmark")

                Button {
                    outdentSelected()
                } label: {
                    Label("Outdent", systemImage: "decrease.indent")
                }
                .disabled(selectedId == nil || !canOutdent(selectedId))
                .help("Promote selected bookmark up one hierarchy level")

                Button {
                    moveSelected(up: true)
                } label: {
                    Image(systemName: "arrow.up")
                }
                .disabled(selectedId == nil || !canMove(selectedId, up: true))
                .help("Move bookmark up among siblings")

                Button {
                    moveSelected(up: false)
                } label: {
                    Image(systemName: "arrow.down")
                }
                .disabled(selectedId == nil || !canMove(selectedId, up: false))
                .help("Move bookmark down among siblings")

                Spacer()

                Button {
                    addChildBookmark()
                } label: {
                    Label("Add Child", systemImage: "arrow.turn.down.right")
                }
                .disabled(selectedId == nil)
                .help("Add a child bookmark under the selected item")

                Button {
                    addSiblingBookmark()
                } label: {
                    Label("Add Sibling", systemImage: "plus")
                }
                .help("Add a bookmark at the current hierarchy level")

                if selectedId != nil {
                    Button(role: .destructive) {
                        deleteSelected()
                    } label: {
                        Image(systemName: "trash")
                    }
                    .help("Delete selected bookmark and any children")
                }
            }
            .padding(.horizontal, 8)

            // Selected node editing fields
            if selectedId != nil {
                HStack(spacing: 12) {
                    TextField("Title", text: $editTitle)
                        .textFieldStyle(.roundedBorder)
                        .onChange(of: editTitle) { _, newTitle in
                            updateSelectedNode(title: newTitle, page: editPage)
                        }

                    HStack(spacing: 4) {
                        Text("Page:")
                        TextField("Page", value: $editPage, format: .number)
                            .frame(width: 50)
                            .textFieldStyle(.roundedBorder)
                            .onChange(of: editPage) { _, newPage in
                                updateSelectedNode(title: editTitle, page: newPage)
                            }
                    }
                    .font(.callout)
                }
                .padding(.horizontal, 8)
            }

            Divider()

            HStack {
                Button("Cancel") {
                    coordinator.showBookmarksSheet = false
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Save Bookmarks") {
                    coordinator.showBookmarksSheet = false
                    let ffiItems = nodes.map { $0.toFfi() }
                    coordinator.workbenchSetBookmarks(bookmarks: ffiItems)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
        .frame(width: 540, height: 480)
        .onAppear {
            let loaded = coordinator.workbenchLoadBookmarks()
            nodes = loaded.map { BookmarkNode.from(ffi: $0) }
            editPage = Int(coordinator.currentPageIndex + 1)
        }
        .onChange(of: selectedId) { _, newId in
            if let id = newId, let found = findNode(in: nodes, id: id) {
                editTitle = found.title
                editPage = found.targetPage
            }
        }
    }

    // MARK: - Tree Operations

    private func flattened(nodes: [BookmarkNode], depth: Int) -> [BookmarkFlatItem] {
        var result: [BookmarkFlatItem] = []
        for node in nodes {
            result.append(BookmarkFlatItem(id: node.id, node: node, depth: depth))
            result.append(contentsOf: flattened(nodes: node.children, depth: depth + 1))
        }
        return result
    }

    private func findNode(in list: [BookmarkNode], id: Int) -> BookmarkNode? {
        for node in list {
            if node.id == id { return node }
            if let child = findNode(in: node.children, id: id) { return child }
        }
        return nil
    }

    private func updateSelectedNode(title: String, page: Int) {
        guard let id = selectedId else { return }
        nodes = updateInTree(list: nodes, id: id, title: title, page: page)
    }

    private func updateInTree(list: [BookmarkNode], id: Int, title: String, page: Int) -> [BookmarkNode] {
        list.map { node in
            if node.id == id {
                var updated = node
                updated.title = title
                updated.targetPage = max(1, page)
                return updated
            } else {
                var updated = node
                updated.children = updateInTree(list: node.children, id: id, title: title, page: page)
                return updated
            }
        }
    }

    private func addSiblingBookmark() {
        let defaultPage = max(1, editPage)
        let newBookmark = BookmarkNode(
            id: BookmarkNode.allocateId(),
            title: "Bookmark \(nodes.count + 1)",
            targetPage: defaultPage,
            children: []
        )
        if let id = selectedId {
            nodes = insertSibling(in: nodes, targetId: id, newNode: newBookmark)
        } else {
            nodes.append(newBookmark)
        }
        selectedId = newBookmark.id
    }

    private func addChildBookmark() {
        guard let id = selectedId else { return }
        let defaultPage = max(1, editPage)
        let newBookmark = BookmarkNode(
            id: BookmarkNode.allocateId(),
            title: "Sub-bookmark",
            targetPage: defaultPage,
            children: []
        )
        nodes = appendChild(in: nodes, targetId: id, child: newBookmark)
        selectedId = newBookmark.id
    }

    private func insertSibling(in list: [BookmarkNode], targetId: Int, newNode: BookmarkNode) -> [BookmarkNode] {
        var result: [BookmarkNode] = []
        for node in list {
            result.append(node)
            if node.id == targetId {
                result.append(newNode)
            } else {
                result[result.count - 1].children = insertSibling(in: node.children, targetId: targetId, newNode: newNode)
            }
        }
        return result
    }

    private func appendChild(in list: [BookmarkNode], targetId: Int, child: BookmarkNode) -> [BookmarkNode] {
        list.map { node in
            if node.id == targetId {
                var updated = node
                updated.children.append(child)
                return updated
            } else {
                var updated = node
                updated.children = appendChild(in: node.children, targetId: targetId, child: child)
                return updated
            }
        }
    }

    private func deleteSelected() {
        guard let id = selectedId else { return }
        nodes = removeFromTree(list: nodes, id: id)
        selectedId = nil
    }

    private func removeFromTree(list: [BookmarkNode], id: Int) -> [BookmarkNode] {
        list.compactMap { node in
            if node.id == id { return nil }
            var updated = node
            updated.children = removeFromTree(list: node.children, id: id)
            return updated
        }
    }

    private func canIndent(_ id: Int?) -> Bool {
        guard let id = id else { return false }
        return canIndentInList(nodes, targetId: id)
    }

    private func canIndentInList(_ list: [BookmarkNode], targetId: Int) -> Bool {
        for (i, node) in list.enumerated() {
            if node.id == targetId {
                return i > 0
            }
            if canIndentInList(node.children, targetId: targetId) {
                return true
            }
        }
        return false
    }

    private func indentSelected() {
        guard let id = selectedId else { return }
        nodes = indentInList(nodes, targetId: id)
    }

    private func indentInList(_ list: [BookmarkNode], targetId: Int) -> [BookmarkNode] {
        var copy = list
        for i in 0..<copy.count {
            if copy[i].id == targetId && i > 0 {
                let item = copy.remove(at: i)
                copy[i - 1].children.append(item)
                return copy
            }
            copy[i].children = indentInList(copy[i].children, targetId: targetId)
        }
        return copy
    }

    private func canOutdent(_ id: Int?) -> Bool {
        guard let id = id else { return false }
        for node in nodes {
            if node.children.contains(where: { $0.id == id }) {
                return true
            }
            if canOutdentInSubtree(node.children, targetId: id) {
                return true
            }
        }
        return false
    }

    private func canOutdentInSubtree(_ list: [BookmarkNode], targetId: Int) -> Bool {
        for node in list {
            if node.children.contains(where: { $0.id == targetId }) {
                return true
            }
            if canOutdentInSubtree(node.children, targetId: targetId) {
                return true
            }
        }
        return false
    }

    private func outdentSelected() {
        guard let id = selectedId else { return }
        nodes = outdentInList(nodes, targetId: id).list
    }

    private func outdentInList(_ list: [BookmarkNode], targetId: Int) -> (list: [BookmarkNode], extracted: BookmarkNode?) {
        var copy = list
        for i in 0..<copy.count {
            for j in 0..<copy[i].children.count {
                if copy[i].children[j].id == targetId {
                    let extracted = copy[i].children.remove(at: j)
                    copy.insert(extracted, at: i + 1)
                    return (copy, nil)
                }
            }
            let res = outdentInList(copy[i].children, targetId: targetId)
            copy[i].children = res.list
            if let ext = res.extracted {
                copy.insert(ext, at: i + 1)
                return (copy, nil)
            }
        }
        return (copy, nil)
    }

    private func canMove(_ id: Int?, up: Bool) -> Bool {
        guard let id = id else { return false }
        return canMoveInList(nodes, targetId: id, up: up)
    }

    private func canMoveInList(_ list: [BookmarkNode], targetId: Int, up: Bool) -> Bool {
        for (i, node) in list.enumerated() {
            if node.id == targetId {
                return up ? (i > 0) : (i < list.count - 1)
            }
            if canMoveInList(node.children, targetId: targetId, up: up) {
                return true
            }
        }
        return false
    }

    private func moveSelected(up: Bool) {
        guard let id = selectedId else { return }
        nodes = moveInList(nodes, targetId: id, up: up)
    }

    private func moveInList(_ list: [BookmarkNode], targetId: Int, up: Bool) -> [BookmarkNode] {
        var copy = list
        for i in 0..<copy.count {
            if copy[i].id == targetId {
                let targetIdx = up ? i - 1 : i + 1
                if targetIdx >= 0 && targetIdx < copy.count {
                    copy.swapAt(i, targetIdx)
                }
                return copy
            }
            copy[i].children = moveInList(copy[i].children, targetId: targetId, up: up)
        }
        return copy
    }
}

/// Row view for outline hierarchy displaying depth and target page.
struct BookmarkTreeRowView: View {
    let title: String
    let page: Int
    let depth: Int
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 6) {
            if depth > 0 {
                Spacer().frame(width: CGFloat(depth * 18))
            }
            Image(systemName: depth == 0 ? "bookmark.fill" : "bookmark")
                .foregroundStyle(isSelected ? Color.primary : Color.blue)
                .font(.caption)
            Text(title)
                .fontWeight(depth == 0 ? .medium : .regular)
            Spacer()
            Text("Page \(page)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .contentShape(Rectangle())
    }
}
