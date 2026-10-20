package org.vocument.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import uniffi.vocument_ffi.FfiPoint2D
import uniffi.vocument_ffi.FfiQuadCorners

/**
 * Camera viewfinder overlay with draggable document corners (`SCAN-002`).
 *
 * The overlay draws the detected document quad and lets the user correct the
 * corners by dragging the handles. The coordinates are normalized to the
 * canvas size, so the overlay is independent of the device resolution.
 */

@Composable
fun ViewfinderOverlay(
    corners: FfiQuadCorners?,
    isManualAdjustment: Boolean,
    onCornerDragged: (cornerIndex: Int, newX: Float, newY: Float) -> Unit,
    modifier: Modifier = Modifier
) {
    var activeDraggingCorner by remember { mutableStateOf<Int?>(null) }
    var canvasSize by remember { mutableStateOf(IntSize.Zero) }

    Canvas(
        modifier = modifier
            .onSizeChanged { canvasSize = it }
            .pointerInput(corners) {
                detectDragGestures(
                    onDragStart = { offset ->
                        if (canvasSize.width > 0 && canvasSize.height > 0) {
                            val activeCorners = corners ?: FfiQuadCorners(
                                topLeft = FfiPoint2D(0.15f, 0.15f),
                                topRight = FfiPoint2D(0.85f, 0.15f),
                                bottomRight = FfiPoint2D(0.85f, 0.85f),
                                bottomLeft = FfiPoint2D(0.15f, 0.85f),
                                confidenceMilli = 999u
                            )
                            val p0 = Offset(activeCorners.topLeft.x * canvasSize.width, activeCorners.topLeft.y * canvasSize.height)
                            val p1 = Offset(activeCorners.topRight.x * canvasSize.width, activeCorners.topRight.y * canvasSize.height)
                            val p2 = Offset(activeCorners.bottomRight.x * canvasSize.width, activeCorners.bottomRight.y * canvasSize.height)
                            val p3 = Offset(activeCorners.bottomLeft.x * canvasSize.width, activeCorners.bottomLeft.y * canvasSize.height)
                            val points = listOf(p0, p1, p2, p3)
                            val hitRadius = 48.dp.toPx()
                            val hitIndex = points.indexOfFirst { (it - offset).getDistance() <= hitRadius }
                            if (hitIndex != -1) {
                                activeDraggingCorner = hitIndex
                            }
                        }
                    },
                    onDragEnd = {
                        activeDraggingCorner = null
                    },
                    onDragCancel = {
                        activeDraggingCorner = null
                    },
                    onDrag = { change, dragAmount ->
                        val cornerIdx = activeDraggingCorner
                        if (cornerIdx != null && canvasSize.width > 0 && canvasSize.height > 0) {
                            change.consume()
                            val activeCorners = corners ?: FfiQuadCorners(
                                topLeft = FfiPoint2D(0.15f, 0.15f),
                                topRight = FfiPoint2D(0.85f, 0.15f),
                                bottomRight = FfiPoint2D(0.85f, 0.85f),
                                bottomLeft = FfiPoint2D(0.15f, 0.85f),
                                confidenceMilli = 999u
                            )
                            val currentNorm = when (cornerIdx) {
                                0 -> activeCorners.topLeft
                                1 -> activeCorners.topRight
                                2 -> activeCorners.bottomRight
                                else -> activeCorners.bottomLeft
                            }
                            val newX = (currentNorm.x + dragAmount.x / canvasSize.width).coerceIn(0.01f, 0.99f)
                            val newY = (currentNorm.y + dragAmount.y / canvasSize.height).coerceIn(0.01f, 0.99f)
                            onCornerDragged(cornerIdx, newX, newY)
                        }
                    }
                )
            }
    ) {
        val canvasWidth = size.width
        val canvasHeight = size.height

        val guideColor = Color.White.copy(alpha = 0.3f)
        drawLine(
            color = guideColor,
            start = Offset(canvasWidth * 0.15f, canvasHeight * 0.5f),
            end = Offset(canvasWidth * 0.85f, canvasHeight * 0.5f),
            strokeWidth = 1f
        )
        drawLine(
            color = guideColor,
            start = Offset(canvasWidth * 0.5f, canvasHeight * 0.15f),
            end = Offset(canvasWidth * 0.5f, canvasHeight * 0.85f),
            strokeWidth = 1f
        )

        if (corners != null) {
            val p1 = Offset(corners.topLeft.x * canvasWidth, corners.topLeft.y * canvasHeight)
            val p2 = Offset(corners.topRight.x * canvasWidth, corners.topRight.y * canvasHeight)
            val p3 = Offset(corners.bottomRight.x * canvasWidth, corners.bottomRight.y * canvasHeight)
            val p4 = Offset(corners.bottomLeft.x * canvasWidth, corners.bottomLeft.y * canvasHeight)

            val polyPath = Path().apply {
                moveTo(p1.x, p1.y)
                lineTo(p2.x, p2.y)
                lineTo(p3.x, p3.y)
                lineTo(p4.x, p4.y)
                close()
            }

            val quadColor = if (isManualAdjustment) {
                Color(0xFF29B6F6) // Cyan/Blue in manual adjustment mode
            } else if (corners.confidenceMilli >= 650u) {
                Color(0xFF00E676) // High confidence: Vibrant Green
            } else {
                Color(0xFFFFD600) // Moderate confidence: Amber
            }

            drawPath(
                path = polyPath,
                color = quadColor.copy(alpha = 0.2f)
            )
            drawPath(
                path = polyPath,
                color = quadColor,
                style = Stroke(width = 3.dp.toPx())
            )

            val points = listOf(p1, p2, p3, p4)
            for ((idx, cp) in points.withIndex()) {
                val isDragging = activeDraggingCorner == idx
                if (isDragging) {
                    drawCircle(color = quadColor.copy(alpha = 0.35f), radius = 24.dp.toPx(), center = cp)
                    drawCircle(color = Color.White, radius = 12.dp.toPx(), center = cp)
                    drawCircle(color = quadColor, radius = 8.dp.toPx(), center = cp)
                } else {
                    drawCircle(color = Color.Black.copy(alpha = 0.4f), radius = 16.dp.toPx(), center = cp)
                    drawCircle(color = Color.White, radius = 8.dp.toPx(), center = cp)
                    drawCircle(color = quadColor, radius = 5.dp.toPx(), center = cp)
                }
            }
        }
    }
