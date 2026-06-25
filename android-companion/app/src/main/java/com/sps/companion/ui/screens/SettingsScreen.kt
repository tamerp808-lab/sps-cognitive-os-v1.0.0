package com.sps.companion.ui.screens

import android.content.Intent
import android.net.Uri
import android.provider.Settings as AndroidSettings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sps.companion.SpsApplication
import com.sps.companion.network.SpsConnectionState
import com.sps.companion.ui.viewmodel.SettingsViewModel
import kotlinx.coroutines.launch

/**
 * Settings Screen — connection, voice, companion, and about sections.
 *
 * Every toggle is wired to the [SpsConfigManager] so changes persist
 * and are observed by services in real-time.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onBack: () -> Unit) {
    val vm: SettingsViewModel = viewModel()
    val config by vm.config.collectAsState()
    val connection by vm.connectionState.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Spacer(Modifier.height(8.dp))

            // ============== Connection ==============
            SectionTitle("SPS Server")
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    var url by remember { mutableStateOf(config.serverUrl) }
                    OutlinedTextField(
                        value = url,
                        onValueChange = { url = it },
                        label = { Text("Server URL") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    Spacer(Modifier.height(8.dp))
                    val statusColor = when (connection) {
                        is SpsConnectionState.Connected -> MaterialTheme.colorScheme.primary
                        is SpsConnectionState.Failed -> MaterialTheme.colorScheme.error
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    }
                    val statusText = when (connection) {
                        is SpsConnectionState.Connected -> "● Connected"
                        is SpsConnectionState.Connecting -> "○ Connecting…"
                        is SpsConnectionState.Failed -> "● ${(connection as SpsConnectionState.Failed).message}"
                        is SpsConnectionState.Disconnected -> "○ Disconnected"
                    }
                    Text(statusText, color = statusColor, style = MaterialTheme.typography.bodyMedium)
                    Spacer(Modifier.height(8.dp))
                    androidx.compose.material3.Button(
                        onClick = {
                            scope.launch {
                                vm.setServerUrl(url)
                            }
                        }
                    ) { Text("Reconnect") }
                }
            }

            // ============== Voice ==============
            SectionTitle("Voice")
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    ToggleRow(
                        label = "Wake Word Detection",
                        description = "Always-listening \"Hey SPS\"",
                        checked = config.wakeWordEnabled,
                        onCheckedChange = { scope.launch { vm.setWakeWordEnabled(it) } },
                    )
                    HorizontalDivider()
                    ToggleRow(
                        label = "TTS Responses",
                        description = "Speak SPS responses aloud",
                        checked = config.ttsEnabled,
                        onCheckedChange = { scope.launch { vm.setTtsEnabled(it) } },
                    )
                    HorizontalDivider()
                    ToggleRow(
                        label = "Continuous Mode",
                        description = "Keep mic open after SPS responds",
                        checked = config.continuousMode,
                        onCheckedChange = { scope.launch { vm.setContinuousMode(it) } },
                    )
                    HorizontalDivider()
                    ToggleRow(
                        label = "Vibrate on Wake",
                        description = "Haptic feedback on wake word",
                        checked = config.vibrateOnWake,
                        onCheckedChange = { scope.launch { vm.setVibrateOnWake(it) } },
                    )
                }
            }

            // ============== Companion ==============
            SectionTitle("Companion")
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    ToggleRow(
                        label = "Floating Bubble",
                        description = "Show SPS bubble over other apps",
                        checked = config.overlayEnabled,
                        onCheckedChange = { scope.launch { vm.setOverlayEnabled(it) } },
                    )
                    HorizontalDivider()
                    ToggleRow(
                        label = "Start on Boot",
                        description = "Start SPS service when device boots",
                        checked = config.startOnBoot,
                        onCheckedChange = { scope.launch { vm.setStartOnBoot(it) } },
                    )
                    HorizontalDivider()
                    // Open system settings for accessibility + notification access.
                    ActionRow(
                        label = "Accessibility Service",
                        description = "Screen reading & automation",
                        actionText = "Open",
                        onAction = {
                            context.startActivity(Intent(AndroidSettings.ACTION_ACCESSIBILITY_SETTINGS))
                        },
                    )
                    HorizontalDivider()
                    ActionRow(
                        label = "Notification Access",
                        description = "Read & dismiss notifications",
                        actionText = "Open",
                        onAction = {
                            context.startActivity(Intent("android.settings.ACTION_NOTIFICATION_LISTENER_SETTINGS"))
                        },
                    )
                    HorizontalDivider()
                    ActionRow(
                        label = "Battery Optimization",
                        description = "Disable for SPS to stay alive",
                        actionText = "Open",
                        onAction = {
                            context.startActivity(Intent(AndroidSettings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
                        },
                    )
                }
            }

            // ============== About ==============
            SectionTitle("About")
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    InfoRow("App", "SPS Companion v1.0.0")
                    InfoRow("Kernel", connection.toString())
                    InfoRow("Architecture", "Event-sourced • Hash-chained • Local-first")
                    InfoRow("Permissions", "See Permissions screen")
                }
            }

            Spacer(Modifier.height(80.dp))
        }
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(text, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
}

@Composable
private fun ToggleRow(label: String, description: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun ActionRow(label: String, description: String, actionText: String, onAction: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        androidx.compose.material3.TextButton(onClick = onAction) { Text(actionText) }
    }
}

@Composable
private fun InfoRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodyMedium)
    }
}
