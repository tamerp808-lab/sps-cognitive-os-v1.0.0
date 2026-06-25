package com.sps.companion.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Send
import androidx.compose.material.icons.filled.Flag
import androidx.compose.material.icons.filled.Psychology
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavController
import com.sps.companion.data.SpsBriefing
import com.sps.companion.data.SpsGoal
import com.sps.companion.data.SpsMemory
import com.sps.companion.ui.components.SpsBrainLogo
import com.sps.companion.ui.viewmodel.HomeViewModel
import kotlinx.coroutines.launch

/**
 * Home Screen — the companion dashboard.
 *
 * Layout:
 * - Top: greeting + brain logo + connection status.
 * - Middle: today's briefing, active goals, recent memories.
 * - Bottom: FAB to invoke voice command.
 *
 * Loads all data from the SPS kernel via [HomeViewModel].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(navController: NavController) {
    val vm: HomeViewModel = viewModel()
    val briefing by vm.briefing.collectAsState()
    val goals by vm.goals.collectAsState()
    val memories by vm.memories.collectAsState()
    val connectionState by vm.connectionState.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        SpsBrainLogo(modifier = Modifier.size(28.dp))
                        Spacer(Modifier.width(12.dp))
                        Text("SPS", style = MaterialTheme.typography.titleLarge)
                    }
                },
                actions = {
                    val status = if (connectionState is com.sps.companion.network.SpsConnectionState.Connected) {
                        Color(0xFF34D399)
                    } else {
                        Color(0xFFF87171)
                    }
                    Box(
                        Modifier
                            .size(10.dp)
                            .background(status, CircleShape)
                    )
                    Spacer(Modifier.width(8.dp))
                    IconButton(onClick = { navController.navigate("settings") }) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings")
                    }
                }
            )
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = { navController.navigate("voice") },
                containerColor = MaterialTheme.colorScheme.primary,
            ) {
                Icon(Icons.Default.Mic, contentDescription = "Talk")
            }
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item { Spacer(Modifier.height(8.dp)) }

            // Greeting + briefing.
            item { BriefingCard(briefing) }

            // Active goals.
            item {
                SectionHeader(
                    icon = Icons.Default.Flag,
                    title = "Active Goals",
                    action = "View all",
                    onAction = { navController.navigate("goals") },
                )
            }
            items(goals.take(3)) { goal -> GoalCard(goal) }

            // Recent memories.
            item {
                SectionHeader(
                    icon = Icons.Default.Psychology,
                    title = "Recent Memories",
                    action = "View all",
                    onAction = { navController.navigate("memory") },
                )
            }
            items(memories.take(3)) { mem -> MemoryCard(mem) }

            item { Spacer(Modifier.height(80.dp)) }
        }
    }
}

@Composable
private fun BriefingCard(briefing: SpsBriefing) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(20.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Column(Modifier.padding(20.dp)) {
            Text(
                briefing.greeting,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                briefing.summary.ifBlank { "Tap the mic below and say \"Hey SPS\" to begin." },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (briefing.topTasks.isNotEmpty()) {
                Spacer(Modifier.height(16.dp))
                Text("Today's Tasks", style = MaterialTheme.typography.titleSmall)
                briefing.topTasks.take(3).forEach { task ->
                    Row(Modifier.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            Modifier
                                .size(8.dp)
                                .background(MaterialTheme.colorScheme.tertiary, CircleShape)
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(task.title, style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }
        }
    }
}

@Composable
private fun SectionHeader(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    action: String? = null,
    onAction: () -> Unit = {},
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
            Text(title, style = MaterialTheme.typography.titleMedium)
        }
        if (action != null) {
            androidx.compose.material3.TextButton(onClick = onAction) {
                Text(action)
            }
        }
    }
}

@Composable
private fun GoalCard(goal: SpsGoal) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
    ) {
        Column(Modifier.padding(16.dp)) {
            Text(goal.title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            if (goal.description.isNotBlank()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    goal.description.take(120),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                )
            }
            Spacer(Modifier.height(12.dp))
            LinearProgressIndicator(
                progress = { goal.progress.toFloat() },
                modifier = Modifier.fillMaxWidth(),
                color = MaterialTheme.colorScheme.primary,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                "${(goal.progress * 100).toInt()}% • ${goal.milestones.count { it.completed }}/${goal.milestones.size} milestones",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun MemoryCard(memory: SpsMemory) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
    ) {
        Column(Modifier.padding(14.dp)) {
            Text(
                memory.content.take(140),
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 3,
            )
            Spacer(Modifier.height(6.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    memory.memoryType,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                )
                Spacer(Modifier.weight(1f))
                Text(
                    java.text.SimpleDateFormat("MMM d", java.util.Locale.getDefault())
                        .format(java.util.Date(memory.createdAt)),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
