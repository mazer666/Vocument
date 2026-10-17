package org.vocument.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentCoreBridgeTest {
    @Test
    fun verifyAndroidPlatformUniFFIHandshake() {
        val bridge = DocumentCoreBridge()
        val handshake = bridge.performHandshake()

        assertEquals("0.1", handshake.baselineVersion)
        assertEquals("M0", handshake.milestone)
        assertTrue("NET-001 strict offline default must be true", handshake.strictOfflineDefault)
        assertFalse("LOG-001 telemetry must be false", handshake.telemetryEnabled)
        assertEquals(60u, handshake.targetFps)
        assertEquals("de-AT", handshake.defaultGermanVariant)
        assertEquals(listOf("deu", "eng"), handshake.builtInOcrLanguages)
    }
}
