package org.vocument.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.vocument_ffi.FfiHighlightAnnotation
import uniffi.vocument_ffi.FfiPdfRectMilliPt
import uniffi.vocument_ffi.FfiTransactionHistorySnapshot

/**
 * Native Android Jetpack Compose panel for M0.4 First Mutations (`ROADMAP.md` M0.4, `ANNOT-001`, `ROTATE-001`, `UNDO-001`, `UNDO-004`).
 * Executes all mutations exclusively through `DocumentCoreBridge` -> `TransactionManager`.
 */
@Composable
fun MutationToolsPanel(
    sessionId: String,
    currentPageIndex: UInt,
    coreBridge: DocumentCoreBridge,
    onMutationApplied: (FfiTransactionHistorySnapshot) -> Unit,
    onError: (String) -> Unit
) {
    val context = LocalContext.current
    var history by remember(sessionId, currentPageIndex) {
        mutableStateOf(runCatching { coreBridge.transactionHistory(sessionId) }.getOrNull())
    }
    var rotationDegrees by remember(sessionId, currentPageIndex, history) {
        mutableStateOf(runCatching { coreBridge.pageRotationDegrees(sessionId, currentPageIndex) }.getOrDefault(0u))
    }
    var highlights by remember(sessionId, currentPageIndex, history) {
        mutableStateOf(
            runCatching { coreBridge.pageHighlightAnnotations(sessionId, currentPageIndex) }
                .getOrDefault(emptyList())
        )
    }
    var exportedJournalPreview by remember(sessionId) { mutableStateOf<String?>(null) }

    fun applyMutation(block: () -> FfiTransactionHistorySnapshot) {
        runCatching(block)
            .onSuccess { updated ->
                history = updated
                rotationDegrees = runCatching {
                    coreBridge.pageRotationDegrees(sessionId, currentPageIndex)
                }.getOrDefault(0u)
                highlights = runCatching {
                    coreBridge.pageHighlightAnnotations(sessionId, currentPageIndex)
                }.getOrDefault(emptyList())
                onMutationApplied(updated)
            }
            .onFailure { err ->
                onError(err.message ?: "Mutation failed")
            }
    }

    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = "Page #${currentPageIndex + 1u} Mutations & Undo/Redo (Rotation: ${rotationDegrees}°)",
            style = MaterialTheme.typography.titleMedium
        )
        Spacer(modifier = Modifier.height(6.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = {
                applyMutation { coreBridge.rotatePages(sessionId, listOf(currentPageIndex), -90) }
            }) {
                Text("Rotate −90°")
            }
            Button(onClick = {
                applyMutation { coreBridge.rotatePages(sessionId, listOf(currentPageIndex), 90) }
            }) {
                Text("Rotate +90°")
            }
            OutlinedButton(onClick = {
                applyMutation { coreBridge.rotatePages(sessionId, listOf(currentPageIndex), 180) }
            }) {
                Text("180°")
            }
        }

        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = {
                applyMutation {
                    val sel = coreBridge.selectPageText(sessionId, currentPageIndex, null)
                    val rect = sel.highlightRects.firstOrNull() ?: FfiPdfRectMilliPt(
                        x0MilliPt = 72_000,
                        y0MilliPt = 700_000,
                        x1MilliPt = 360_000,
                        y1MilliPt = 724_000
                    )
                    val rev = history?.currentRevision ?: 1uL
                    coreBridge.addHighlightAnnotation(
                        sessionId,
                        FfiHighlightAnnotation(
                            annotationId = "annot_android_${currentPageIndex}_$rev",
                            pageIndex = currentPageIndex,
                            rect = rect,
                            colorR = 255u,
                            colorG = 230u,
                            colorB = 80u,
                            opacityMilli = 450u,
                            author = "Vocument Android",
                            contents = "Highlight on page #${currentPageIndex + 1u}"
                        )
                    )
                }
            }) {
                Text("Highlight First Line")
            }

            OutlinedButton(
                enabled = history?.canUndo == true,
                onClick = { applyMutation { coreBridge.undo(sessionId) } }
            ) {
                Text("Undo (${history?.undoCount ?: 0u})")
            }

            OutlinedButton(
                enabled = history?.canRedo == true,
                onClick = { applyMutation { coreBridge.redo(sessionId) } }
            ) {
                Text("Redo (${history?.redoCount ?: 0u})")
            }
        }

        history?.let { hist ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = "Rev ${hist.currentRevision} (${if (hist.isDirty) "Modified" else "Clean"}) • " +
                    "Undo: ${hist.undoDescription ?: "none"} • Redo: ${hist.redoDescription ?: "none"}",
                style = MaterialTheme.typography.bodySmall
            )
        }

        Spacer(modifier = Modifier.height(8.dp))
        OutlinedButton(onClick = {
            runCatching { coreBridge.exportTransactionJournalJson(sessionId) }
                .onSuccess { json ->
                    exportedJournalPreview = json
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                    clipboard?.setPrimaryClip(ClipData.newPlainText("Vocument Transaction Journal", json))
                }
                .onFailure { err -> onError(err.message ?: "Journal export failed") }
        }) {
            Text("Copy Persistent Transaction Journal JSON")
        }

        exportedJournalPreview?.let { json ->
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = json.lines().take(6).joinToString("\n"),
                style = MaterialTheme.typography.bodySmall
            )
        }

        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Highlights on Page #${currentPageIndex + 1u} (${highlights.size})",
            style = MaterialTheme.typography.labelLarge
        )
        LazyColumn(modifier = Modifier.fillMaxWidth()) {
            items(highlights, key = { it.annotationId }) { annot ->
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 4.dp)
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(8.dp),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(
                            text = annot.contents ?: annot.annotationId,
                            style = MaterialTheme.typography.bodySmall
                        )
                        OutlinedButton(onClick = {
                            applyMutation {
                                coreBridge.removeHighlightAnnotation(
                                    sessionId,
                                    currentPageIndex,
                                    annot.annotationId
                                )
                            }
                        }) {
                            Text("Remove")
                        }
                    }
                }
            }
        }
    }
}
