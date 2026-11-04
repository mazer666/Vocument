package org.vocument.app

import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.vocument_ffi.*

/**
 * Native Android Jetpack Compose Document Comparison & Diffing Screen (`M3.7`, `COMPARE-001`–`004`).
 * Displays structural text diffs, visual page diffs, legal clause navigation, and report export.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CompareScreen(
    report: FfiComparisonReport,
    onBack: () -> Unit,
    onExportReport: (format: String) -> Unit = {},
    coreBridge: DocumentCoreBridge? = null,
    sessionIdA: String? = null,
    sessionIdB: String? = null
) {
    var selectedChangeId by remember { mutableStateOf(report.changes.firstOrNull()?.id) }
    var syncScroll by remember { mutableStateOf(true) }
    var showVisualOverlay by remember { mutableStateOf(true) }
    var activeTab by remember { mutableStateOf(0) } // 0: Changes List, 1: Dual Viewports, 2: Report

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            text = "Dokumentenvergleich",
                            fontWeight = FontWeight.Bold,
                            fontSize = 18.sp
                        )
                        Text(
                            text = "${report.docATitle} ↔ ${report.docBTitle}",
                            fontSize = 12.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                },
                navigationIcon = {
                    TextButton(onClick = onBack) {
                        Text("← Zurück")
                    }
                },
                actions = {
                    TextButton(onClick = { onExportReport("html") }) {
                        Text("Export")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // Summary KPI bar
            ComparisonSummaryBar(summary = report.summary)

            // Tab navigation
            TabRow(selectedTabIndex = activeTab) {
                Tab(
                    selected = activeTab == 0,
                    onClick = { activeTab = 0 },
                    text = { Text("Änderungen (${report.changes.size})") }
                )
                Tab(
                    selected = activeTab == 1,
                    onClick = { activeTab = 1 },
                    text = { Text("Gegenüberstellung") }
                )
                Tab(
                    selected = activeTab == 2,
                    onClick = { activeTab = 2 },
                    text = { Text("Bericht") }
                )
            }

            when (activeTab) {
                0 -> ChangesListView(
                    changes = report.changes,
                    selectedId = selectedChangeId,
                    onSelectChange = { selectedChangeId = it }
                )
                1 -> DualViewportView(
                    report = report,
                    selectedChangeId = selectedChangeId,
                    syncScroll = syncScroll,
                    onToggleSync = { syncScroll = !syncScroll },
                    showVisualOverlay = showVisualOverlay,
                    onToggleVisual = { showVisualOverlay = !showVisualOverlay },
                    coreBridge = coreBridge,
                    sessionIdA = sessionIdA,
                    sessionIdB = sessionIdB
                )
                2 -> ReportPreviewView(
                    markdownReport = report.markdownReport,
                    onExportReport = onExportReport
                )
            }
        }
    }
}

@Composable
fun ComparisonSummaryBar(summary: FfiComparisonSummary) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceVariant,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                KpiPill(label = "+${summary.insertionsCount}", color = Color(0xFF2E7D32), desc = "Einfügungen")
                KpiPill(label = "-${summary.deletionsCount}", color = Color(0xFFC62828), desc = "Löschungen")
                KpiPill(label = "~${summary.replacementsCount}", color = Color(0xFFEF6C00), desc = "Ersetzungen")
                KpiPill(label = "⇄${summary.movesCount}", color = Color(0xFF1565C0), desc = "Verschiebungen")
            }

            val similarityPercent = (summary.similarityRatio * 100f).toInt()
            Text(
                text = "$similarityPercent% Ähnlichkeit",
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
fun KpiPill(label: String, color: Color, desc: String) {
    Surface(
        color = color.copy(alpha = 0.15f),
        shape = MaterialTheme.shapes.small,
        modifier = Modifier.semantics { contentDescription = "$label $desc" }
    ) {
        Text(
            text = label,
            color = color,
            fontWeight = FontWeight.Bold,
            fontSize = 12.sp,
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
        )
    }
}

@Composable
fun ChangesListView(
    changes: List<FfiChangeItem>,
    selectedId: String?,
    onSelectChange: (String) -> Unit
) {
    if (changes.isEmpty()) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = "Keine Unterschiede gefunden. Dokumente sind identisch.",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    } else {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            items(changes, key = { it.id }) { item ->
                ChangeItemCard(
                    item = item,
                    isSelected = item.id == selectedId,
                    onClick = { onSelectChange(item.id) }
                )
            }
        }
    }
}

@Composable
fun ChangeItemCard(
    item: FfiChangeItem,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    val borderColor = if (isSelected) MaterialTheme.colorScheme.primary else Color.Transparent

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(
            containerColor = if (isSelected) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.3f)
            else MaterialTheme.colorScheme.surface
        ),
        border = CardDefaults.outlinedCardBorder().copy(
            brush = androidx.compose.ui.graphics.SolidColor(borderColor)
        )
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    DiffOperationTag(op = item.operation)
                    item.clauseHint?.let { clause ->
                        Text(
                            text = clause,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.secondary
                        )
                    }
                }

                item.pageA?.let { pA ->
                    Text(
                        text = "S. ${pA + 1u}",
                        fontSize = 10.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            Spacer(modifier = Modifier.height(6.dp))

            item.textA?.let { textA ->
                if (textA.isNotBlank()) {
                    Text(
                        text = textA,
                        color = Color(0xFFC62828),
                        textDecoration = TextDecoration.LineThrough,
                        fontSize = 12.sp,
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }

            item.textB?.let { textB ->
                if (textB.isNotBlank()) {
                    Text(
                        text = textB,
                        color = Color(0xFF2E7D32),
                        textDecoration = TextDecoration.Underline,
                        fontSize = 12.sp,
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }
        }
    }
}

@Composable
fun DiffOperationTag(op: FfiDiffOperation) {
    val (label, color) = when (op) {
        FfiDiffOperation.Insert -> "Einfügung" to Color(0xFF2E7D32)
        FfiDiffOperation.Delete -> "Löschung" to Color(0xFFC62828)
        FfiDiffOperation.Replace -> "Ersetzung" to Color(0xFFEF6C00)
        is FfiDiffOperation.Move -> "Verschoben" to Color(0xFF1565C0)
        FfiDiffOperation.Unchanged -> "Identisch" to Color.Gray
    }

    Surface(
        color = color.copy(alpha = 0.15f),
        shape = MaterialTheme.shapes.extraSmall
    ) {
        Text(
            text = label,
            color = color,
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp)
        )
    }
}

@Composable
fun DualViewportView(
    report: FfiComparisonReport,
    selectedChangeId: String?,
    syncScroll: Boolean,
    onToggleSync: () -> Unit,
    showVisualOverlay: Boolean,
    onToggleVisual: () -> Unit,
    coreBridge: DocumentCoreBridge? = null,
    sessionIdA: String? = null,
    sessionIdB: String? = null
) {
    var pageIndexA by remember { mutableStateOf(0u) }
    var pageIndexB by remember { mutableStateOf(0u) }
    var zoomScale by remember { mutableStateOf(1000u) }
    var bitmapA by remember { mutableStateOf<ImageBitmap?>(null) }
    var bitmapB by remember { mutableStateOf<ImageBitmap?>(null) }
    var visualDiffResult by remember { mutableStateOf<FfiPageDiffResult?>(null) }

    // Respond to change item selection: jump to aligned pages
    val selectedItem = remember(selectedChangeId) {
        report.changes.firstOrNull { it.id == selectedChangeId }
    }
    LaunchedEffect(selectedChangeId) {
        if (selectedItem != null) {
            if (selectedItem.pageA != null && selectedItem.pageB != null) {
                pageIndexA = selectedItem.pageA!!
                pageIndexB = selectedItem.pageB!!
            } else if (selectedItem.pageA != null) {
                pageIndexA = selectedItem.pageA!!
                if (syncScroll) {
                    val mapped = report.pageAlignments.firstOrNull { it.pageA == pageIndexA }?.pageB
                    if (mapped != null) pageIndexB = mapped
                }
            } else if (selectedItem.pageB != null) {
                pageIndexB = selectedItem.pageB!!
                if (syncScroll) {
                    val mapped = report.pageAlignments.firstOrNull { it.pageB == pageIndexB }?.pageA
                    if (mapped != null) pageIndexA = mapped
                }
            }
        }
    }

    // Asynchronously render pages into Bitmaps via MuPDF
    LaunchedEffect(pageIndexA, pageIndexB, zoomScale, sessionIdA, sessionIdB, coreBridge) {
        if (coreBridge != null && sessionIdA != null) {
            withContext(Dispatchers.IO) {
                runCatching {
                    val geomA = coreBridge.pageGeometry(sessionIdA, pageIndexA)
                    val reqA = FfiTileRenderRequest(
                        pageIndex = pageIndexA,
                        scaleMilli = zoomScale,
                        tileXPx = 0u,
                        tileYPx = 0u,
                        tileWidthPx = maxOf(1u, geomA.widthMilliPt / 1000u),
                        tileHeightPx = maxOf(1u, geomA.heightMilliPt / 1000u)
                    )
                    val resA = coreBridge.renderViewportWithPrefetch(sessionIdA, reqA)
                    val pixels = coreBridge.decodeRgba8ToArgbPixels(resA.visibleTile)
                    Bitmap.createBitmap(
                        pixels,
                        resA.visibleTile.widthPx.toInt(),
                        resA.visibleTile.heightPx.toInt(),
                        Bitmap.Config.ARGB_8888
                    ).asImageBitmap()
                }.onSuccess { bmp ->
                    bitmapA = bmp
                }
            }
        }

        if (coreBridge != null && sessionIdB != null) {
            withContext(Dispatchers.IO) {
                runCatching {
                    val geomB = coreBridge.pageGeometry(sessionIdB, pageIndexB)
                    val reqB = FfiTileRenderRequest(
                        pageIndex = pageIndexB,
                        scaleMilli = zoomScale,
                        tileXPx = 0u,
                        tileYPx = 0u,
                        tileWidthPx = maxOf(1u, geomB.widthMilliPt / 1000u),
                        tileHeightPx = maxOf(1u, geomB.heightMilliPt / 1000u)
                    )
                    val resB = coreBridge.renderViewportWithPrefetch(sessionIdB, reqB)
                    val pixels = coreBridge.decodeRgba8ToArgbPixels(resB.visibleTile)
                    Bitmap.createBitmap(
                        pixels,
                        resB.visibleTile.widthPx.toInt(),
                        resB.visibleTile.heightPx.toInt(),
                        Bitmap.Config.ARGB_8888
                    ).asImageBitmap()
                }.onSuccess { bmp ->
                    bitmapB = bmp
                }
            }
        }

        if (coreBridge != null && sessionIdA != null && sessionIdB != null && showVisualOverlay) {
            withContext(Dispatchers.IO) {
                runCatching {
                    val opts = FfiComparisonOptions(
                        granularity = FfiDiffGranularity.WORD,
                        ignoreWhitespace = true,
                        ignoreCase = false,
                        visualPixelThreshold = 16u,
                        visualClusterDistancePt = 10.0f,
                        alignClauses = true
                    )
                    coreBridge.comparePagesVisual(sessionIdA, pageIndexA, sessionIdB, pageIndexB, opts)
                }.onSuccess { diff ->
                    visualDiffResult = diff
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = syncScroll, onCheckedChange = { onToggleSync() })
                    Text("Synchron", fontSize = 12.sp)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = showVisualOverlay, onCheckedChange = { onToggleVisual() })
                    Text("Visueller Diff", fontSize = 12.sp)
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = { if (zoomScale > 500u) zoomScale -= 250u }) {
                    Text("−", fontSize = 16.sp, fontWeight = FontWeight.Bold)
                }
                Text("${zoomScale / 10u}%", fontSize = 12.sp)
                TextButton(onClick = { if (zoomScale < 3000u) zoomScale += 250u }) {
                    Text("+", fontSize = 16.sp, fontWeight = FontWeight.Bold)
                }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 8.dp)
        ) {
            // Pane A (Original)
            Card(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
            ) {
                Column(modifier = Modifier.padding(8.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = report.docATitle,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 13.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            TextButton(
                                onClick = {
                                    if (pageIndexA > 0u) {
                                        val targetA = pageIndexA - 1u
                                        pageIndexA = targetA
                                        if (syncScroll) {
                                            val mapped = report.pageAlignments.firstOrNull { it.pageA == targetA }?.pageB
                                            if (mapped != null) pageIndexB = mapped
                                        }
                                    }
                                },
                                enabled = pageIndexA > 0u
                            ) { Text("◀") }
                            Text("${pageIndexA + 1u}/${maxOf(1u, report.pageCountA)}", fontSize = 11.sp)
                            TextButton(
                                onClick = {
                                    if (pageIndexA + 1u < report.pageCountA) {
                                        val targetA = pageIndexA + 1u
                                        pageIndexA = targetA
                                        if (syncScroll) {
                                            val mapped = report.pageAlignments.firstOrNull { it.pageA == targetA }?.pageB
                                            if (mapped != null) pageIndexB = mapped
                                        }
                                    }
                                },
                                enabled = pageIndexA + 1u < report.pageCountA
                            ) { Text("▶") }
                        }
                    }
                    Spacer(modifier = Modifier.height(4.dp))
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(Color.White)
                    ) {
                        if (bitmapA != null) {
                            Canvas(modifier = Modifier.fillMaxSize()) {
                                drawImage(bitmapA!!)
                                for (change in report.changes) {
                                    if (change.pageA == pageIndexA && change.rectA != null) {
                                        val r = change.rectA!!
                                        val x = r.x0MilliPt.toFloat() / 1000f
                                        val y = r.y0MilliPt.toFloat() / 1000f
                                        val w = (r.x1MilliPt - r.x0MilliPt).toFloat() / 1000f
                                        val h = (r.y1MilliPt - r.y0MilliPt).toFloat() / 1000f
                                        val col = when (change.operation) {
                                            FfiDiffOperation.Delete -> Color(0x60C62828)
                                            FfiDiffOperation.Replace -> Color(0x60EF6C00)
                                            is FfiDiffOperation.Move -> Color(0x601565C0)
                                            else -> Color(0x604CAF50)
                                        }
                                        drawRect(color = col, topLeft = Offset(x, y), size = Size(w, h))
                                    }
                                }
                            }
                        } else {
                            Box(
                                modifier = Modifier.fillMaxSize().padding(16.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    "${report.docATitle}\nSeite ${pageIndexA + 1u} von ${report.pageCountA}",
                                    color = Color.DarkGray,
                                    textAlign = TextAlign.Center
                                )
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.width(8.dp))

            // Pane B (Modified)
            Card(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
            ) {
                Column(modifier = Modifier.padding(8.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = report.docBTitle,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 13.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            TextButton(
                                onClick = {
                                    if (pageIndexB > 0u) {
                                        val targetB = pageIndexB - 1u
                                        pageIndexB = targetB
                                        if (syncScroll) {
                                            val mapped = report.pageAlignments.firstOrNull { it.pageB == targetB }?.pageA
                                            if (mapped != null) pageIndexA = mapped
                                        }
                                    }
                                },
                                enabled = pageIndexB > 0u
                            ) { Text("◀") }
                            Text("${pageIndexB + 1u}/${maxOf(1u, report.pageCountB)}", fontSize = 11.sp)
                            TextButton(
                                onClick = {
                                    if (pageIndexB + 1u < report.pageCountB) {
                                        val targetB = pageIndexB + 1u
                                        pageIndexB = targetB
                                        if (syncScroll) {
                                            val mapped = report.pageAlignments.firstOrNull { it.pageB == targetB }?.pageA
                                            if (mapped != null) pageIndexA = mapped
                                        }
                                    }
                                },
                                enabled = pageIndexB + 1u < report.pageCountB
                            ) { Text("▶") }
                        }
                    }
                    Spacer(modifier = Modifier.height(4.dp))
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(Color.White)
                    ) {
                        if (bitmapB != null) {
                            Canvas(modifier = Modifier.fillMaxSize()) {
                                drawImage(bitmapB!!)
                                if (showVisualOverlay && visualDiffResult != null) {
                                    for (box in visualDiffResult!!.differenceBoundingBoxes) {
                                        val x = box.x0MilliPt.toFloat() / 1000f
                                        val y = box.y0MilliPt.toFloat() / 1000f
                                        val w = (box.x1MilliPt - box.x0MilliPt).toFloat() / 1000f
                                        val h = (box.y1MilliPt - box.y0MilliPt).toFloat() / 1000f
                                        drawRect(color = Color(0x50FF0000), topLeft = Offset(x, y), size = Size(w, h))
                                    }
                                }
                                for (change in report.changes) {
                                    if (change.pageB == pageIndexB && change.rectB != null) {
                                        val r = change.rectB!!
                                        val x = r.x0MilliPt.toFloat() / 1000f
                                        val y = r.y0MilliPt.toFloat() / 1000f
                                        val w = (r.x1MilliPt - r.x0MilliPt).toFloat() / 1000f
                                        val h = (r.y1MilliPt - r.y0MilliPt).toFloat() / 1000f
                                        val col = when (change.operation) {
                                            FfiDiffOperation.Insert -> Color(0x602E7D32)
                                            FfiDiffOperation.Replace -> Color(0x60EF6C00)
                                            is FfiDiffOperation.Move -> Color(0x601565C0)
                                            else -> Color(0x604CAF50)
                                        }
                                        drawRect(color = col, topLeft = Offset(x, y), size = Size(w, h))
                                    }
                                }
                            }
                        } else {
                            Box(
                                modifier = Modifier.fillMaxSize().padding(16.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    "${report.docBTitle}\nSeite ${pageIndexB + 1u} von ${report.pageCountB}",
                                    color = Color.DarkGray,
                                    textAlign = TextAlign.Center
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun ReportPreviewView(
    markdownReport: String,
    onExportReport: (format: String) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Vergleichsbericht", fontWeight = FontWeight.Bold, fontSize = 16.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { onExportReport("html") }) {
                    Text("HTML")
                }
                OutlinedButton(onClick = { onExportReport("md") }) {
                    Text("Markdown")
                }
                OutlinedButton(onClick = { onExportReport("json") }) {
                    Text("JSON")
                }
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        Surface(
            color = MaterialTheme.colorScheme.surfaceVariant,
            shape = MaterialTheme.shapes.small,
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
        ) {
            Text(
                text = markdownReport,
                fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                fontSize = 11.sp,
                modifier = Modifier.padding(12.dp)
            )
        }
    }
}
