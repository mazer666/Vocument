import SwiftUI
import AppKit

/// Offline Help and User Manual Viewer (`HELP-001`, `HELP-002`, `ROADMAP.md` M3.6).
///
/// Provides bundled, 100% offline access to the complete Vocument documentation,
/// feature workflows, keyboard shortcuts, and honest security threat model.
struct HelpTopic: Identifiable {
    let id: String
    let title: String
    let category: String
    let content: String
    let shortcuts: [String]
}

struct HelpSheetView: View {
    @Binding var isPresented: Bool
    @State private var searchQuery: String = ""
    @State private var selectedTopicId: String = "workbench_overview"

    private var topics: [HelpTopic] {
        [
            HelpTopic(
                id: "workbench_overview",
                title: "Page Workbench Overview",
                category: "Page Workbench",
                content: """
                The Page Workbench provides non-destructive, visual manipulation of PDF document structure.

                Key capabilities include:
                • Visual Thumbnail Grid with multi-page selection.
                • Page Reordering via drag-and-drop or Option-Arrow keyboard shortcuts.
                • Page Duplication with AcroForm field de-duplication.
                • Page Deletion with fail-closed Last-Page Protection.
                • Lossless Page Extraction into standalone PDF files.
                • Digital Signature Preflight Safeguard: signed documents refuse mutations unless signature invalidation is explicitly confirmed.
                """,
                shortcuts: ["⌥↑ / ⌥↓: Reorder page", "⌘D: Duplicate page", "⌫: Delete page", "⌘A: Select all"]
            ),
            HelpTopic(
                id: "bates_and_stamps",
                title: "Bates Numbering & Stamps",
                category: "Legal & Stamping",
                content: """
                Legal Bates numbering and stamp overlay operations (M3.4):

                • Bates Numbering: sequential numeric sequences with configurable prefix, digit padding, suffix, starting index, and 6 standard layout positions.
                • Multi-Zone Headers & Footers: 6 layout zones with dynamic tokens for page count, date, document title, and Bates sequence.
                • Hierarchical Bookmarks: visual outline tree editor supporting title edits, indent, outdent, and page target anchors.
                • Rubber Stamps & Watermarks: presets ('DRAFT', 'CONFIDENTIAL', 'APPROVED') and custom text with rotation and alpha transparency.
                """,
                shortcuts: ["Bates: Toolbar > Bates", "Stamps: Toolbar > Stamp"]
            ),
            HelpTopic(
                id: "optimization_preflight",
                title: "Optimization & Preflight",
                category: "Production & Export",
                content: """
                Document optimization, asset extraction, and preflight audit (M3.5):

                • Lossless Image Extraction: extracts raw embedded DCT (JPEG) and JPX (JPEG 2000) streams directly without re-compression degradation.
                • Stream Compaction: reduces file size via duplicate font/stream deduplication and ISO 32000-1 compressed object streams. Does not alter content streams (do_clean = 0).
                • Restricted Structural Preflight: audits font embedding across /FontDescriptor dictionaries and identifies transparency groups (/ExtGState /BM, /ca < 1, /SMask) with explicit legal disclaimers.
                """,
                shortcuts: ["Optimize: Toolbar > Optimize", "Preflight: Toolbar > Preflight"]
            ),
            HelpTopic(
                id: "security_threat_model",
                title: "Security & Threat Model",
                category: "Security & Privacy",
                content: """
                Honest Security and Privacy Architecture (ROLE-09, ROLE-10):

                • 100% Offline Default: Zero network telemetry, zero cloud calls, zero external analytics.
                • Crash Recovery Integrity: Persistent transaction journals employ a deterministic unkeyed SHA-256 digest chain covering commands and revision counters. This guarantees detection of accidental file corruption, bitrot, and interrupted writes.
                • Honest Threat Boundary: Unkeyed SHA-256 is NOT an HMAC and does NOT resist an active attacker with local write access. Confidentiality and access control rely on operating system App Sandbox boundaries, POSIX permissions, and full-disk encryption (FileVault / LUKS).
                • Storage Minimization: Persistent journals strip raw binary document dumps from inverse commands to avoid plaintext disk leakage.
                """,
                shortcuts: ["⌘Z: Undo operation", "⇧⌘Z: Redo operation"]
            ),
            HelpTopic(
                id: "shortcuts_reference",
                title: "Keyboard Shortcuts Reference",
                category: "Productivity",
                content: """
                Standard Cocoa and Vocument keybindings:

                • ⌘O: Open PDF file
                • ⌘Z: Undo last transaction
                • ⇧⌘Z: Redo transaction
                • ⌘D: Duplicate selected page
                • ⌫: Delete selected pages (protected against last page)
                • ⌥↑: Move selected page up
                • ⌥↓: Move selected page down
                • ⌘?: Open this Offline Help (HELP-002)
                • ⌘+: Zoom in
                • ⌘-: Zoom out
                • ⌘0: Reset zoom to 100%
                """,
                shortcuts: ["⌘?: Open Help", "⌘O: Open PDF", "⌘Z: Undo", "⇧⌘Z: Redo", "⌘D: Duplicate", "⌫: Delete"]
            )
        ]
    }

    private var filteredTopics: [HelpTopic] {
        if searchQuery.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return topics
        }
        let q = searchQuery.lowercased()
        return topics.filter {
            $0.title.lowercased().contains(q)
                || $0.content.lowercased().contains(q)
                || $0.category.lowercased().contains(q)
                || $0.shortcuts.contains { $0.lowercased().contains(q) }
        }
    }

    private var selectedTopic: HelpTopic? {
        topics.first { $0.id == selectedTopicId } ?? topics.first
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Image(systemName: "questionmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(.tint)
                Text("Vocument Offline-Hilfe (HELP-002)")
                    .font(.title2)
                    .bold()
                Spacer()
                Button("Fertig") {
                    isPresented = false
                }
                .keyboardShortcut(.defaultAction)
                .accessibilityLabel("Hilfe schließen")
            }
            .padding(16)
            .background(Color(NSColor.windowBackgroundColor))

            Divider()

            // Main Content Area
            HStack(spacing: 0) {
                // Topic list / search
                VStack(spacing: 8) {
                    TextField("Themen suchen…", text: $searchQuery)
                        .textFieldStyle(.roundedBorder)
                        .padding([.top, .horizontal], 10)
                        .accessibilityLabel("Hilfethemen suchen")

                    List(filteredTopics) { topic in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(topic.title)
                                .font(.headline)
                            Text(topic.category)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .padding(.vertical, 4)
                        .contentShape(Rectangle())
                        .onTapGesture {
                            selectedTopicId = topic.id
                        }
                    }
                    .listStyle(.sidebar)
                }
                .frame(minWidth: 220, maxWidth: 300)

                Divider()

                // Topic Content
                ScrollView {
                    if let topic = selectedTopic {
                        VStack(alignment: .leading, spacing: 16) {
                            Text(topic.title)
                                .font(.title)
                                .bold()
                            Text(topic.category)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)

                            Divider()

                            Text(topic.content)
                                .font(.body)
                                .lineSpacing(4)

                            if !topic.shortcuts.isEmpty {
                                Text("Tastaturkürzel:")
                                    .font(.headline)
                                    .padding(.top, 8)

                                ForEach(topic.shortcuts, id: \.self) { shortcut in
                                    HStack {
                                        Image(systemName: "command")
                                            .foregroundStyle(.secondary)
                                        Text(shortcut)
                                            .font(.system(.body, design: .monospaced))
                                    }
                                }
                            }
                        }
                        .padding(24)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .frame(minWidth: 400)
                .background(Color(NSColor.textBackgroundColor))
            }
        }
        .frame(minWidth: 700, minHeight: 480)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Vocument Offline-Hilfe")
        .accessibilityIdentifier("help_sheet_view")
    }
}
