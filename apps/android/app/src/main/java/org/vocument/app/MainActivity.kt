package org.vocument.app

import android.content.ComponentCallbacks2
import android.content.Context
import android.net.Uri
import android.os.Bundle
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.vocument_ffi.DocumentCoreHandshake
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiPageDescriptor
import uniffi.vocument_ffi.FfiSearchIndexProgress
import uniffi.vocument_ffi.FfiViewportRenderResult

/**
 * Native Android 13+ Activity using Jetpack Compose (`MOBILE-001`, `MOBILE-002`, `BIGMOBILE-001`, `VIEW-001`).
 * Opens PDFs via Android Storage Access Framework (`FileBroker`), renders tiles via `DocumentActor` +
 * `TileCache`, provides M0.3 thumbnails, logical page labels, cleaned text copy, and progressive
 * SQLite FTS5 search, and forwards Android `onTrimMemory` pressure signals to `DocumentCore` (`ROADMAP.md` M0.3).
 */
class MainActivity : ComponentActivity() {
    private val coreBridge = DocumentCoreBridge()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                val handshake = remember { coreBridge.performHandshake() }
                MinimalReaderSessionScreen(handshake = handshake, coreBridge = coreBridge)
            }
        }
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        val pressureLevel = when {
            level >= ComponentCallbacks2.TRIM_MEMORY_COMPLETE ||
                level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ->
                FfiMemoryPressureLevel.CRITICAL
            level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
                level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND ->
                FfiMemoryPressureLevel.WARNING
            else -> FfiMemoryPressureLevel.NORMAL
        }
        runCatching { coreBridge.handleMemoryPressure(pressureLevel) }
    }
}

@Composable
fun MinimalReaderSessionScreen(
    handshake: DocumentCoreHandshake,
    coreBridge: DocumentCoreBridge
) {
    val context = LocalContext.current
    var activeSession by remember { mutableStateOf<FfiDocumentSessionSnapshot?>(null) }
    var currentPageIndex by remember { mutableStateOf(0u) }
    var scaleMilli by remember { mutableStateOf(1000u) }
    var viewportResult by remember { mutableStateOf<FfiViewportRenderResult?>(null) }
    var pageDescriptors by remember { mutableStateOf<List<FfiPageDescriptor>>(emptyList()) }
    var indexProgress by remember { mutableStateOf<FfiSearchIndexProgress?>(null) }
    var activeTab by remember { mutableStateOf(0) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    fun refreshViewport(
        session: FfiDocumentSessionSnapshot,
        pageIndex: UInt,
        scale: UInt
    ) {
        runCatching {
            val req = buildViewportRenderRequest(coreBridge, session.sessionId, pageIndex, scale)
            coreBridge.renderViewportWithPrefetch(session.sessionId, req)
        }.onSuccess { rendered ->
            currentPageIndex = pageIndex
            scaleMilli = scale
            viewportResult = rendered
            errorMessage = null
        }.onFailure { err ->
            errorMessage = err.message
        }
    }

    val openPdfLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        if (uri != null) {
            runCatching {
                activeSession?.let { coreBridge.closeSession(it.sessionId) }
                openUriViaSafBroker(context, coreBridge, uri)
            }.onSuccess { snapshot ->
                activeSession = snapshot
                pageDescriptors = runCatching { coreBridge.pageDescriptors(snapshot.sessionId) }.getOrDefault(emptyList())
                indexProgress = runCatching { coreBridge.indexSearchBatch(snapshot.sessionId, 0u, 2u) }.getOrNull()
                refreshViewport(snapshot, pageIndex = 0u, scale = 1000u)
            }.onFailure { err ->
                errorMessage = err.message
            }
        }
    }

    Scaffold(
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = activeTab == 0,
                    onClick = { activeTab = 0 },
                    icon = {},
                    label = { Text("Pages") }
                )
                NavigationBarItem(
                    selected = activeTab == 1,
                    onClick = { activeTab = 1 },
                    icon = {},
                    label = { Text("Nav & Search") }
                )
                NavigationBarItem(
                    selected = activeTab == 2,
                    onClick = { activeTab = 2 },
                    icon = {},
                    label = { Text("Tools") }
                )
            }
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(16.dp)
        ) {
            Text(
                text = "Vocument",
                style = MaterialTheme.typography.headlineSmall
            )
            Text(
                text = "Milestone ${handshake.milestone} — Baseline v${handshake.baselineVersion}",
                style = MaterialTheme.typography.bodySmall
            )
            Spacer(modifier = Modifier.height(8.dp))
            Button(onClick = { openPdfLauncher.launch(arrayOf("application/pdf")) }) {
                Text("Open PDF (SAF)…")
            }

            errorMessage?.let { msg ->
                Spacer(modifier = Modifier.height(8.dp))
                Text(text = msg, color = MaterialTheme.colorScheme.error)
            }

            val session = activeSession
            val currentViewport = viewportResult
            if (session != null && currentViewport != null) {
                Spacer(modifier = Modifier.height(8.dp))
                when (activeTab) {
                    1 -> NavigationAndSearchPanel(
                        sessionId = session.sessionId,
                        currentPageIndex = currentPageIndex,
                        descriptors = pageDescriptors,
                        indexProgress = indexProgress,
                        coreBridge = coreBridge,
                        onSelectPage = { targetPage ->
                            refreshViewport(session, targetPage, scaleMilli)
                            activeTab = 0
                        },
                        onIndexNextBatch = {
                            indexProgress = runCatching {
                                coreBridge.indexSearchBatch(session.sessionId, currentPageIndex, 2u)
                            }.getOrNull()
                        },
                        onError = { err -> errorMessage = err }
                    )
                    2 -> MutationToolsPanel(
                        sessionId = session.sessionId,
                        currentPageIndex = currentPageIndex,
                        coreBridge = coreBridge,
                        onMutationApplied = {
                            activeSession = runCatching {
                                coreBridge.getSessionSnapshot(session.sessionId)
                            }.getOrDefault(session)
                            refreshViewport(session, currentPageIndex, scaleMilli)
                        },
                        onError = { err -> errorMessage = err }
                    )
                    else -> DocumentCanvas(
                        session = session,
                        currentPageIndex = currentPageIndex,
                        scaleMilli = scaleMilli,
                        viewportResult = currentViewport,
                        coreBridge = coreBridge,
                        onPageChange = { nextPage -> refreshViewport(session, nextPage, scaleMilli) },
                        onScaleChange = { nextScale -> refreshViewport(session, currentPageIndex, nextScale) },
                        onMemoryPressure = { level ->
                            runCatching {
                                coreBridge.handleMemoryPressure(level)
                                if (level == FfiMemoryPressureLevel.CRITICAL) {
                                    coreBridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)
                                }
                                refreshViewport(session, currentPageIndex, scaleMilli)
                            }
                        }
                    )
                }
            }
        }
    }
}

private fun openUriViaSafBroker(
    context: Context,
    coreBridge: DocumentCoreBridge,
    uri: Uri
): FfiDocumentSessionSnapshot {
    val resolver = context.contentResolver
    var displayName = uri.lastPathSegment ?: "document.pdf"
    var lastModified: Long? = null
    resolver.query(uri, null, null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) {
            val nameIdx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (nameIdx >= 0 && !cursor.isNull(nameIdx)) {
                displayName = cursor.getString(nameIdx)
            }
            val modIdx = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_LAST_MODIFIED)
            if (modIdx >= 0 && !cursor.isNull(modIdx)) {
                lastModified = cursor.getLong(modIdx)
            }
        }
    }
    val docId = runCatching { DocumentsContract.getDocumentId(uri) }.getOrNull()
    val bytes = resolver.openInputStream(uri)?.use { it.readBytes() }
        ?: error("Unable to open InputStream for SAF URI $uri")

    return coreBridge.openSafDocument(
        safUriString = uri.toString(),
        displayName = displayName,
        safDocumentId = docId,
        lastModifiedUnixMs = lastModified,
        bytes = bytes
    )
}
