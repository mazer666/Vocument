package org.vocument.app

import android.content.Context
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import uniffi.vocument_ffi.documentCoreConfigureTessdataPath

/**
 * Deterministic provisioner for offline German and English Tesseract traineddata models
 * in the Android filesystem (`context.filesDir/tessdata`).
 */
object TessdataProvisioner {
    private const val TAG = "TessdataProvisioner"
    val REQUIRED_MODELS = listOf("deu.traineddata", "eng.traineddata")

    fun ensureTessdata(context: Context): File {
        val tessdataDir = File(context.filesDir, "tessdata")
        if (!tessdataDir.exists()) {
            tessdataDir.mkdirs()
        }

        for (model in REQUIRED_MODELS) {
            val destFile = File(tessdataDir, model)
            val assetPath = "tessdata/$model"

            val assetSize = runCatching {
                context.assets.open(assetPath).use { it.available().toLong() }
            }.getOrDefault(-1L)

            if (!destFile.exists() || (assetSize > 0 && destFile.length() != assetSize)) {
                Log.i(TAG, "Extracting $assetPath to ${destFile.absolutePath}...")
                context.assets.open(assetPath).use { input ->
                    FileOutputStream(destFile).use { output ->
                        input.copyTo(output)
                    }
                }
                Log.i(TAG, "Extracted $model (${destFile.length()} bytes)")
            }
        }

        for (model in REQUIRED_MODELS) {
            val f = File(tessdataDir, model)
            if (!f.exists() || f.length() == 0L) {
                error("Failed to provision required tessdata model: ${f.absolutePath}")
            }
        }

        documentCoreConfigureTessdataPath(tessdataDir.absolutePath)
        Log.i(TAG, "Deterministic tessdata provisioned at: ${tessdataDir.absolutePath}")
        return tessdataDir
    }
}
