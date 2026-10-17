package org.vocument.app

fun main() {
    val bridge = DocumentCoreBridge()
    val handshake = bridge.performHandshake()

    check(handshake.baselineVersion == "0.1") {
        "Expected baselineVersion '0.1', got '${handshake.baselineVersion}'"
    }
    check(handshake.milestone == "M0") {
        "Expected milestone 'M0', got '${handshake.milestone}'"
    }
    check(handshake.strictOfflineDefault) {
        "Expected strictOfflineDefault == true (NET-001)"
    }
    check(!handshake.telemetryEnabled) {
        "Expected telemetryEnabled == false (LOG-001)"
    }
    check(handshake.targetFps == 60u) {
        "Expected targetFps == 60 (PERF-001), got ${handshake.targetFps}"
    }
    check(handshake.defaultGermanVariant == "de-AT") {
        "Expected defaultGermanVariant == 'de-AT' (I18N-002), got '${handshake.defaultGermanVariant}'"
    }
    check(handshake.builtInOcrLanguages == listOf("deu", "eng")) {
        "Expected builtInOcrLanguages == ['deu', 'eng'] (OCR-002), got ${handshake.builtInOcrLanguages}"
    }

    println(
        "✓ Kotlin (Android) → UniFFI → Rust DocumentCore handshake verified: " +
            "baseline=${handshake.baselineVersion}, milestone=${handshake.milestone}, " +
            "offline=${handshake.strictOfflineDefault}, telemetry=${handshake.telemetryEnabled}, " +
            "fps=${handshake.targetFps}, locale=${handshake.defaultGermanVariant}, " +
            "ocr=${handshake.builtInOcrLanguages}"
    )
}
