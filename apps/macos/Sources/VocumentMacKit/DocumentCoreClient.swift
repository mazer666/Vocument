import Foundation

/// Thin Swift-side representation of the `DocumentCore` bootstrap state (`ARCHITECTURE.md` §§3–5).
/// Product and document logic resides in the Rust core (`vocument-core`).
public struct DocumentCoreClient: Sendable {
    public let baselineVersion: String
    public let currentMilestone: String
    public let strictOfflineDefault: Bool

    public init(
        baselineVersion: String = "0.1",
        currentMilestone: String = "M0",
        strictOfflineDefault: Bool = true
    ) {
        self.baselineVersion = baselineVersion
        self.currentMilestone = currentMilestone
        self.strictOfflineDefault = strictOfflineDefault
    }
}
