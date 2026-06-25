package com.sps.companion.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sps.companion.ui.components.SpsBrainLogo
import com.sps.companion.ui.viewmodel.VoiceViewModel
import com.sps.companion.voice.VoiceState

/**
 * Voice Screen — the primary voice interaction surface.
 *
 * Layout:
 * - Animated SPS brain logo (faster when listening).
 * - Voice state text ("Listening…", "Processing…", "Speaking…").
 * - Partial transcript (what the user is saying, in real-time).
 * - SPS response text.
 * - Big mic button — tap to start/stop.
 *
 * When the user taps the mic, we run the full pipeline:
 * STT → SPS server → TTS.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VoiceScreen(onBack: () -> Unit) {
    val vm: VoiceViewModel = viewModel()
    val state by vm.state.collectAsState()
    val partial by vm.partialTranscript.collectAsState()
    val response by vm.response.collectAsState()
    val context = LocalContext.current

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Voice") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            // Brain logo — animated based on state.
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                SpsBrainLogo(
                    modifier = Modifier.padding(top = 32.dp, bottom = 24.dp),
                    active = state is VoiceState.Listening || state is VoiceState.Speaking,
                    size = 180,
                )
                val statusText = when (state) {
                    is VoiceState.Idle -> "Tap the mic and speak"
                    is VoiceState.Listening -> "Listening…"
                    is VoiceState.Processing -> "Thinking…"
                    is VoiceState.Speaking -> "Speaking…"
                    is VoiceState.Error -> "Error: ${(state as VoiceState.Error).message}"
                }
                Text(
                    statusText,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }

            // Transcript + response.
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .padding(vertical = 24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                AnimatedVisibility(visible = partial.isNotBlank(), enter = fadeIn(), exit = fadeOut()) {
                    Text(
                        partial,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
                Spacer(Modifier.height(16.dp))
                AnimatedVisibility(visible = response.isNotBlank(), enter = fadeIn(), exit = fadeOut()) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(
                                color = MaterialTheme.colorScheme.surfaceVariant,
                                shape = RoundedCornerShape(16.dp),
                            )
                            .padding(16.dp),
                    ) {
                        Text(
                            response,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurface,
                            fontWeight = FontWeight.Medium,
                        )
                    }
                }
            }

            // Mic button.
            Button(
                onClick = {
                    if (state is VoiceState.Listening) {
                        vm.stopListening()
                    } else {
                        vm.startListening(context)
                    }
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(72.dp),
                shape = RoundedCornerShape(36.dp),
            ) {
                Icon(
                    if (state is VoiceState.Listening) Icons.Default.Stop else Icons.Default.Mic,
                    contentDescription = null,
                )
                Spacer(Modifier.height(0.dp))
                Text(
                    if (state is VoiceState.Listening) "Stop" else "Tap to Speak",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(start = 8.dp),
                )
            }
        }
    }
}
