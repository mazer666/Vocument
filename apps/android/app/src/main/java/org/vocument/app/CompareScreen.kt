package org.vocument.app

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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
    onExportReport: (format: String) -> Unit = {}
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
                    onToggleVisual = { showVisualOverlay = !showVisualOverlay }
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
    onToggleVisual: () -> Unit
) {
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
            Text(
                text = "${report.pageCountA} S. ↔ ${report.pageCountB} S.",
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 8.dp)
        ) {
            // Pane A
            Card(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
            ) {
                Column(modifier = Modifier.padding(8.dp)) {
                    Text(
                        text = report.docATitle,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 13.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(Color.White)
                            .padding(8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("Dokument A (Original)\n${report.pageCountA} Seiten", color = Color.DarkGray)
                    }
                }
            }

            Spacer(modifier = Modifier.width(8.dp))

            // Pane B
            Card(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
            ) {
                Column(modifier = Modifier.padding(8.dp)) {
                    Text(
                        text = report.docBTitle,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 13.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(Color.White)
                            .padding(8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("Dokument B (Modifiziert)\n${report.pageCountB} Seiten", color = Color.DarkGray)
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
