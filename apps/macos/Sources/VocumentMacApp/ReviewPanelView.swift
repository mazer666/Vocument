import SwiftUI
import AppKit
import VocumentMacKit
import VocumentFFI

/// The review panel of a document (`M2`, `ANNOT-001`, `LINKSEC-001`,
/// `ATTACH-001`, `SIGREV-001`, `REDACT-002`).
///
/// The panel is the place where the findings of the core reach the user:
/// the links and the actions of the document, the attachments, the
/// signatures, the comments of the reviewer with the rail of the profession,
/// and — first of all — the **warnings of a redaction**.
///
/// The warnings are not decoration. A redaction removes what the user
/// marked; the same text can stand somewhere else in the document, and a
/// place that could not be read at all is not a proof that the text is gone.
/// A user who sees only "the redaction succeeded" would believe that the
/// word is gone from the document while it is still in it. The panel
/// therefore shows the warnings in red, above everything else.
struct ReviewPanelView: View {
    @ObservedObject var coordinator: MacDocumentSessionCoordinator

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if !coordinator.securityWarnings.isEmpty {
                GroupBox("Warnings — the document is not free of the marked text") {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(coordinator.securityWarnings, id: \.self) { warning in
                            Label(warning, systemImage: "exclamationmark.triangle.fill")
                                .foregroundStyle(.red)
                                .font(.callout)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

            GroupBox("Findings of the document") {
                VStack(alignment: .leading, spacing: 4) {
                    if let inspection = coordinator.reviewInspection {
                        Text(
                            "Risky: \(inspection.riskyCount) · "
                            + "leaving the document: \(inspection.externalCount) · "
                            + "signed: \(inspection.isSigned ? "yes" : "no")"
                        )
                        .font(.caption2)
                        .foregroundStyle(.secondary)

                        ForEach(inspection.actions.indices, id: \.self) { index in
                            let action = inspection.actions[index]
                            if action.isRisky {
                                Label(
                                    "Action (\(String(describing: action.kind))): \(action.target)",
                                    systemImage: "exclamationmark.triangle"
                                )
                                .font(.caption)
                            }
                        }
                        ForEach(inspection.attachments.indices, id: \.self) { index in
                            let attachment = inspection.attachments[index]
                            if attachment.isRisky {
                                Label(
                                    "Attachment: \(attachment.fileName) (\(attachment.mimeType))",
                                    systemImage: "exclamationmark.triangle"
                                )
                                .font(.caption)
                            }
                        }
                        ForEach(inspection.signatures.indices, id: \.self) { index in
                            let signature = inspection.signatures[index]
                            if signature.changedSinceSigning {
                                Label(
                                    "Signature \(signature.fieldName) was changed after it was signed",
                                    systemImage: "exclamationmark.triangle"
                                )
                                .font(.caption)
                            }
                        }
                    } else {
                        Text("No inspection yet.")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Redaction") {
                VStack(alignment: .leading, spacing: 6) {
                    if let selection = coordinator.redactionSelection {
                        Text("Marked place:")
                            .font(.caption2)
                            .bold()
                        Text(
                            "x \(selection.x0MilliPt)–\(selection.x1MilliPt) · "
                            + "y \(selection.y0MilliPt)–\(selection.y1MilliPt) (millipoints)"
                        )
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        if coordinator.redactionSelectionText.isEmpty {
                            Text("No text lies in the marked place.")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        } else {
                            Text(coordinator.redactionSelectionText)
                                .font(.caption)
                                .lineLimit(6)
                        }
                        Button("Remove the marked content for good") {
                            coordinator.redactSelectedRegions()
                        }
                        .font(.caption)
                        .buttonStyle(.borderedProminent)
                        Button("Mark another place") {
                            coordinator.clearRedactionSelection()
                        }
                        .font(.caption2)
                    } else {
                        Text(
                            "Drag on the page to mark a place. The marked text is shown "
                            + "here before it is removed."
                        )
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    }
                    Text(
                        "The removal cannot be undone. If the same text stands "
                        + "somewhere else in the document, the warning above names the place."
                    )
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Comments of the reviewer") {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(coordinator.pageComments, id: \.annotationId) { comment in
                        HStack(alignment: .top, spacing: 6) {
                            Circle()
                                .fill(
                                    Color(
                                        red: Double(comment.colorR) / 255.0,
                                        green: Double(comment.colorG) / 255.0,
                                        blue: Double(comment.colorB) / 255.0
                                    )
                                )
                                .frame(width: 10, height: 10)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(comment.contents)
                                    .font(.caption)
                                Text(
                                    comment.paletteKey.isEmpty
                                        ? (comment.author ?? "without author")
                                        : "\(comment.paletteKey) · \(comment.author ?? "without author")"
                                )
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("×") {
                                coordinator.removeComment(comment)
                            }
                            .font(.caption2)
                            .help("Take the comment back")
                        }
                    }

                    Divider()

                    HStack(spacing: 6) {
                        Picker("Rail", selection: $coordinator.commentPaletteKey) {
                            ForEach(coordinator.juristPalette, id: \.key) { entry in
                                Text(entry.label).tag(entry.key)
                            }
                        }
                        .labelsHidden()
                        .frame(minWidth: 140)
                    }

                    HStack(spacing: 6) {
                        TextField("Comment…", text: $coordinator.commentDraft)
                            .textFieldStyle(.roundedBorder)
                            .onSubmit {
                                coordinator.addCommentToCurrentPage()
                            }
                        Button("Add") {
                            coordinator.addCommentToCurrentPage()
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            Spacer()
        }
        .frame(width: 320)
    }
}
