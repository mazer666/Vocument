package org.vocument.app

import android.content.Context
import android.graphics.BitmapFactory
import android.util.Log
import java.io.File
import kotlin.math.max
import kotlin.math.min
import uniffi.vocument_ffi.*

data class AndroidOcrVerificationResult(
    val passed: Boolean,
    val cer: Double,
    val characterAccuracy: Double,
    val keywordHitRate: Double,
    val meanConfidence: Double,
    val totalWords: Int,
    val totalCharacters: Int,
    val boundingBoxesValid: Boolean,
    val characterBoundingBoxesValid: Boolean,
    val detectedLanguage: String,
    val fullText: String,
    val errorMessage: String? = null
)

object AndroidOcrRuntimeGate {
    private const val TAG = "VocumentOcrGate"

    fun runVerification(context: Context, coreBridge: DocumentCoreBridge): AndroidOcrVerificationResult {
        Log.i(TAG, "Starting authentic on-device OCR verification on Android...")
        try {
            // 1. Ensure tessdata is provisioned in the Android filesystem
            val tessdataDir = TessdataProvisioner.ensureTessdata(context)
            coreBridge.configureTessdataPath(tessdataDir.absolutePath)

            // 2. Load the camera-distorted Austrian legal contract fixture from assets
            val bitmap = context.assets.open("corpus/deu_vertrag_vertraulichkeit_camera_distorted.png").use { stream ->
                BitmapFactory.decodeStream(stream)
            } ?: error("Failed to decode camera distorted test image from assets")

            val expectedText = context.assets.open("corpus/deu_vertrag_vertraulichkeit_camera_distorted_expected.txt").use { stream ->
                stream.bufferedReader().readText()
            }

            val width = bitmap.width.toUInt()
            val height = bitmap.height.toUInt()
            val argbPixels = IntArray(bitmap.width * bitmap.height)
            bitmap.getPixels(argbPixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)

            val rgbaBytes = ByteArray(bitmap.width * bitmap.height * 4)
            var byteIdx = 0
            for (pixel in argbPixels) {
                val a = (pixel shr 24 and 0xFF).toByte()
                val r = (pixel shr 16 and 0xFF).toByte()
                val g = (pixel shr 8 and 0xFF).toByte()
                val b = (pixel and 0xFF).toByte()
                rgbaBytes[byteIdx] = r
                rgbaBytes[byteIdx + 1] = g
                rgbaBytes[byteIdx + 2] = b
                rgbaBytes[byteIdx + 3] = a
                byteIdx += 4
            }

            val rawFrame = FfiScanFrame(
                width = width,
                height = height,
                stride = width,
                format = FfiScanPixelFormat.RGBA8,
                orientationDegrees = 0u,
                pixels = rgbaBytes,
                timestampMs = System.currentTimeMillis().toULong(),
                deviceAngleDegrees = 0f,
                brightnessHint = 1.0f,
                isSteady = true
            )

            // 3. Apply M1.1 Perspective Rectification
            val corners = FfiQuadCorners(
                topLeft = FfiPoint2D(233.32f / 1500.0f, 82.95f / 2050.0f),
                topRight = FfiPoint2D(1465.35f / 1500.0f, 223.33f / 2050.0f),
                bottomRight = FfiPoint2D(1266.68f / 1500.0f, 1967.05f / 2050.0f),
                bottomLeft = FfiPoint2D(34.65f / 1500.0f, 1826.67f / 2050.0f),
                confidenceMilli = 900u
            )
            val rectified = coreBridge.warpPerspective(rawFrame, corners, 1240.0f / 1755.0f)

            // 4. Apply M1.2 Intelligent Scan Enhancement (Illumination Whitening & Shadow Removal)
            val enhanced = coreBridge.enhanceScanFrame(rectified, FfiScanCleanupProfile.DOCUMENT_AUTO)

            // 5. Execute Authentic Native Tesseract 5 OCR on Android
            val ocrRes = coreBridge.ocrFrame(enhanced, listOf("deu"), detectLanguage = true)

            // 6. Assert non-empty text and non-empty words
            if (ocrRes.fullText.isBlank()) {
                throw AssertionError("Android OCR produced empty text output!")
            }
            if (ocrRes.words.isEmpty()) {
                throw AssertionError("Android OCR produced 0 recognized words!")
            }

            // 7. Verify Word and Character Bounding Boxes
            var bboxesValid = true
            var charBboxesValid = true
            var totalCharacters = 0

            for (word in ocrRes.words) {
                val box = word.bbox
                if (box.x1 <= box.x0 || box.y1 <= box.y0) {
                    bboxesValid = false
                    Log.e(TAG, "Invalid bounding box for word '${word.text}': x0=${box.x0}, y0=${box.y0}, x1=${box.x1}, y1=${box.y1}")
                    break
                }
                for (ch in word.characters) {
                    totalCharacters++
                    val cbox = ch.bbox
                    if (cbox.x1 <= cbox.x0 || cbox.y1 <= cbox.y0) {
                        charBboxesValid = false
                        Log.e(TAG, "Invalid bounding box for character '${ch.text}': x0=${cbox.x0}, y0=${cbox.y0}, x1=${cbox.x1}, y1=${cbox.y1}")
                        break
                    }
                }
                if (!charBboxesValid) break
            }
            if (!bboxesValid) {
                throw AssertionError("Android OCR word bounding boxes were invalid or empty!")
            }
            if (!charBboxesValid || totalCharacters == 0) {
                throw AssertionError("Android OCR character glyph bounding boxes were invalid or empty (total=$totalCharacters)!")
            }

            // 8. Verify Detected Language (must be German/Austrian legal variant)
            val langValid = ocrRes.detectedLanguage == "deu" || ocrRes.detectedLanguage.startsWith("de")
            if (!langValid) {
                throw AssertionError("Android OCR detected language mismatch: expected 'deu' or 'de-AT', got '${ocrRes.detectedLanguage}'")
            }

            // 9. Compute Levenshtein Character Error Rate (CER) and Accuracy identically to Rust corpus.rs
            val (cer, charAccuracy) = computeCharacterErrorRate(expectedText, ocrRes.fullText)
            val keywords = listOf(
                "VEREINBARUNG", "VERTRAULICHKEIT", "GEHEIMHALTUNG",
                "Vertrags-ID", "Österreichisches", "Geltungsbereich",
                "Informationen", "Vertragsparteien", "Geheimhaltung",
                "Geschäfts-", "Betriebsgeheimnisse", "ABGB", "UWG",
                "Pflichten", "Informationsempfängers", "Empfänger",
                "Vertragszweck", "Erfüllungsort", "Gerichtsstand",
                "Schriftform", "Wien", "Österreich"
            )
            var matchedKeywords = 0
            val recognizedTextNormalized = ocrRes.fullText.lowercase()
            for (kw in keywords) {
                if (recognizedTextNormalized.contains(kw.lowercase())) {
                    matchedKeywords++
                }
            }
            val keywordHitRate = matchedKeywords.toDouble() / keywords.size.toDouble()

            // 10. Evaluate Hardened Quality Gate (CER <= 5.0%, Char Acc >= 95.0%, Keywords >= 95.0%)
            val passed = cer <= 0.05 && charAccuracy >= 0.95 && keywordHitRate >= 0.95 && bboxesValid && charBboxesValid && langValid

            val result = AndroidOcrVerificationResult(
                passed = passed,
                cer = cer,
                characterAccuracy = charAccuracy,
                keywordHitRate = keywordHitRate,
                meanConfidence = ocrRes.meanConfidence.toDouble(),
                totalWords = ocrRes.words.size,
                totalCharacters = totalCharacters,
                boundingBoxesValid = bboxesValid,
                characterBoundingBoxesValid = charBboxesValid,
                detectedLanguage = ocrRes.detectedLanguage,
                fullText = ocrRes.fullText
            )

            val logMsg = "Result: passed=$passed, cer=${String.format("%.2f", cer * 100)}%, charAcc=${String.format("%.2f", charAccuracy * 100)}%, kwHit=${String.format("%.1f", keywordHitRate * 100)}%, words=${ocrRes.words.size}, chars=$totalCharacters, bboxes=$bboxesValid, charBboxes=$charBboxesValid, lang=${ocrRes.detectedLanguage}"
            if (passed) {
                Log.i(TAG, "SUCCESS: $logMsg")
            } else {
                Log.e(TAG, "FAILURE: $logMsg")
            }

            // Write verification result to file in app files directory for adb retrieval
            runCatching {
                val outJson = """{"passed": $passed, "cer": $cer, "character_accuracy": $charAccuracy, "keyword_hit_rate": $keywordHitRate, "total_words": ${ocrRes.words.size}, "total_characters": $totalCharacters, "bounding_boxes_valid": $bboxesValid, "character_bounding_boxes_valid": $charBboxesValid, "detected_language": "${ocrRes.detectedLanguage}"}"""
                File(context.filesDir, "ocr_verification_result.json").writeText(outJson)
            }

            return result
        } catch (e: Throwable) {
            Log.e(TAG, "FATAL: Android OCR verification failed: ${e.message}", e)
            runCatching {
                File(context.filesDir, "ocr_verification_result.json").writeText("""{"passed": false, "error": "${e.message?.replace("\"", "\\\"")}"}""")
            }
            return AndroidOcrVerificationResult(
                passed = false,
                cer = 1.0,
                characterAccuracy = 0.0,
                keywordHitRate = 0.0,
                meanConfidence = 0.0,
                totalWords = 0,
                totalCharacters = 0,
                boundingBoxesValid = false,
                characterBoundingBoxesValid = false,
                detectedLanguage = "unknown",
                fullText = "",
                errorMessage = e.message
            )
        }
    }

    private fun computeCharacterErrorRate(expected: String, recognized: String): Pair<Double, Double> {
        val normExp = expected.replace("\\s+".toRegex(), " ").trim()
        val normRec = recognized.replace("\\s+".toRegex(), " ").trim()
        if (normExp.isEmpty()) return Pair(0.0, 1.0)
        val dist = levenshtein(normExp, normRec)
        val cer = dist.toDouble() / normExp.length.toDouble()
        val accuracy = max(0.0, 1.0 - min(1.0, cer))
        return Pair(cer, accuracy)
    }

    private fun levenshtein(s1: String, s2: String): Int {
        val dp = Array(s1.length + 1) { IntArray(s2.length + 1) }
        for (i in 0..s1.length) dp[i][0] = i
        for (j in 0..s2.length) dp[0][j] = j
        for (i in 1..s1.length) {
            for (j in 1..s2.length) {
                val cost = if (s1[i - 1] == s2[j - 1]) 0 else 1
                dp[i][j] = min(
                    min(dp[i - 1][j] + 1, dp[i][j - 1] + 1),
                    dp[i - 1][j - 1] + cost
                )
            }
        }
        return dp[s1.length][s2.length]
    }
}
