package org.vocument.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.Test
import uniffi.vocument_ffi.FfiDiffOperation

/**
 * Android test verifying native Document Comparison & Diffing operations (`M3.7`, `COMPARE-001`–`COMPARE-004`).
 */
class CompareBridgeTest {
    companion object {
        @JvmStatic
        @BeforeClass
        fun setUpNativeLibrary() {
            val candidatePaths = listOf(
                "../../target/debug",
                "../../../target/debug",
                "target/debug"
            )
            val existing = candidatePaths.firstOrNull { File(it).exists() }
            if (existing != null) {
                val absolute = File(existing).absolutePath
                val current = System.getProperty("jna.library.path")
                System.setProperty(
                    "jna.library.path",
                    if (current.isNullOrBlank()) absolute else "$current:$absolute"
                )
            }
        }
    }

    private val bridge = DocumentCoreBridge()

    @Test
    fun testCompareIdenticalTextStreamsProducesZeroChanges() {
        val text = "§ 1 Geltungsbereich\nDiese Vereinbarung gilt für alle Mitarbeiter."
        val report = bridge.compareTextStreams("Doc A", text, "Doc B", text)

        assertEquals("Doc A", report.docATitle)
        assertEquals("Doc B", report.docBTitle)
        assertEquals(0u, report.summary.totalChanges)
        assertEquals(0u, report.summary.insertionsCount)
        assertEquals(0u, report.summary.deletionsCount)
        assertEquals(0u, report.summary.replacementsCount)
        assertEquals(0u, report.summary.movesCount)
        assertTrue(report.summary.similarityRatio >= 0.999f)
        assertTrue(report.changes.isEmpty())
        assertFalse(report.markdownReport.isBlank())
        assertFalse(report.htmlReport.isBlank())
        assertFalse(report.jsonReport.isBlank())
    }

    @Test
    fun testCompareModifiedTextStreamsDetectsInsertionsAndDeletionsWithClauses() {
        val textA = """
            § 1 Präambel
            Die Parteien vereinbaren eine vertrauensvolle Zusammenarbeit.
            § 2 Vergütung
            Das Honorar beträgt 5.000 Euro monatlich.
        """.trimIndent()

        val textB = """
            § 1 Präambel
            Die Vertragsparteien vereinbaren eine verbindliche Zusammenarbeit.
            § 2 Vergütung
            Das Honorar beträgt 6.500 Euro monatlich.
            § 3 Geheimhaltung
            Alle Informationen sind vertraulich zu behandeln.
        """.trimIndent()

        val report = bridge.compareTextStreams("Vertrag v1", textA, "Vertrag v2", textB)

        assertTrue("Expected changes > 0", report.summary.totalChanges > 0u)
        assertTrue("Expected insertions", report.summary.insertionsCount >= 1u)

        // Verify clause detection
        val hasClause1 = report.changes.any { it.clauseHint?.contains("§ 1") == true }
        val hasClause2 = report.changes.any { it.clauseHint?.contains("§ 2") == true }
        assertTrue("Expected § 1 or § 2 clause context in diff", hasClause1 || hasClause2)

        // Verify report outputs
        assertTrue("Markdown report should contain comparison summary", report.markdownReport.contains("Comparison Report"))
        assertTrue("HTML report should be self-contained HTML document", report.htmlReport.contains("<!DOCTYPE html>"))
        assertTrue("JSON report should contain valid JSON structure", report.jsonReport.contains("\"total_changes\""))
    }
}
