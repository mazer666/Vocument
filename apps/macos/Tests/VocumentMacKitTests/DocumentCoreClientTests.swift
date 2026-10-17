import Testing
import VocumentFFI
@testable import VocumentMacKit

@Test
func verifyMacPlatformUniFFIHandshake() throws {
    let client = DocumentCoreClient()
    let handshake = try client.performHandshake()
    #expect(handshake.baselineVersion == "0.1")
    #expect(handshake.milestone == "M0")
    #expect(handshake.strictOfflineDefault == true)
    #expect(handshake.telemetryEnabled == false)
    #expect(handshake.targetFps == 60)
    #expect(handshake.defaultGermanVariant == "de-AT")
    #expect(handshake.builtInOcrLanguages == ["deu", "eng"])
}
