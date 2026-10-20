package org.vocument.app

import android.content.Context
import android.util.Log
import android.view.ViewGroup
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import uniffi.vocument_ffi.FfiScanFrame
import uniffi.vocument_ffi.FfiScanPixelFormat
import androidx.compose.runtime.mutableStateOf
import java.util.concurrent.Executors

private const val TAG = "CameraPreviewView"

/**
 * CameraX Live Preview & Frame Ingestion View (`SCAN-001`, `SCAN-004`).
 * Feeds live camera frames directly into the shared Rust `ImagingEngine` for real-time edge detection.
 *
 * The callback runs on the **camera thread**, so that the analysis of the
 * frame does not block the interface. The caller publishes its result on the
 * main thread, because the state of the interface may only be changed there.
 *
 * The camera is unbound when the view leaves: a camera that stays bound keeps
 * the privacy indicator of the system on and drains the battery.
 */
@Composable
fun CameraPreviewView(
    modifier: Modifier = Modifier,
    onFrameAnalyzed: (FfiScanFrame) -> Unit
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val cameraExecutor = remember { Executors.newSingleThreadExecutor() }
    // The camera provider is needed to unbind the camera again. A camera that
    // stays bound keeps the privacy indicator of the system on and drains the
    // battery, even though the view is gone.
    val cameraProviderRef = remember { mutableStateOf<ProcessCameraProvider?>(null) }

    DisposableEffect(Unit) {
        onDispose {
            cameraExecutor.shutdown()
            // The camera is released when the view leaves. The privacy
            // indicator of the system goes out with it.
            runCatching { cameraProviderRef.value?.unbindAll() }
        }
    }

    AndroidView(
        modifier = modifier.fillMaxSize(),
        factory = { ctx ->
            val previewView = PreviewView(ctx).apply {
                layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                scaleType = PreviewView.ScaleType.FILL_CENTER
            }

            val cameraProviderFuture = ProcessCameraProvider.getInstance(ctx)
            cameraProviderFuture.addListener({
                runCatching {
                    val cameraProvider = cameraProviderFuture.get()
                    cameraProviderRef.value = cameraProvider

                    val preview = Preview.Builder().build().also {
                        it.surfaceProvider = previewView.surfaceProvider
                    }

                    var lastAnalysisTimestamp = 0L
                    val imageAnalysis = ImageAnalysis.Builder()
                        .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_RGBA_8888)
                        .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                        .build()
                        .also { analysis ->
                            analysis.setAnalyzer(cameraExecutor) { imageProxy ->
                                try {
                                    val now = System.currentTimeMillis()
                                    // Throttle edge detection to ~8 fps (125 ms interval) to respect battery and thermal budgets (BIGMOBILE-001)
                                    if (now - lastAnalysisTimestamp >= 125) {
                                        lastAnalysisTimestamp = now
                                        val frame = extractFrameFromImageProxy(imageProxy)
                                        if (frame != null) {
                                            // The callback runs on the camera
                                            // thread, so that the analysis of
                                            // the frame does not block the
                                            // interface. The caller publishes
                                            // its result on the main thread.
                                            onFrameAnalyzed(frame)
                                        }
                                    }
                                } catch (e: Exception) {
                                    Log.e(TAG, "Frame analysis error", e)
                                } finally {
                                    imageProxy.close()
                                }
                            }
                        }

                    val cameraSelector = CameraSelector.DEFAULT_BACK_CAMERA

                    cameraProvider.unbindAll()
                    cameraProvider.bindToLifecycle(
                        lifecycleOwner,
                        cameraSelector,
                        preview,
                        imageAnalysis
                    )
                }.onFailure { err ->
                    Log.e(TAG, "Failed to bind CameraX lifecycle", err)
                }
            }, ContextCompat.getMainExecutor(ctx))

            previewView
        }
    )
}

/**
 * Converts a CameraX RGBA_8888 `ImageProxy` into an immutable `FfiScanFrame` for Rust ingestion.
 */
fun extractFrameFromImageProxy(imageProxy: ImageProxy): FfiScanFrame? {
    val planes = imageProxy.planes
    if (planes.isEmpty()) return null

    val buffer = planes[0].buffer
    val pixelStride = planes[0].pixelStride
    val rowStride = planes[0].rowStride
    val width = imageProxy.width
    val height = imageProxy.height

    val bytes = if (pixelStride == 4 && rowStride == width * 4) {
        val data = ByteArray(buffer.remaining())
        buffer.get(data)
        data
    } else {
        // Handle row stride padding if rowStride > width * pixelStride
        val data = ByteArray(width * height * 4)
        var rowOffset = 0
        val rowBuffer = ByteArray(rowStride)
        for (row in 0 until height) {
            buffer.position(row * rowStride)
            val bytesToRead = minOf(buffer.remaining(), rowStride)
            buffer.get(rowBuffer, 0, bytesToRead)
            System.arraycopy(rowBuffer, 0, data, rowOffset, width * 4)
            rowOffset += width * 4
        }
        data
    }

    return FfiScanFrame(
        width = width.toUInt(),
        height = height.toUInt(),
        stride = width.toUInt(),
        format = FfiScanPixelFormat.RGBA8,
        orientationDegrees = imageProxy.imageInfo.rotationDegrees.toUInt(),
        pixels = bytes,
        timestampMs = imageProxy.imageInfo.timestamp.toULong() / 1_000_000u,
        deviceAngleDegrees = 0.0f,
        brightnessHint = 1.0f,
        isSteady = true
    )
}
