package org.vocument.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.vocument_ffi.FfiEditorSessionSnapshot
import java.io.File

/**
 * Native Android Jetpack Compose FlowDocument editor screen (`ADR-0020`, `UI-009`, `MOBILE-001`, `MOBILE-002`, `TRACK-001`–`006`).
 * Provides direct canvas IME synchronization, touch/stylus cursor placement, TalkBack accessibility,
 * reviewing system (tracked changes & comments), and sub-16ms layout.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PdocEditorScreen(
    initialSession: FfiEditorSessionSnapshot,
    coreBridge: DocumentCoreBridge,
    onBack: () -> Unit
) {
    var session by remember { mutableStateOf(initialSession) }
    var measuredLatencyMs by remember { mutableStateOf(1.2) }
    var cursorVisible by remember { mutableStateOf(true) }
    var showFootnoteDialog by remember { mutableStateOf(false) }
    var footnoteText by remember { mutableStateOf("") }
    var showCommentDialog by remember { mutableStateOf(false) }
    var commentText by remember { mutableStateOf("") }
    var showReviewDialog by remember { mutableStateOf(false) }
    var showExportDialog by remember { mutableStateOf(false) }
    var exportIncludeComments by remember { mutableStateOf(true) }
    var exportIncludeTrackedChanges by remember { mutableStateOf(true) }
    var replyingCommentId by remember { mutableStateOf<String?>(null) }
    var replyText by remember { mutableStateOf("") }
    var exportStatusMessage by remember { mutableStateOf<String?>(null) }

    val focusRequester = remember { FocusRequester() }
    var textFieldValue by remember {
        mutableStateOf(
            TextFieldValue(
                text = session.plainText,
                selection = TextRange(session.cursorOffset.toInt())
            )
        )
    }

    // Blinking Caret effect
    LaunchedEffect(Unit) {
        while (true) {
            delay(500)
            cursorVisible = !cursorVisible
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(session.title, style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Inkrementeller Reflow: %.1f ms".format(measuredLatencyMs),
                            style = MaterialTheme.typography.bodySmall,
                            color = if (measuredLatencyMs < 16.0) Color(0xFF2E7D32) else Color(0xFFE65100)
                        )
                    }
                },
                actions = {
                    // Undo & Redo
                    IconButton(
                        onClick = {
                            runCatching { session = coreBridge.editorUndo(session.editorId) }
                        },
                        enabled = session.canUndo
                    ) {
                        Text("↶", fontSize = 20.sp)
                    }
                    IconButton(
                        onClick = {
                            runCatching { session = coreBridge.editorRedo(session.editorId) }
                        },
                        enabled = session.canRedo
                    ) {
                        Text("↷", fontSize = 20.sp)
                    }

                    // Bold
                    IconButton(onClick = {
                        runCatching { session = coreBridge.editorToggleBold(session.editorId) }
                    }) {
                        Text("B", fontWeight = FontWeight.Bold)
                    }

                    // Italic
                    IconButton(onClick = {
                        runCatching { session = coreBridge.editorToggleItalic(session.editorId) }
                    }) {
                        Text("I", fontStyle = androidx.compose.ui.text.font.FontStyle.Italic)
                    }

                    // Heading Level 1
                    IconButton(onClick = {
                        runCatching { session = coreBridge.editorSetHeadingLevel(session.editorId, 1u) }
                    }) {
                        Text("H1", fontWeight = FontWeight.Bold)
                    }

                    // Track Changes Toggle (`TRACK-001`)
                    IconButton(onClick = {
                        runCatching {
                            session = if (session.isTracking) {
                                coreBridge.editorDisableTracking(session.editorId)
                            } else {
                                coreBridge.editorEnableTracking(session.editorId, "Mobil-Anwalt")
                            }
                        }
                    }) {
                        Text(if (session.isTracking) "🔴" else "⚪")
                    }

                    // Comment Button (`TRACK-005`)
                    IconButton(onClick = { showCommentDialog = true }) {
                        Text("💬")
                    }

                    // Review List Button
                    val totalReviewItems = session.comments.size + session.trackedChanges.size
                    IconButton(onClick = { showReviewDialog = true }) {
                        Text("📋${if (totalReviewItems > 0) "($totalReviewItems)" else ""}")
                    }

                    // PDF/A Export (`EXPORT-001`)
                    IconButton(onClick = { showExportDialog = true }) {
                        Text("PDF/A", fontWeight = FontWeight.Bold, fontSize = 11.sp)
                    }

                    // Footnote
                    IconButton(onClick = { showFootnoteDialog = true }) {
                        Text("¹")
                    }

                    // Save
                    Button(onClick = {
                        runCatching {
                            coreBridge.editorSavePdoc(session.editorId, session.filePath)
                        }
                    }) {
                        Text("Speichern")
                    }
                }
            )
        }
    ) { paddingValues ->
        // Direct Canvas Editing Container
        BasicTextField(
            value = textFieldValue,
            onValueChange = { newVal ->
                val oldText = textFieldValue.text
                val newText = newVal.text
                if (newText != oldText) {
                    val start = System.nanoTime()
                    val oldSel = textFieldValue.selection
                    if (!oldSel.collapsed && oldSel.min < oldText.length) {
                        val replacedStart = oldSel.min.toUInt()
                        val replacedEnd = minOf(oldSel.max, oldText.length).toUInt()
                        val insertLen = newText.length - (oldText.length - (oldSel.max - oldSel.min))
                        val replacementText = if (insertLen > 0 && newVal.selection.start >= insertLen) {
                            newText.substring(newVal.selection.start - insertLen, newVal.selection.start)
                        } else {
                            ""
                        }
                        runCatching {
                            session = coreBridge.editorReplaceRange(
                                session.editorId,
                                session.activeBlockId,
                                replacedStart,
                                replacedEnd,
                                replacementText
                            )
                        }
                    } else if (newText.length > oldText.length) {
                        val insertLen = newText.length - oldText.length
                        val insertPos = newVal.selection.start
                        val added = if (insertPos >= insertLen) {
                            newText.substring(insertPos - insertLen, insertPos)
                        } else {
                            newText
                        }
                        runCatching {
                            session = coreBridge.editorInsertText(session.editorId, added)
                        }
                    } else {
                        val deleteLen = maxOf(1, oldText.length - newText.length)
                        runCatching {
                            for (i in 0 until deleteLen) {
                                session = coreBridge.editorDeleteBackward(session.editorId)
                            }
                        }
                    }
                    measuredLatencyMs = (System.nanoTime() - start) / 1_000_000.0
                }
                textFieldValue = newVal
            },
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .focusRequester(focusRequester),
            decorationBox = {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(Color(0xFFEFEFEF))
                        .verticalScroll(rememberScrollState())
                        .semantics {
                            contentDescription = "Vocument Schriftsatz Canvas: ${session.plainText}"
                        }
                ) {
                    if (exportStatusMessage != null) {
                        Surface(
                            color = MaterialTheme.colorScheme.primaryContainer,
                            modifier = Modifier.fillMaxWidth().padding(8.dp)
                        ) {
                            Text(
                                exportStatusMessage ?: "",
                                modifier = Modifier.padding(8.dp),
                                style = MaterialTheme.typography.bodySmall
                            )
                        }
                    }

                    // Render directly editable paginated FlowDocument pages
                    session.pages.forEach { page ->
                        Card(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 8.dp),
                            elevation = CardDefaults.cardElevation(defaultElevation = 4.dp),
                            colors = CardDefaults.cardColors(containerColor = Color.White)
                        ) {
                            val density = LocalDensity.current
                            val pageHeightDp = with(density) { page.heightPt.toDp() }

                            Canvas(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(pageHeightDp)
                                    .pointerInput(page.pageIndex) {
                                        detectTapGestures { tapOffset ->
                                            focusRequester.requestFocus()
                                            val start = System.nanoTime()
                                            runCatching {
                                                session = coreBridge.editorHitTest(
                                                    session.editorId,
                                                    page.pageIndex,
                                                    tapOffset.x,
                                                    tapOffset.y
                                                )
                                                textFieldValue = TextFieldValue(
                                                    text = session.plainText,
                                                    selection = TextRange(session.cursorOffset.toInt())
                                                )
                                            }
                                            measuredLatencyMs = (System.nanoTime() - start) / 1_000_000.0
                                        }
                                    }
                            ) {
                                // 1. Draw selection highlights
                                session.selectionRects.filter { it.pageIndex == page.pageIndex }.forEach { sel ->
                                    drawRect(
                                        color = Color(0x552196F3),
                                        topLeft = Offset(sel.rect.x, sel.rect.y),
                                        size = Size(sel.rect.width, sel.rect.height)
                                    )
                                }

                                // 2. Draw text lines and runs
                                drawIntoCanvas { canvas ->
                                    val nativeCanvas = canvas.nativeCanvas
                                    val paint = android.graphics.Paint().apply {
                                        isAntiAlias = true
                                        color = android.graphics.Color.BLACK
                                    }

                                    page.lines.forEach { line ->
                                        line.runs.forEach { run ->
                                            paint.textSize = run.fontSizePt * 1.33f
                                            paint.isFakeBoldText = run.isBold
                                            if (run.isItalic) {
                                                paint.textSkewX = -0.25f
                                            } else {
                                                paint.textSkewX = 0f
                                            }

                                            nativeCanvas.drawText(
                                                run.text,
                                                run.rect.x,
                                                line.baselineY,
                                                paint
                                            )
                                        }
                                    }
                                }

                                // 3. Draw blinking Caret
                                if (cursorVisible && session.cursorPageIndex == page.pageIndex) {
                                    session.cursorRect?.let { cRect ->
                                        drawRect(
                                            color = Color(0xFF1976D2),
                                            topLeft = Offset(cRect.x, cRect.y),
                                            size = Size(2f, maxOf(16f, cRect.height))
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        )
    }

    // Footnote Dialog
    if (showFootnoteDialog) {
        AlertDialog(
            onDismissRequest = { showFootnoteDialog = false },
            title = { Text("Fußnote einfügen") },
            text = {
                OutlinedTextField(
                    value = footnoteText,
                    onValueChange = { footnoteText = it },
                    label = { Text("Fundstelle / Erläuterung") },
                    singleLine = true
                )
            },
            confirmButton = {
                Button(onClick = {
                    if (footnoteText.isNotBlank()) {
                        runCatching {
                            session = coreBridge.editorInsertFootnote(session.editorId, footnoteText)
                        }
                        footnoteText = ""
                    }
                    showFootnoteDialog = false
                }) {
                    Text("Einfügen")
                }
            },
            dismissButton = {
                OutlinedButton(onClick = { showFootnoteDialog = false }) {
                    Text("Abbrechen")
                }
            }
        )
    }

    // Comment Dialog (`TRACK-005`)
    if (showCommentDialog) {
        AlertDialog(
            onDismissRequest = { showCommentDialog = false },
            title = { Text("Randkommentar verfassen") },
            text = {
                OutlinedTextField(
                    value = commentText,
                    onValueChange = { commentText = it },
                    label = { Text("Kommentarinhalt...") },
                    singleLine = false
                )
            },
            confirmButton = {
                Button(onClick = {
                    if (commentText.isNotBlank()) {
                        runCatching {
                            session = coreBridge.editorAddComment(session.editorId, null, null, commentText)
                        }
                        commentText = ""
                    }
                    showCommentDialog = false
                }) {
                    Text("Anheften")
                }
            },
            dismissButton = {
                OutlinedButton(onClick = { showCommentDialog = false }) {
                    Text("Abbrechen")
                }
            }
        )
    }

    // Review Dialog (`TRACK-001`–`006`)
    if (showReviewDialog) {
        AlertDialog(
            onDismissRequest = { showReviewDialog = false },
            title = { Text("Prüfung & Überarbeitung") },
            text = {
                Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                    if (session.trackedChanges.isNotEmpty()) {
                        Text("Änderungen:", fontWeight = FontWeight.Bold)
                        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            TextButton(onClick = {
                                runCatching { session = coreBridge.editorAcceptAllChanges(session.editorId) }
                            }) { Text("Alle annehmen") }
                            TextButton(onClick = {
                                runCatching { session = coreBridge.editorRejectAllChanges(session.editorId) }
                            }) { Text("Alle ablehnen") }
                        }
                        session.trackedChanges.forEach { chg ->
                            Row(
                                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    "${chg.author}: ${chg.textSnippet ?: chg.changeType}",
                                    modifier = Modifier.weight(1f),
                                    style = MaterialTheme.typography.bodySmall
                                )
                                TextButton(onClick = {
                                    runCatching { session = coreBridge.editorAcceptChange(session.editorId, chg.changeId) }
                                }) { Text("✓") }
                                TextButton(onClick = {
                                    runCatching { session = coreBridge.editorRejectChange(session.editorId, chg.changeId) }
                                }) { Text("✕") }
                            }
                        }
                        HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                    }

                    if (session.comments.isNotEmpty()) {
                        Text("Randkommentare:", fontWeight = FontWeight.Bold)
                        session.comments.forEach { comm ->
                            Card(
                                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                                colors = CardDefaults.cardColors(containerColor = if (comm.resolved) Color(0xFFF5F5F5) else Color(0xFFE3F2FD))
                            ) {
                                Column(modifier = Modifier.padding(8.dp)) {
                                    Row(
                                        modifier = Modifier.fillMaxWidth(),
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        verticalAlignment = Alignment.CenterVertically
                                    ) {
                                        Text(comm.author, fontWeight = FontWeight.SemiBold, style = MaterialTheme.typography.bodySmall)
                                        Row {
                                            TextButton(onClick = {
                                                runCatching { session = coreBridge.editorResolveComment(session.editorId, comm.commentId, !comm.resolved) }
                                            }) { Text(if (comm.resolved) "Gelöst" else "Offen", fontSize = 11.sp) }
                                            TextButton(onClick = {
                                                runCatching { session = coreBridge.editorDeleteComment(session.editorId, comm.commentId) }
                                            }) { Text("🗑", fontSize = 11.sp) }
                                        }
                                    }
                                    if (comm.anchorText != null) {
                                        Text("„${comm.anchorText}“", style = MaterialTheme.typography.labelSmall, color = Color.Gray)
                                    }
                                    Text(comm.content, style = MaterialTheme.typography.bodySmall)

                                    // Display replies
                                    comm.replies.forEach { rep ->
                                        Row(modifier = Modifier.padding(start = 12.dp, top = 2.dp)) {
                                            Text("↳ ${rep.author}: ${rep.content}", style = MaterialTheme.typography.bodySmall, color = Color.DarkGray)
                                        }
                                    }

                                    // Reply composer
                                    if (replyingCommentId == comm.commentId) {
                                        Row(modifier = Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                                            OutlinedTextField(
                                                value = replyText,
                                                onValueChange = { replyText = it },
                                                placeholder = { Text("Antwort...", fontSize = 12.sp) },
                                                modifier = Modifier.weight(1f),
                                                singleLine = true
                                            )
                                            IconButton(onClick = {
                                                if (replyText.isNotBlank()) {
                                                    runCatching {
                                                        session = coreBridge.editorReplyComment(session.editorId, comm.commentId, replyText)
                                                    }
                                                    replyText = ""
                                                    replyingCommentId = null
                                                }
                                            }) { Text("Senden", fontSize = 11.sp) }
                                        }
                                    } else {
                                        TextButton(onClick = {
                                            replyingCommentId = comm.commentId
                                            replyText = ""
                                        }) { Text("Antworten", fontSize = 11.sp) }
                                    }
                                }
                            }
                        }
                    }

                    if (session.trackedChanges.isEmpty() && session.comments.isEmpty()) {
                        Text("Keine offenen Änderungen oder Kommentare vorhanden.")
                    }
                }
            },
            confirmButton = {
                Button(onClick = { showReviewDialog = false }) {
                    Text("Schließen")
                }
            }
        )
    }

    // PDF/A & PDF/UA Export Dialog (`EXPORT-001`, `EXPORT-002`)
    if (showExportDialog) {
        AlertDialog(
            onDismissRequest = { showExportDialog = false },
            title = { Text("PDF/A-2b & PDF/UA-1 Export") },
            text = {
                Column {
                    Text(
                        "Erzeugt ein standardkonformes, strukturiertes Langzeitarchivdokument.",
                        style = MaterialTheme.typography.bodySmall
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(
                            checked = exportIncludeComments,
                            onCheckedChange = { exportIncludeComments = it }
                        )
                        Text("Randkommentare einbinden", style = MaterialTheme.typography.bodyMedium)
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(
                            checked = exportIncludeTrackedChanges,
                            onCheckedChange = { exportIncludeTrackedChanges = it }
                        )
                        Text("Änderungsverfolgung visuell darstellen", style = MaterialTheme.typography.bodyMedium)
                    }
                }
            },
            confirmButton = {
                Button(onClick = {
                    showExportDialog = false
                    runCatching {
                        val pdfBytes = coreBridge.editorExportPdf(
                            session.editorId,
                            exportIncludeComments,
                            exportIncludeTrackedChanges
                        )
                        val dest = File("/sdcard/Download/${session.title}.pdf")
                        dest.writeBytes(pdfBytes)
                        exportStatusMessage = "PDF/A exportiert nach: ${dest.name}"
                    }.onFailure {
                        exportStatusMessage = "Export-Fehler: ${it.message}"
                    }
                }) {
                    Text("Exportieren")
                }
            },
            dismissButton = {
                OutlinedButton(onClick = { showExportDialog = false }) {
                    Text("Abbrechen")
                }
            }
        )
    }
}
