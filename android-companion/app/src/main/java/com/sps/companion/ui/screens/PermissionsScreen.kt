package com.sps.companion.ui.screens

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings as AndroidSettings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/**
 * Permissions Screen — first-run onboarding.
 *
 * Walks the user through granting:
 * 1. Microphone (required for wake word + voice commands)
 * 2. Notifications (Android 13+, required for foreground service)
 * 3. Overlay (optional, for floating bubble)
 * 4. Battery optimization (optional, for always-on wake word)
 * 5. Accessibility (optional, for screen reading & automation)
 * 6. Notification access (optional, for notification reading)
 *
 * Each permission shows a status chip (granted/denied) and a button to
 * open the system settings if denied.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PermissionsScreen(onBack: () -> Unit) {
    val context = LocalContext.current

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Permissions") },
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
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Spacer(Modifier.height(8.dp))

            Text(
                "Grant permissions for SPS to be a true companion",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            // 1. Microphone
            var micGranted by remember {
                mutableStateOf(
                    context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
                        PackageManager.PERMISSION_GRANTED
                )
            }
            val micLauncher = rememberLauncherForActivityResult(
                ActivityResultContracts.RequestPermission()
            ) { granted -> micGranted = granted }

            PermissionCard(
                title = "Microphone",
                description = "Required for wake word detection and voice commands",
                granted = micGranted,
                actionText = if (micGranted) "Granted" else "Grant",
                onAction = { micLauncher.launch(Manifest.permission.RECORD_AUDIO) },
                required = true,
            )

            // 2. Notifications (Android 13+)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                var notifGranted by remember {
                    mutableStateOf(
                        context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
                            PackageManager.PERMISSION_GRANTED
                    )
                }
                val notifLauncher = rememberLauncherForActivityResult(
                    ActivityResultContracts.RequestPermission()
                ) { granted -> notifGranted = granted }

                PermissionCard(
                    title = "Notifications",
                    description = "Required for the persistent SPS notification (Android 13+)",
                    granted = notifGranted,
                    actionText = if (notifGranted) "Granted" else "Grant",
                    onAction = { notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS) },
                    required = true,
                )
            }

            // 3. Overlay
            val canDrawOverlays = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                AndroidSettings.canDrawOverlays(context)
            } else true
            PermissionCard(
                title = "Display Over Other Apps",
                description = "Show the floating SPS bubble over any app",
                granted = canDrawOverlays,
                actionText = if (canDrawOverlays) "Granted" else "Open Settings",
                onAction = {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                        val intent = Intent(
                            AndroidSettings.ACTION_MANAGE_OVERLAY_PERMISSION,
                            Uri.parse("package:${context.packageName}")
                        )
                        context.startActivity(intent)
                    }
                },
                required = false,
            )

            // 4. Battery optimization
            PermissionCard(
                title = "Disable Battery Optimization",
                description = "Keeps SPS alive in the background for always-on wake word",
                granted = false, // Always show as actionable
                actionText = "Open Settings",
                onAction = {
                    context.startActivity(Intent(AndroidSettings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
                },
                required = false,
            )

            // 5. Accessibility
            PermissionCard(
                title = "Accessibility Service",
                description = "Screen reading & automation (\"tap the send button\")",
                granted = com.sps.companion.service.SpsAccessibilityService.isEnabled(context),
                actionText = "Open Settings",
                onAction = {
                    context.startActivity(Intent(AndroidSettings.ACTION_ACCESSIBILITY_SETTINGS))
                },
                required = false,
            )

            // 6. Notification access
            PermissionCard(
                title = "Notification Access",
                description = "Read & dismiss notifications (\"read my notifications\")",
                granted = com.sps.companion.service.SpsNotificationListener.isEnabled(context),
                actionText = "Open Settings",
                onAction = {
                    context.startActivity(Intent("android.settings.ACTION_NOTIFICATION_LISTENER_SETTINGS"))
                },
                required = false,
            )

            Spacer(Modifier.height(80.dp))
        }
    }
}

@Composable
private fun PermissionCard(
    title: String,
    description: String,
    granted: Boolean,
    actionText: String,
    onAction: () -> Unit,
    required: Boolean,
) {
    Card(Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
                    if (required) {
                        Spacer(Modifier.height(0.dp))
                        Text(
                            "  • required",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                }
                Spacer(Modifier.height(4.dp))
                Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Spacer(Modifier.height(0.dp))
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(
                    if (granted) Icons.Default.Check else Icons.Default.Close,
                    contentDescription = null,
                    tint = if (granted) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(4.dp))
                if (!granted) {
                    androidx.compose.material3.TextButton(onClick = onAction) { Text(actionText) }
                }
            }
        }
    }
}
