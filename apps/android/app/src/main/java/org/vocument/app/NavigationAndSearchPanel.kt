package org.vocument.app

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import uniffi.vocument_ffi.FfiPageDescriptor
import uniffi.vocument_ffi.FfiSearchIndexProgress
import uniffi.vocument_ffi.FfiSearchResponse

/**
 * Native Jetpack Compose panel for M0.3 Navigation & Classic Search (`PAGEVIEW-001`, `PAGELABEL-001..004`,
 * `OCRCOPY-001..002`, `SEARCH-001`, `SEARCH-002`, `SEARCH-009`, `MOBILE-002`).
 */
@Composable
fun NavigationAndSearchPanel(
    sessionId: String,
    currentPageIndex: UInt,
    descriptors: List<FfiPageDescriptor>,
    indexProgress: FfiSearchIndexProgress?,
    coreBridge: DocumentCoreBridge,
    onSelectPage: (UInt) -> Unit,
    onIndexNextBatch: () -> Unit,
    onError: (String) -> Unit
) {
    val clipboard = LocalClipboardManager.current
    var jumpInput by remember { mutableStateOf("") }
    var searchQuery by remember { mutableStateOf("") }
    var searchResponse by remember { mutableStateOf<FfiSearchResponse?>(null) }
    var copyFeedback by remember { mutableStateOf<String?>(null) }

    Column(modifier = Modifier.fillMaxWidth()) {
        // Logical Page Label + Physical Page Jump (PAGELABEL-002, PAGELABEL-003)
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedTextField(
                value = jumpInput,
                onValueChange = { jumpInput = it },
                label = { Text("Go to label or #page (e.g. ii, 1, #3)") },
                singleLine = true,
                modifier = Modifier.weight(1f)
            )
            Button(
                onClick = {
                    runCatching {
                        coreBridge.resolvePageNavigation(sessionId, jumpInput)
                    }.onSuccess { target ->
                        onSelectPage(target.pageIndex)
                    }.onFailure { err ->
                        onError(err.message ?: "Unable to resolve page navigation")
                    }
                }
            ) {
                Text("Go")
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        // Cleaned Plain-Text Copy vs Raw Copy (OCRCOPY-001, OCRCOPY-002)
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Button(
                onClick = {
                    runCatching {
                        coreBridge.selectPageText(sessionId, currentPageIndex, null)
                    }.onSuccess { selection ->
                        clipboard.setText(AnnotatedString(selection.cleanedText))
                        copyFeedback = "Copied cleaned text (${selection.cleanedText.length} chars)"
                    }.onFailure { err ->
                        onError(err.message ?: "Text selection failed")
                    }
                }
            ) {
                Text("Copy Cleaned Text (OCRCOPY-002)")
            }
            OutlinedButton(
                onClick = {
                    runCatching {
                        coreBridge.selectPageText(sessionId, currentPageIndex, null)
                    }.onSuccess { selection ->
                        clipboard.setText(AnnotatedString(selection.rawText))
                        copyFeedback = "Copied raw text (${selection.rawText.length} chars)"
                    }.onFailure { err ->
                        onError(err.message ?: "Text selection failed")
                    }
                }
            ) {
                Text("Copy Raw")
            }
        }
        copyFeedback?.let { msg ->
            Text(text = msg, style = MaterialTheme.typography.bodySmall)
        }

        Spacer(modifier = Modifier.height(8.dp))

        // Progressive SQLite FTS5 Classic Search (SEARCH-001, SEARCH-002, SEARCH-009)
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                label = { Text("Search PDF (SQLite FTS5)…") },
                singleLine = true,
                modifier = Modifier.weight(1f)
            )
            Button(
                onClick = {
                    runCatching {
                        coreBridge.searchExact(sessionId, searchQuery)
                    }.onSuccess { res ->
                        searchResponse = res
                    }.onFailure { err ->
                        onError(err.message ?: "Search failed")
                    }
                }
            ) {
                Text("Search")
            }
            if (indexProgress != null && !indexProgress.isComplete) {
                OutlinedButton(onClick = onIndexNextBatch) {
                    Text("Index +")
                }
            }
        }

        indexProgress?.let { progress ->
            Text(
                text = "FTS5 Index: ${progress.indexedPages}/${progress.totalPages} pages " +
                    if (progress.isComplete) "(Complete)" else "(Progressive — searchable now)",
                style = MaterialTheme.typography.bodySmall
            )
        }

        searchResponse?.let { response ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = "Results (${response.hits.size}) — indexed ${response.progress.indexedPages}/${response.progress.totalPages}:",
                style = MaterialTheme.typography.labelMedium
            )
            response.hits.forEach { hit ->
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 2.dp)
                        .clickable { onSelectPage(hit.pageIndex) }
                ) {
                    Column(modifier = Modifier.padding(8.dp)) {
                        Text(
                            text = "Page ${hit.logicalPageLabel} (#${hit.physicalPageNumber}) — ${hit.matchCount} match(es)",
                            style = MaterialTheme.typography.labelLarge
                        )
                        Text(
                            text = hit.snippet,
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                }
            }
        }

        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Thumbnails & Logical Page Labels (${descriptors.size} pages):",
            style = MaterialTheme.typography.labelMedium
        )
        LazyColumn(modifier = Modifier.height(180.dp)) {
            items(descriptors) { descriptor ->
                ThumbnailRowItem(
                    sessionId = sessionId,
                    descriptor = descriptor,
                    isSelected = descriptor.pageIndex == currentPageIndex,
                    coreBridge = coreBridge,
                    onClick = { onSelectPage(descriptor.pageIndex) }
                )
            }
        }
    }
}

@Composable
private fun ThumbnailRowItem(
    sessionId: String,
    descriptor: FfiPageDescriptor,
    isSelected: Boolean,
    coreBridge: DocumentCoreBridge,
    onClick: () -> Unit
) {
    val thumbBitmap = remember(sessionId, descriptor.pageIndex) {
        runCatching {
            val thumb = coreBridge.renderPageThumbnail(sessionId, descriptor.pageIndex, 120u)
            val pixels = coreBridge.decodeThumbnailToArgbPixels(thumb)
            Bitmap.createBitmap(
                pixels,
                thumb.widthPx.toInt(),
                thumb.heightPx.toInt(),
                Bitmap.Config.ARGB_8888
            )
        }.getOrNull()
    }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 3.dp)
            .clickable(onClick = onClick)
    ) {
        Row(
            modifier = Modifier.padding(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            if (thumbBitmap != null) {
                Image(
                    bitmap = thumbBitmap.asImageBitmap(),
                    contentDescription = "Thumbnail page ${descriptor.logicalLabel}",
                    modifier = Modifier.size(width = 48.dp, height = 64.dp)
                )
                Spacer(modifier = Modifier.width(12.dp))
            }
            Column {
                Text(
                    text = "Logical: ${descriptor.logicalLabel}" + if (isSelected) " • Active" else "",
                    style = MaterialTheme.typography.bodyMedium
                )
                Text(
                    text = "Physical Page #${descriptor.physicalPageNumber}",
                    style = MaterialTheme.typography.bodySmall
                )
            }
        }
    }
}
