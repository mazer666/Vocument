import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// Visual Page Workbench for native single-document page restructuring (`ROADMAP.md` M3.1, `ADR-0019`).
/// Provides asynchronous thumbnail grid, multi-selection, drag/reorder, duplication,
/// deletion with last-page guard, rotation, extraction, Undo/Redo, and preflight alerts.
struct PageWorkbenchView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    private let gridColumns = [GridItem(.adaptive(minimum: 160, maximum: 220), spacing: 24)]

    var body: some View {
        VStack(spacing: 0) {
            workbenchToolbar
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(Color(NSColor.windowBackgroundColor))

            Divider()

            if coordinator.isWorkbenchProcessing {
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                    Text(coordinator.workbenchProcessingStatus ?? "Processing document…")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 6)
                .background(Color.blue.opacity(0.10))
            }

            if let preflight = coordinator.workbenchPreflightReport {
                preflightNoticeBanner(preflight)
            }

            if let error = coordinator.errorMessage {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                    Text(error)
                        .font(.callout)
                        .foregroundStyle(.primary)
                    Spacer()
                    Button {
                        coordinator.errorMessage = nil
                    } label: {
                        Image(systemName: "xmark.circle")
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 6)
                .background(Color.red.opacity(0.12))
            }

            if let notification = coordinator.workbenchNotificationMessage {
                HStack {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                    Text(notification)
                        .font(.callout)
                    Spacer()
                    Button {
                        coordinator.workbenchNotificationMessage = nil
                    } label: {
                        Image(systemName: "xmark.circle")
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 6)
                .background(Color.green.opacity(0.12))
            }

            ScrollView {
                LazyVGrid(columns: gridColumns, spacing: 24) {
                    ForEach(coordinator.pageDescriptors, id: \.pageIndex) { desc in
                        thumbnailCard(for: desc)
                    }
                }
                .padding(24)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .confirmationDialog(
            "Invalidate Digital Signatures?",
            isPresented: $coordinator.showSignatureConfirmation,
            titleVisibility: .visible
        ) {
            Button("Invalidate & Proceed", role: .destructive) {
                coordinator.confirmAndExecutePendingAction()
            }
            Button("Cancel", role: .cancel) {
                coordinator.cancelPendingAction()
            }
        } message: {
            Text("This document contains digital signatures. Modifying page sequence or content will invalidate the cryptographic signatures per ISO 32000-1 §12.8, creating an unsigned working copy.")
        }
        .confirmationDialog(
            "Delete Selected Pages?",
            isPresented: $coordinator.showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                coordinator.workbenchDeleteSelectedPages()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Are you sure you want to delete \(coordinator.selectedWorkbenchPages.count) page(s)? This action can be undone via Edit > Undo.")
        }
        .sheet(isPresented: $coordinator.showCropSheet) {
            CropSheetView(coordinator: coordinator)
        }
        .onAppear {
            coordinator.refreshWorkbenchPreflight()
        }
    }

    // MARK: - Toolbar

    private var workbenchToolbar: some View {
        HStack(spacing: 12) {
            // Undo / Redo
            Button {
                coordinator.workbenchUndo()
            } label: {
                Label("Undo", systemImage: "arrow.uturn.backward")
            }
            .disabled(coordinator.transactionHistory?.canUndo != true)
            .help("Undo the last page operation (Cmd+Z)")

            Button {
                coordinator.workbenchRedo()
            } label: {
                Label("Redo", systemImage: "arrow.uturn.forward")
            }
            .disabled(coordinator.transactionHistory?.canRedo != true)
            .help("Redo the last undone page operation (Cmd+Shift+Z)")

            Divider().frame(height: 18)

            // Reorder
            Button {
                coordinator.workbenchMoveSelectedPages(delta: -1)
            } label: {
                Image(systemName: "arrow.left")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .keyboardShortcut("[", modifiers: .command)
            .help("Move selected page(s) left (Cmd+[)")

            Button {
                coordinator.workbenchMoveSelectedPages(delta: 1)
            } label: {
                Image(systemName: "arrow.right")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .keyboardShortcut("]", modifiers: .command)
            .help("Move selected page(s) right (Cmd+])")

            Divider().frame(height: 18)

            // Page Actions
            Button {
                coordinator.workbenchDuplicateSelectedPages()
            } label: {
                Label("Duplicate", systemImage: "plus.square.on.square")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .keyboardShortcut("d", modifiers: .command)
            .help("Duplicate selected page(s), cloning contents and generating unique field names (Cmd+D)")

            Button {
                coordinator.showDeleteConfirmation = true
            } label: {
                Label("Delete", systemImage: "trash")
            }
            .disabled(
                coordinator.selectedWorkbenchPages.isEmpty
                || coordinator.selectedWorkbenchPages.count >= (coordinator.activeSession?.pdfSummary.pageCount ?? 1)
            )
            .keyboardShortcut(.delete, modifiers: [])
            .help("Delete selected page(s) and re-anchor internal bookmarks and links (Delete)")

            Divider().frame(height: 18)

            // Rotation
            Button {
                coordinator.workbenchRotateSelectedPages(deltaDegrees: 270)
            } label: {
                Image(systemName: "rotate.left")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .keyboardShortcut("r", modifiers: [.command, .shift])
            .help("Rotate selected page(s) 90° counter-clockwise (Cmd+Shift+R)")

            Button {
                coordinator.workbenchRotateSelectedPages(deltaDegrees: 90)
            } label: {
                Image(systemName: "rotate.right")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .keyboardShortcut("r", modifiers: .command)
            .help("Rotate selected page(s) 90° clockwise (Cmd+R)")

            Divider().frame(height: 18)

            // Geometry, Crop & Deskew (M3.3, CROP-001, ROTATE-003)
            Button {
                coordinator.showCropSheet = true
            } label: {
                Label("Crop…", systemImage: "crop")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .help("Crop page boundaries (A4, Letter, Auto-Crop Margins, Custom) with Display or Hard Crop")

            Button {
                coordinator.workbenchDeskewSelectedPages()
            } label: {
                Label("Deskew", systemImage: "skew")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .help("Automatically detect page skew angle and apply fine vector rotation matrix")

            Divider().frame(height: 18)

            // Extraction & Multi-Document Operations (M3.1, M3.2)
            Button {
                coordinator.workbenchExtractSelectedPages()
            } label: {
                Label("Extract…", systemImage: "square.and.arrow.up")
            }
            .disabled(coordinator.selectedWorkbenchPages.isEmpty)
            .help("Extract selected page(s) into a new standalone PDF document without modifying source")

            Button {
                coordinator.workbenchInsertPages()
            } label: {
                Label("Insert…", systemImage: "arrow.down.doc")
            }
            .disabled(coordinator.activeSession == nil)
            .help("Insert pages from an external PDF document at current position (M3.2)")

            Button {
                coordinator.workbenchMergeDocuments()
            } label: {
                Label("Merge…", systemImage: "doc.on.doc")
            }
            .disabled(coordinator.activeSession == nil)
            .help("Merge one or more external PDF documents into current document (M3.2)")

            Button {
                coordinator.workbenchSplitDocument()
            } label: {
                Label("Split…", systemImage: "scissors")
            }
            .disabled((coordinator.activeSession?.pdfSummary.pageCount ?? 0) < 2)
            .help("Split document into standalone PDF parts by selection or into single pages (M3.2)")

            Spacer()

            // Selection Info & Select All
            Text("\(coordinator.selectedWorkbenchPages.count) of \(coordinator.activeSession?.pdfSummary.pageCount ?? 0) selected")
                .font(.caption)
                .foregroundStyle(.secondary)

            Button("Select All") {
                coordinator.workbenchSelectAllPages()
            }
            .font(.caption)
            .keyboardShortcut("a", modifiers: .command)
            .help("Select all pages in the document (Cmd+A)")
        }
    }

    // MARK: - Preflight Banner

    private func preflightNoticeBanner(_ report: FfiPageWorkbenchPreflightReport) -> some View {
        HStack(spacing: 12) {
            if report.isSigned {
                HStack(spacing: 6) {
                    Image(systemName: "lock.shield.fill")
                        .foregroundStyle(.orange)
                    Text("Digitally Signed Document: In-place mutations will invalidate signatures.")
                        .font(.caption)
                }
            }
            if report.hasTaggedPdf {
                HStack(spacing: 6) {
                    Image(systemName: "doc.text.fill")
                        .foregroundStyle(.blue)
                    Text("Tagged PDF: Structural elements & ParentTree maintained (PDF/UA re-validation recommended after page edits).")
                        .font(.caption)
                }
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
        .background(Color.yellow.opacity(0.12))
    }

    // MARK: - Thumbnail Card

    private func thumbnailCard(for desc: FfiPageDescriptor) -> some View {
        let isSelected = coordinator.selectedWorkbenchPages.contains(desc.pageIndex)

        return VStack(spacing: 8) {
            ZStack {
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color(NSColor.controlBackgroundColor))
                    .shadow(color: Color.black.opacity(isSelected ? 0.3 : 0.1), radius: isSelected ? 4 : 2, y: 1)

                if let cgImg = coordinator.thumbnailImages[desc.pageIndex] {
                    Image(decorative: cgImg, scale: 1.0)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .padding(6)
                } else {
                    VStack(spacing: 6) {
                        ProgressView().controlSize(.small)
                        Text("Rendering…")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .frame(width: 160, height: 210)
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(isSelected ? Color.accentColor : Color.secondary.opacity(0.2), lineWidth: isSelected ? 3 : 1)
            )

            // Page label and number
            HStack {
                Text("Page \(desc.pageIndex + 1)")
                    .font(.caption)
                    .bold()
                if !desc.logicalLabel.isEmpty && desc.logicalLabel != "\(desc.pageIndex + 1)" {
                    Text("(\(desc.logicalLabel))")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(6)
        .background(isSelected ? Color.accentColor.opacity(0.08) : Color.clear)
        .cornerRadius(8)
        .contentShape(Rectangle())
        .onTapGesture {
            let flags = NSEvent.modifierFlags
            if flags.contains(.command) {
                coordinator.workbenchTogglePageSelection(desc.pageIndex)
            } else if flags.contains(.shift) {
                coordinator.workbenchSelectRangeToPage(desc.pageIndex)
            } else {
                coordinator.workbenchSelectPage(desc.pageIndex)
            }
        }
        .help("Click to select, Cmd+Click to toggle, Shift+Click for range selection")
    }
}
