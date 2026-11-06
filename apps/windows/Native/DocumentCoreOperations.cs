using System;
using System.Collections.Generic;

namespace Vocument.Windows.Native;

/// <summary>
/// Capability handling of the native bridge.
/// <para>
/// The Windows client talks to the shared Rust core through the C ABI of
/// <c>platform/vocument-ffi</c>. That ABI currently exposes the document session
/// operations of the PDF workbench. Operations that exist in the core but are not
/// exported by the C ABI are never emulated in the shell: the client reports the
/// missing entry point instead of returning a fabricated result (QA-003).
/// </para>
/// </summary>
public sealed partial class DocumentCoreClient
{
    private static readonly HashSet<CoreOperation> ImplementedOperations = new()
    {
        CoreOperation.DocumentOpen,
        CoreOperation.PageGeometry,
        CoreOperation.PageRender,
        CoreOperation.TextSearch,
        CoreOperation.TextSelection,
        CoreOperation.HighlightAnnotation,
        CoreOperation.PageRotation,
        CoreOperation.TransactionUndo,
        CoreOperation.TransactionRedo,
        CoreOperation.SessionSave,
        CoreOperation.RecoveryCheckpoint
    };

    /// <summary>
    /// All operations the Windows client can execute against the linked core build.
    /// </summary>
    public static IReadOnlyCollection<CoreOperation> ImplementedCoreOperations => ImplementedOperations;

    /// <summary>
    /// Reports whether the client implements the given core operation.
    /// </summary>
    public bool SupportsOperation(CoreOperation operation) => ImplementedOperations.Contains(operation);

    /// <summary>
    /// Reports whether the linked core build exports the C ABI entry point of the
    /// given operation. This distinguishes a missing core entry point from a missing
    /// client side ABI contract.
    /// </summary>
    public bool IsCoreEntryPointExported(CoreOperation operation)
    {
        var descriptor = DocumentCoreCapabilities.Describe(operation);
        return NativeMethods.TryGetExport(descriptor.RequiredEntryPoint);
    }

    /// <summary>
    /// Ensures that the given operation can be executed. The method throws
    /// <see cref="CoreCapabilityUnavailableException"/> when the operation is not
    /// available, naming the core entry point that is missing.
    /// </summary>
    public void EnsureOperationAvailable(CoreOperation operation)
    {
        if (SupportsOperation(operation))
        {
            return;
        }

        var descriptor = DocumentCoreCapabilities.Describe(operation);
        var message = IsCoreEntryPointExported(operation)
            ? $"'{descriptor.Description}' requires the core entry point '{descriptor.RequiredEntryPoint}'. The linked core build exports it, but the Windows client does not implement that ABI contract yet."
            : DocumentCoreCapabilities.DescribeUnavailable(operation);

        throw new CoreCapabilityUnavailableException(operation, message);
    }
}
