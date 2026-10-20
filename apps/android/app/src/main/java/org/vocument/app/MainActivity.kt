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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import android.os.Handler
import android.os.Looper
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import uniffi.vocument_ffi.DocumentCoreHandshake
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiPageDescriptor
import uniffi.vocument_ffi.FfiSaveConflict
import uniffi.vocument_ffi.FfiSaveConflictResolution
import uniffi.vocument_ffi.FfiSaveOutcome
import uniffi.vocument_ffi.FfiSaveSuccessSnapshot
import uniffi.vocument_ffi.FfiSearchIndexProgress
import uniffi.vocument_ffi.FfiViewportRenderResult

/**
 * The result of a save: either the path of the written file, or the conflict
 * that the user has to resolve.
 */
private data class SaveResult(
    val path: String?,
    val message: String?,
    val conflict: FfiSaveConflict?
)

/**
 * Interval in which the energy and thermal state of the device is reported to
 * the core (`POWER-001`).
 *
 * The state changes rarely; a minute is enough to notice that the device is
 * charging or that it got hot, without waking the device up all the time.
 */
private const val POWER_REPORT_INTERVAL_MS = 60_000L

/**
 * Native Android 13+ Activity using Jetpack Compose (`MOBILE-001`, `MOBILE-002`, `BIGMOBILE-001`, `VIEW-001`).
 * Opens PDFs via Android Storage Access Framework (`FileBroker`), renders tiles via `DocumentActor` +
 * `TileCache`, provides M0.3 thumbnails, logical page labels, cleaned text copy, and progressive
 * SQLite FTS5 search, and forwards Android `onTrimMemory` pressure signals to `DocumentCore` (`ROADMAP.md` M0.3).
 */
class MainActivity : ComponentActivity() {
    private val coreBridge = DocumentCoreBridge()
    private val powerReporterHandler = Handler(Looper.getMainLooper())
    private val powerReporterRunnable = object : Runnable {
        override fun run() {
            reportPowerState()
            powerReporterHandler.postDelayed(this, POWER_REPORT_INTERVAL_MS)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val tessdataDir = TessdataProvisioner.ensureTessdata(this)
        coreBridge.configureTessdataPath(tessdataDir.absolutePath)

        val runOcr = intent?.getStringExtra("action") == "verify-ocr" ||
            intent?.getBooleanExtra("run_ocr_verification", false) == true
        if (runOcr) {
            AndroidOcrRuntimeGate.runVerification(this, coreBridge)
        }

        setContent {
            MaterialTheme {
                val handshake = remember { coreBridge.performHandshake() }
                MinimalReaderSessionScreen(handshake = handshake, coreBridge = coreBridge)
            }
        }
    }

    /**
     * Reports the energy and thermal state of the device to the shared core.
     *
     * The state is reported when the application comes to the foreground and
     * then periodically, because a device that is charging or that gets hot
     * does not announce itself.
     */
    override fun onStart() {
        super.onStart()
        reportPowerState()
        powerReporterHandler.postDelayed(powerReporterRunnable, POWER_REPORT_INTERVAL_MS)
    }

    override fun onStop() {
        powerReporterHandler.removeCallbacks(powerReporterRunnable)
        super.onStop()
    }

    private fun reportPowerState() {
        runCatching { AndroidPowerReporter.report(this) }
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
    // The path of the last successful save, shown to the user. A scan that
    // cannot be saved is a scan that is lost, so the result has to be visible.
    var lastSavedPath by remember { mutableStateOf<String?>(null) }
    var isSaving by remember { mutableStateOf(false) }
    var statusMessage by remember { mutableStateOf<String?>(null) }
    // The conflict of a save, if the file was changed outside the application.
    // The user decides how it is resolved (`FILELOCK-004`).
    var saveConflict by remember { mutableStateOf<FfiSaveConflict?>(null) }

    // The viewport rendering runs on a background thread. The UI thread only
    // receives the finished result.
    val viewportExecutor = remember { Executors.newSingleThreadExecutor() }
    // The executor owns a thread. It has to be shut down when the composition
    // leaves, otherwise every recreation of the activity leaves a thread
    // behind.
    DisposableEffect(Unit) {
        onDispose { viewportExecutor.shutdown() }
    }
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    // A plain counter, not a Compose state: it is read on the background
    // thread and must not trigger a recomposition.
    val viewportRequestCounter = remember { AtomicInteger(0) }

    fun refreshViewport(
        session: FfiDocumentSessionSnapshot,
        pageIndex: UInt,
        scale: UInt
    ) {
        // The viewport render and the prefetch of the neighbouring pages call
        // into MuPDF and block until the rendering is finished. They must not
        // run on the UI thread: a page change would freeze the interface for
        // the whole rendering.
        //
        // The request counter makes the result of an outdated request
        // obsolete. A fast page change therefore cannot show the result of a
        // previous page.
        val requestId = viewportRequestCounter.incrementAndGet()
        viewportExecutor.execute {
            val rendered = runCatching {
                val req = buildViewportRenderRequest(coreBridge, session.sessionId, pageIndex, scale)
                coreBridge.renderViewportWithPrefetch(session.sessionId, req)
            }
            mainHandler.post {
                if (requestId != viewportRequestCounter.get()) {
                    // A newer request was started in the meantime: its result
                    // is the one that matters.
                    return@post
                }
                rendered.onSuccess { result ->
                    currentPageIndex = pageIndex
                    scaleMilli = scale
                    viewportResult = result
                    errorMessage = null
                }.onFailure { err ->
                    errorMessage = err.message
                }
            }
        }
    }

    // The save writes the file and syncs it to the disk. Both block, so the
    // work happens on a background executor and only the result reaches the
    // interface.
    val saveExecutor = remember { Executors.newSingleThreadExecutor() }
    DisposableEffect(Unit) {
        onDispose { saveExecutor.shutdown() }
    }

    /**
     * Saves the active session into the documents directory of the application.
     *
     * A file that was changed outside the application is never overwritten
     * silently. The conflict is reported and the user decides how it is
     * resolved (`FILELOCK-004`).
     */
    fun saveActiveSession(context: Context) {
        val session = activeSession ?: return
        isSaving = true
        saveExecutor.execute {
            val result = runCatching {
                // The scan is saved into the documents directory of the
                // application, with a name that cannot collide with an
                // existing file.
                val documents = context.getExternalFilesDir(null)
                    ?: context.filesDir
                val stamp = java.text.SimpleDateFormat("yyyyMMdd_HHmmss", java.util.Locale.US)
                    .format(java.util.Date())
                val target = java.io.File(documents, "Vocument_Scan_$stamp.pdf")
                val outcome = coreBridge.saveSessionAs(session.sessionId, target.absolutePath)
                when (outcome) {
                    is FfiSaveOutcome.Success -> SaveResult(
                        target.absolutePath,
                        "Saved ${outcome.snapshot.bytesWritten} bytes to ${outcome.snapshot.targetToken}",
                        null
                    )
                    is FfiSaveOutcome.Conflict -> SaveResult(
                        null,
                        null,
                        outcome.conflict
                    )
                }
            }
            mainHandler.post {
                isSaving = false
                result.onSuccess { outcome ->
                    lastSavedPath = outcome.path
                    errorMessage = null
                    statusMessage = outcome.message
                    saveConflict = outcome.conflict
                }.onFailure { err ->
                    errorMessage = "Save error: ${err.message}"
                }
            }
        }
    }

    /**
     * Resolves a save conflict in the way the user chose.
     */
    fun resolveConflict(resolution: FfiSaveConflictResolution) {
        val session = activeSession ?: return
        val conflict = saveConflict ?: return
        isSaving = true
        saveExecutor.execute {
            val result = runCatching {
                coreBridge.resolveSaveConflict(session.sessionId, resolution)
            }
            mainHandler.post {
                isSaving = false
                saveConflict = null
                result.onSuccess { snapshot ->
                    if (snapshot != null) {
                        lastSavedPath = snapshot.targetToken
                        statusMessage = "Saved ${snapshot.bytesWritten} bytes to ${snapshot.targetToken}"
                        errorMessage = null
                    } else {
                        statusMessage = "The save was cancelled; the document stays unchanged."
                    }
                }.onFailure { err ->
                    errorMessage = "Conflict resolution failed: ${err.message}"
                }
            }
        }
    }

    // The export writes the document into a location that the user owns. The
    // user chooses the place, the application writes the bytes into the
    // document provider.
    val exportPdfLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.CreateDocument("application/pdf")
    ) { uri: Uri? ->
        val session = activeSession
        if (uri != null && session != null) {
            exportSessionToSaf(context, coreBridge, session.sessionId, uri) { path, message ->
                if (path != null) {
                    lastSavedPath = path
                    statusMessage = message
                    errorMessage = null
                } else {
                    errorMessage = message
                }
            }
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
                NavigationBarItem(
                    selected = activeTab == 3,
                    onClick = { activeTab = 3 },
                    icon = {},
                    label = { Text("Scan (M1)") }
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

            // A scanned document must be savable. Without this action the
            // result of a scan would only exist in memory.
            if (activeSession != null) {
                Spacer(modifier = Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(
                        enabled = !isSaving,
                        onClick = { saveActiveSession(context) }
                    ) {
                        Text(if (isSaving) "Saving…" else "Save PDF")
                    }
                    // The export writes the document into a place that the user
                    // owns: a file the user chose, in their own storage.
                    OutlinedButton(
                        enabled = !isSaving,
                        onClick = { exportPdfLauncher.launch("Vocument_Scan.pdf") }
                    ) {
                        Text("Export PDF…")
                    }
                }
                lastSavedPath?.let { path ->
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = "Saved: $path",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
                statusMessage?.let { message ->
                    Text(
                        text = message,
                        style = MaterialTheme.typography.bodySmall
                    )
                }
            }

            errorMessage?.let { msg ->
                Spacer(modifier = Modifier.height(8.dp))
                Text(text = msg, color = MaterialTheme.colorScheme.error)
            }

            // A file that was changed outside the application is never
            // overwritten silently. The user decides how the conflict is
            // resolved (`FILELOCK-004`).
            saveConflict?.let { conflict ->
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = "The file was changed outside the application (${conflict.comparisonSummary}). How should it be handled?",
                    style = MaterialTheme.typography.bodySmall
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(
                        onClick = {
                            resolveConflict(
                                FfiSaveConflictResolution.SAVE_LOCAL_AS_COLLISION_FREE_COPY
                            )
                        }
                    ) {
                        Text("Save as copy")
                    }
                    OutlinedButton(
                        onClick = {
                            resolveConflict(
                                FfiSaveConflictResolution.OVERWRITE_EXTERNAL_EXPLICITLY
                            )
                        }
                    ) {
                        Text("Overwrite")
                    }
                    OutlinedButton(
                        onClick = { resolveConflict(FfiSaveConflictResolution.RELOAD_EXTERNAL_VERSION) }
                    ) {
                        Text("Reload")
                    }
                    OutlinedButton(
                        onClick = { resolveConflict(FfiSaveConflictResolution.CANCEL) }
                    ) {
                        Text("Cancel")
                    }
                }
            }

            if (activeTab == 3) {
                Spacer(modifier = Modifier.height(8.dp))
                ScanCaptureScreen(
                    coreBridge = coreBridge,
                    onDocumentSessionOpened = { newSession ->
                        activeSession = newSession
                        currentPageIndex = 0u
                        refreshViewport(newSession, 0u, scaleMilli)
                        activeTab = 0
                    }
                )
            } else {
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
}

/**
 * Writes the document of the active session into a location that the user
 * chose.
 *
 * The bytes come from the core, the document provider does the writing. The
 * core has no access to a `content://` URI, so it hands the bytes over.
 */
private fun exportSessionToSaf(
    context: Context,
    coreBridge: DocumentCoreBridge,
    sessionId: String,
    uri: Uri,
    onResult: (String?, String?) -> Unit = { _, _ -> }
) {
    Thread {
        val result = runCatching {
            val bytes = coreBridge.exportSessionBytes(sessionId)
            context.contentResolver.openOutputStream(uri, "wt")?.use { stream ->
                stream.write(bytes)
                stream.flush()
            } ?: error("The document provider did not open the file for writing")
            bytes.size
        }
        android.os.Handler(android.os.Looper.getMainLooper()).post {
            result.onSuccess { size ->
                onResult(uri.toString(), "Exported $size bytes to $uri")
            }.onFailure { err ->
                onResult(null, "Export failed: ${err.message}")
            }
        }
    }.start()
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
