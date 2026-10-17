import Testing
@testable import VocumentMacKit

@Test
func verifyMacPlatformBaseline() {
    let client = DocumentCoreClient()
    #expect(client.baselineVersion == "0.1")
    #expect(client.currentMilestone == "M0")
    #expect(client.strictOfflineDefault == true)
}
