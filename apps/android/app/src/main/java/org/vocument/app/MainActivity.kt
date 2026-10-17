package org.vocument.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.vocument_ffi.DocumentCoreHandshake

/**
 * Native Android 13+ Activity using Jetpack Compose (`MOBILE-001`, `MOBILE-002`, `VIEW-001` Minimal Reader preset).
 * Queries the shared Rust `DocumentCore` over UniFFI on startup.
 */
class MainActivity : ComponentActivity() {
    private val coreBridge = DocumentCoreBridge()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                val handshake = remember { coreBridge.performHandshake() }
                MinimalReaderBootstrapScreen(handshake = handshake)
            }
        }
    }
}

@Composable
fun MinimalReaderBootstrapScreen(handshake: DocumentCoreHandshake) {
    Scaffold(
        bottomBar = {
            // MOBILE-002: Small bottom bar around Pages / Search / Tools
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
            Spacer(modifier = Modifier.height(16.dp))
            Text("Offline-First Policy: ${if (handshake.strictOfflineDefault) "Active (deny_all)" else "Unexpected"}")
            Text("Telemetry: ${if (handshake.telemetryEnabled) "Enabled" else "Disabled (LOG-001)"}")
            Text("Target Render Rate: ${handshake.targetFps} FPS")
            Text("Default German Variant: ${handshake.defaultGermanVariant}")
            Text("Built-in OCR Languages: ${handshake.builtInOcrLanguages.joinToString(", ")}")
        }
    }
}
