using System;
using System.Collections.Generic;
using System.Linq;

namespace Vocument.Windows.Native;

/// <summary>
/// Operations that the Windows shell can request from the shared Rust core.
/// </summary>
public enum CoreOperation
{
    DocumentOpen,
    PageGeometry,
    PageRender,
    TextSearch,
    TextSelection,
    HighlightAnnotation,
    PageRotation,
    TransactionUndo,
    TransactionRedo,
    SessionSave,
    RecoveryCheckpoint,
    PageReorder,
    PageDelete,
    PageExtract,
    PageStamp,
    FlowDocumentOpen,
    FlowPdfExport,
    DocumentCompare
}

/// <summary>
/// Describes how a core operation is reached from the Windows shell.
/// </summary>
/// <param name="Operation">The operation itself.</param>
/// <param name="RequiredEntryPoint">The C ABI entry point that implements it.</param>
/// <param name="Description">What the operation does in the core.</param>
public sealed record CoreOperationDescriptor(
    CoreOperation Operation,
    string RequiredEntryPoint,
    string Description);

/// <summary>
/// Catalog of the core operations the Windows shell depends on.
/// <para>
/// The catalog is the single place that names the C ABI entry point of an operation.
/// The shell uses it to decide whether an operation can be executed against the
/// linked core build instead of pretending that an operation succeeded.
/// </para>
/// </summary>
public static class DocumentCoreCapabilities
{
    private static readonly IReadOnlyDictionary<CoreOperation, CoreOperationDescriptor> Descriptors =
        new Dictionary<CoreOperation, CoreOperationDescriptor>
        {
            [CoreOperation.DocumentOpen] = new(CoreOperation.DocumentOpen, "vocument_c_open_document", "Opens a document through the scoped file broker and returns the session snapshot."),
            [CoreOperation.PageGeometry] = new(CoreOperation.PageGeometry, "vocument_c_get_page_geometry", "Returns the unscaled page geometry in milli-points."),
            [CoreOperation.PageRender] = new(CoreOperation.PageRender, "vocument_c_render_page_tile", "Rasterises a visible viewport tile with the MuPDF engine."),
            [CoreOperation.TextSearch] = new(CoreOperation.TextSearch, "vocument_c_search_exact", "Runs the exact FTS5 search over the document index."),
            [CoreOperation.TextSelection] = new(CoreOperation.TextSelection, "vocument_c_select_page_text", "Extracts the raw and cleaned text of a page."),
            [CoreOperation.HighlightAnnotation] = new(CoreOperation.HighlightAnnotation, "vocument_c_add_highlight", "Adds a highlight annotation through the transaction manager."),
            [CoreOperation.PageRotation] = new(CoreOperation.PageRotation, "vocument_c_rotate_pages", "Rotates pages through the transaction manager."),
            [CoreOperation.TransactionUndo] = new(CoreOperation.TransactionUndo, "vocument_c_undo", "Undoes the last transaction of a session."),
            [CoreOperation.TransactionRedo] = new(CoreOperation.TransactionRedo, "vocument_c_redo", "Redoes the last undone transaction of a session."),
            [CoreOperation.SessionSave] = new(CoreOperation.SessionSave, "vocument_c_save_session", "Saves a session through the MuPDF save pipeline."),
            [CoreOperation.RecoveryCheckpoint] = new(CoreOperation.RecoveryCheckpoint, "vocument_c_check_recovery", "Reports whether the session has a recovery checkpoint."),
            [CoreOperation.PageReorder] = new(CoreOperation.PageReorder, "vocument_c_reorder_pages", "Moves pages inside the page tree (core: session_registry().reorder_pages)."),
            [CoreOperation.PageDelete] = new(CoreOperation.PageDelete, "vocument_c_delete_pages", "Deletes pages (core: session_registry().delete_pages)."),
            [CoreOperation.PageExtract] = new(CoreOperation.PageExtract, "vocument_c_extract_pages", "Extracts pages into a new document (core: session_registry().extract_pages)."),
            [CoreOperation.PageStamp] = new(CoreOperation.PageStamp, "vocument_c_apply_stamp", "Applies a stamp to pages (core: workbench stamps)."),
            [CoreOperation.FlowDocumentOpen] = new(CoreOperation.FlowDocumentOpen, "vocument_c_open_flow_document", "Opens a .pdoc FlowDocument (core: vocument-flow)."),
            [CoreOperation.FlowPdfExport] = new(CoreOperation.FlowPdfExport, "vocument_c_export_flow_pdf", "Exports a FlowDocument as tagged PDF (core: vocument-layout pdf_export)."),
            [CoreOperation.DocumentCompare] = new(CoreOperation.DocumentCompare, "vocument_c_compare_documents", "Compares two sessions (core: session_registry().compare_sessions).")
        };

    /// <summary>All known operations with their C ABI entry point.</summary>
    public static IReadOnlyCollection<CoreOperationDescriptor> All => Descriptors.Values.ToList();

    /// <summary>Returns the descriptor of an operation.</summary>
    public static CoreOperationDescriptor Describe(CoreOperation operation)
    {
        if (!Descriptors.TryGetValue(operation, out var descriptor))
        {
            throw new ArgumentOutOfRangeException(nameof(operation), operation, "Unknown core operation.");
        }

        return descriptor;
    }

    /// <summary>
    /// Formats the reason why an operation cannot be executed against the linked core.
    /// </summary>
    public static string DescribeUnavailable(CoreOperation operation)
    {
        var descriptor = Describe(operation);
        return $"'{descriptor.Description}' requires the core entry point '{descriptor.RequiredEntryPoint}', which the linked core build does not export.";
    }
}

/// <summary>
/// Raised when a core operation is requested that the linked core build does not
/// expose through the C ABI. The shell never emulates such an operation: the user is
/// told which core entry point is missing.
/// </summary>
public sealed class CoreCapabilityUnavailableException : InvalidOperationException
{
    public CoreCapabilityUnavailableException(CoreOperation operation)
        : base(DocumentCoreCapabilities.DescribeUnavailable(operation))
    {
        Operation = operation;
        RequiredEntryPoint = DocumentCoreCapabilities.Describe(operation).RequiredEntryPoint;
    }

    public CoreCapabilityUnavailableException(CoreOperation operation, string message)
        : base(message)
    {
        Operation = operation;
        RequiredEntryPoint = DocumentCoreCapabilities.Describe(operation).RequiredEntryPoint;
    }

    public CoreOperation Operation { get; }

    public string RequiredEntryPoint { get; }
}
