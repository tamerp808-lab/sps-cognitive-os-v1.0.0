package com.sps.companion

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Chat
import androidx.compose.material.icons.filled.Flag
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Psychology
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.sps.companion.ui.screens.ChatScreen
import com.sps.companion.ui.screens.GoalsScreen
import com.sps.companion.ui.screens.HomeScreen
import com.sps.companion.ui.screens.MemoryScreen
import com.sps.companion.ui.screens.PermissionsScreen
import com.sps.companion.ui.screens.SettingsScreen
import com.sps.companion.ui.screens.VoiceScreen
import com.sps.companion.ui.theme.SpsTheme

/**
 * SPS Companion Main Activity — the Compose Navigation host.
 *
 * Routes:
 * - home    → dashboard (briefing, goals, memories)
 * - chat    → text chat with SPS (streaming)
 * - voice   → voice interaction (STT → SPS → TTS)
 * - goals   → long-term goals + milestones + tasks
 * - memory  → memory search & browse
 * - settings → connection, voice, companion settings
 * - permissions → first-run permission grants
 *
 * Bottom nav has 5 items: Home / Chat / Voice / Goals / Memory.
 * Settings + Permissions are pushed routes (no bottom-nav highlight).
 */
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Handle incoming intents (spps:// URL scheme, share, app shortcuts).
        val initialAction = intent?.getStringExtra("sps.action")

        setContent {
            SpsTheme {
                val navController = rememberNavController()
                val backStack by navController.currentBackStackEntryAsState()
                val currentRoute = backStack?.destination?.route

                Scaffold(
                    bottomBar = {
                        // Hide bottom bar on settings/permissions.
                        if (currentRoute in BottomNavItems.routes) {
                            NavigationBar {
                                BottomNavItems.items.forEach { item ->
                                    val selected = backStack?.destination?.hierarchy?.any { it.route == item.route } == true
                                    NavigationBarItem(
                                        selected = selected,
                                        onClick = {
                                            navController.navigate(item.route) {
                                                popUpTo(navController.graph.findStartDestination().id) { saveState = true }
                                                launchSingleTop = true
                                                restoreState = true
                                            }
                                        },
                                        icon = { Icon(item.icon, contentDescription = item.label) },
                                        label = { Text(item.label) },
                                    )
                                }
                            }
                        }
                    }
                ) { padding ->
                    NavHost(
                        navController = navController,
                        startDestination = "home",
                        modifier = Modifier.padding(padding),
                    ) {
                        composable("home") { HomeScreen(navController) }
                        composable("chat") { ChatScreen(onBack = { navController.popBackStack() }) }
                        composable("voice") { VoiceScreen(onBack = { navController.popBackStack() }) }
                        composable("goals") { GoalsScreen(onBack = { navController.popBackStack() }) }
                        composable("memory") { MemoryScreen(onBack = { navController.popBackStack() }) }
                        composable("settings") { SettingsScreen(onBack = { navController.popBackStack() }) }
                        composable("permissions") { PermissionsScreen(onBack = { navController.popBackStack() }) }
                    }
                }

                // Handle sps.action extras (from shortcuts / QS tile / widget).
                androidx.compose.runtime.LaunchedEffect(initialAction) {
                    when (initialAction) {
                        "talk" -> navController.navigate("voice")
                        "goals" -> navController.navigate("goals")
                        "memory" -> navController.navigate("memory")
                    }
                }
            }
        }
    }
}

/** Bottom navigation items. */
private data class NavItem(
    val route: String,
    val label: String,
    val icon: androidx.compose.ui.graphics.vector.ImageVector,
)

private object BottomNavItems {
    val items = listOf(
        NavItem("home", "Home", Icons.Default.Home),
        NavItem("chat", "Chat", Icons.Default.Chat),
        NavItem("voice", "Voice", Icons.Default.Mic),
        NavItem("goals", "Goals", Icons.Default.Flag),
        NavItem("memory", "Memory", Icons.Default.Psychology),
    )
    val routes = items.map { it.route }.toSet()
}
