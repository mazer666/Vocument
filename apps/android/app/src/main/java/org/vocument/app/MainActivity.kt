package org.vocument.app

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

/**
 * Native Android 13+ Activity using Jetpack Compose (`MOBILE-001`, `MOBILE-002`, `VIEW-001` Minimal Reader preset).
 * Opens PDFs via Android Storage Access Framework (`FileBroker`) and delegates all PDF session management
 * to the shared Rust `DocumentCore` over UniFFI (`ROADMAP.md` M0.1).
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
}

@Composable
fun MinimalReaderSessionScreen(
    handshake: DocumentCoreHandshake,
    coreBridge: DocumentCoreBridge
) {
    val context = LocalContext.current
    var activeSession by remember { mutableStateOf<FfiDocumentSessionSnapshot?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    val openPdfLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        if (uri != null) {
            runCatching {
                activeSession?.let { coreBridge.closeSession(it.sessionId) }
                openUriViaSafBroker(context, coreBridge, uri)
            }.onSuccess { snapshot ->
                activeSession = snapshot
                errorMessage = null
            }.onFailure { err ->
                errorMessage = err.message
            }
        }
    }

    Scaffold(
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = true,
                    onClick = {},
                    icon = {},
                    label = { Text("Pages") }
                )
                NavigationBarItem(
                    selected = false,
                    onClick = {},
                    icon = {},
                    label = { Text("Search") }
                )
                NavigationBarItem(
                    selected = false,
                    onClick = {},
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
                .padding(24.dp)
        ) {
            Text(
                text = "Vocument",
                style = MaterialTheme.typography.headlineMedium
            )
            Text(
                text = "Milestone ${handshake.milestone} — Baseline v${handshake.baselineVersion}",
                style = MaterialTheme.typography.bodyMedium
            )
            Spacer(modifier = Modifier.height(12.dp))
            Button(onClick = { openPdfLauncher.launch(arrayOf("application/pdf")) }) {
                Text("Open PDF (SAF)…")
            }

            errorMessage?.let { msg ->
                Spacer(modifier = Modifier.height(8.dp))
                Text(text = msg, color = MaterialTheme.colorScheme.error)
            }

            activeSession?.let { session ->
                Spacer(modifier = Modifier.height(16.dp))
                Text("Session ID: ${session.sessionId}")
                Text("Document ID: ${session.documentId}")
                Text("File: ${session.fileIdentity.displayName} (${session.fileIdentity.sizeBytes} B)")
                Text("SHA-256: ${session.fileIdentity.sha256Hex}")
                Text("Pages: ${session.pdfSummary.pageCount}")
                Text("Title: ${session.pdfSummary.title ?: "—"}")
                Text("Engine: ${session.pdfSummary.engineIdentifier}")
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
