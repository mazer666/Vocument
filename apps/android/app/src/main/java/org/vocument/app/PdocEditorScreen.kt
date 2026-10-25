package org.vocument.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.vocument_ffi.FfiEditorSessionSnapshot

/**
 * Native Android Jetpack Compose FlowDocument editor screen (`ADR-0020`, `UI-009`, `MOBILE-001`, `MOBILE-002`).
 * Provides Gboard IME input, touch/stylus cursor placement, TalkBack accessibility, and sub-16ms layout.
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
    var inputBuffer by remember { mutableStateOf("") }

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

                    // Bullet List
                    IconButton(onClick = {
                        runCatching { session = coreBridge.editorInsertBulletList(session.editorId) }
                    }) {
                        Text("•")
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
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .background(Color(0xFFEFEFEF))
                .verticalScroll(rememberScrollState())
                .semantics {
                    contentDescription = "Vocument Schriftsatz: ${session.plainText}"
                }
        ) {
            // Hidden text input field driving Gboard and IME input
            OutlinedTextField(
                value = inputBuffer,
                onValueChange = { newText ->
                    if (newText.length > inputBuffer.length) {
                        val added = newText.substring(inputBuffer.length)
                        val start = System.nanoTime()
                        runCatching {
                            session = coreBridge.editorInsertText(session.editorId, added)
                        }
                        measuredLatencyMs = (System.nanoTime() - start) / 1_000_000.0
                    } else if (newText.length < inputBuffer.length) {
                        runCatching {
                            session = coreBridge.editorDeleteBackward(session.editorId)
                        }
                    }
                    inputBuffer = newText
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(8.dp),
                placeholder = { Text("Hier tippen (Gboard IME / Touch / Stift)...") },
                singleLine = false
            )

            // Render paginated FlowDocument pages
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
                                    val start = System.nanoTime()
                                    runCatching {
                                        session = coreBridge.editorHitTest(
                                            session.editorId,
                                            page.pageIndex,
                                            tapOffset.x,
                                            tapOffset.y
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
}
