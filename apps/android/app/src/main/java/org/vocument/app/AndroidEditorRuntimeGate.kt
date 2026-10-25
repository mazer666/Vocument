package org.vocument.app

import android.content.Context
import android.util.Log
import java.io.File
import uniffi.vocument_ffi.*

data class AndroidEditorVerificationResult(
    val passed: Boolean,
    val editorId: String,
    val totalPages: UInt,
    val latencyMicros: ULong,
    val savedPdocPath: String,
    val errorMessage: String? = null
)

object AndroidEditorRuntimeGate {
    private const val TAG = "VocumentEditorGate"

    fun runVerification(context: Context, coreBridge: DocumentCoreBridge): AndroidEditorVerificationResult {
        Log.i(TAG, "Starting authentic native editor verification on Android (M4.3, ADR-0020)...")
        try {
            // 1. Create editor session
            val session = coreBridge.createEditorSession("Android Schriftsatz")
            val id = session.editorId
            Log.i(TAG, "Editor session created: $id, pages: ${session.totalPages}")

            // 2. Insert legal text with § symbol
            val s1 = coreBridge.editorInsertText(
                id,
                "In Übereinstimmung mit § 242 BGB sind Leistungspflichten nach Treu und Glauben zu erfüllen."
            )
            require(s1.plainText.contains("§ 242 BGB")) { "Plain text missing § 242 BGB" }
            require(s1.canUndo) { "Editor canUndo must be true" }

            // 3. Formatting: Bold
            coreBridge.editorToggleBold(id)
            val s2 = coreBridge.editorInsertText(id, " Dies ist bindendes Recht.")
            require(s2.plainText.contains("Dies ist bindendes Recht")) { "Bold insertion missing text" }

            // 4. Undo and Redo
            val sUndo = coreBridge.editorUndo(id)
            require(!sUndo.plainText.contains("Dies ist bindendes Recht")) { "Undo did not revert bold insertion" }
            require(sUndo.canRedo) { "canRedo must be true after undo" }

            val sRedo = coreBridge.editorRedo(id)
            require(sRedo.plainText.contains("Dies ist bindendes Recht")) { "Redo did not restore bold insertion" }

            // 5. Paragraph Break & Heading
            coreBridge.editorInsertParagraphBreak(id)
            coreBridge.editorInsertText(id, "I. Klagebegründung")
            val sH1 = coreBridge.editorSetHeadingLevel(id, 1u)
            require(sH1.plainText.contains("I. Klagebegründung")) { "H1 heading missing" }

            // 6. Lists, Tables, and Footnotes
            coreBridge.editorInsertBulletList(id)
            coreBridge.editorInsertTable(id, 3u, 2u)
            coreBridge.editorInsertFootnote(id, "BGHZ 120, 345 - Leitsatz.")

            // 7. Hit-testing
            val sHit = coreBridge.editorHitTest(id, 0u, 100f, 100f)
            require(sHit.cursorRect != null) { "Cursor rect must be set after hit test" }

            // 8. Latency budget (< 16,000 µs = 16 ms)
            val latency = coreBridge.editorMeasureTypingLatencyMicros(
                id,
                "Echtzeit-Textverarbeitung und inkrementelles Layout auf Android."
            )
            Log.i(TAG, "Measured typing latency: $latency µs (budget < 16000 µs)")
            require(latency < 16_000uL) { "Typing latency $latency exceeds 16ms budget" }

            // 9. Atomic save to .pdoc in app cache dir
            val targetFile = File(context.cacheDir, "android_test_${System.currentTimeMillis()}.pdoc")
            val savedPath = coreBridge.editorSavePdoc(id, targetFile.absolutePath)
            require(targetFile.exists() && targetFile.length() > 0) { "Saved .pdoc file does not exist or is empty" }
            Log.i(TAG, "Saved hardened .pdoc package to $savedPath")

            coreBridge.closeEditorSession(id)

            // 10. Reopen .pdoc and verify roundtrip integrity
            val reopened = coreBridge.openPdocEditorSession(targetFile.absolutePath)
            require(reopened.plainText.contains("§ 242 BGB")) { "Reopened document missing § 242 BGB" }
            require(reopened.plainText.contains("I. Klagebegründung")) { "Reopened document missing H1" }

            coreBridge.closeEditorSession(reopened.editorId)
            targetFile.delete()

            Log.i(TAG, "SUCCESS: Result: passed=true, latencyMicros=$latency")
            return AndroidEditorVerificationResult(
                passed = true,
                editorId = id,
                totalPages = reopened.totalPages,
                latencyMicros = latency,
                savedPdocPath = savedPath
            )
        } catch (e: Exception) {
            Log.e(TAG, "FAILED: ${e.message}", e)
            return AndroidEditorVerificationResult(
                passed = false,
                editorId = "",
                totalPages = 0u,
                latencyMicros = 0uL,
                savedPdocPath = "",
                errorMessage = e.message
            )
        }
    }
}
