package com.sps.companion

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.ComponentName
import android.content.Context
import android.content.pm.ServiceInfo
import android.os.Build
import android.util.Log
import com.sps.companion.data.SpsConfig
import com.sps.companion.data.SpsConfigManager
import com.sps.companion.network.SpsClient
import com.sps.companion.network.SpsConnectionState
import com.sps.companion.service.SpsForegroundService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * SPS Application — the single composition root for the entire companion.
 *
 * Owns:
 * - [spsClient]: the persistent HTTP + WebSocket connection to the local SPS kernel.
 * - [configManager]: DataStore-backed settings (server URL, wake word, voice prefs).
 * - [connectionState]: a hot stream of connection status for the UI.
 *
 * On startup it:
 * 1. Creates the three notification channels (persistent / voice / overlay).
 * 2. Loads the saved config and connects to the SPS server.
 * 3. Starts the foreground service (which owns wake-word detection).
 */
class SpsApplication : Application() {

    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** Persistent SPS server client (HTTP + WebSocket). */
    lateinit var spsClient: SpsClient
        private set

    /** DataStore-backed configuration. */
    lateinit var configManager: SpsConfigManager
        private set

    private val _connectionState = MutableStateFlow<SpsConnectionState>(SpsConnectionState.Disconnected)
    /** Hot stream of SPS server connection state — observed by UI + services. */
    val connectionState: StateFlow<SpsConnectionState> = _connectionState.asStateFlow()

    override fun onCreate() {
        super.onCreate()
        instance = this

        Log.i(TAG, "SPS Companion starting")

        // 1. Config (DataStore-backed, async-loaded).
        configManager = SpsConfigManager(this)
        appScope.launch {
            val config = configManager.config.value
            // 2. SPS client.
            spsClient = SpsClient(config.serverUrl)
            spsClient.connectionState.collect { state ->
                _connectionState.value = state
            }
        }

        // 3. Notification channels.
        createNotificationChannels()

        // 4. Start the foreground service. This is what keeps SPS alive
        //    when the user closes the app — it owns the persistent
        //    notification and the wake-word loop.
        SpsForegroundService.start(this)
    }

    /** Reconnect the SPS client (called after the server URL changes). */
    fun reconnect(newUrl: String) {
        if (::spsClient.isInitialized) {
            spsClient.close()
        }
        appScope.launch {
            spsClient = SpsClient(newUrl)
            spsClient.connectionState.collect { state ->
                _connectionState.value = state
            }
        }
    }

    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val nm = getSystemService(NotificationManager::class.java) ?: return

        // Persistent — low importance, can't be dismissed, no sound.
        nm.createNotificationChannel(
            NotificationChannel(
                CHANNEL_PERSISTENT,
                getString(R.string.notification_channel_persistent),
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = getString(R.string.notification_channel_persistent_desc)
                setShowBadge(false)
            }
        )

        // Voice — high importance, makes a sound.
        nm.createNotificationChannel(
            NotificationChannel(
                CHANNEL_VOICE,
                getString(R.string.notification_channel_voice),
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = getString(R.string.notification_channel_voice_desc)
            }
        )

        // Overlay — min importance, no sound.
        nm.createNotificationChannel(
            NotificationChannel(
                CHANNEL_OVERLAY,
                getString(R.string.notification_channel_overlay),
                NotificationManager.IMPORTANCE_MIN
            ).apply {
                description = getString(R.string.notification_channel_overlay_desc)
                setShowBadge(false)
            }
        )
    }

    companion object {
        private const val TAG = "SpsApplication"

        const val CHANNEL_PERSISTENT = "sps.persistent"
        const val CHANNEL_VOICE = "sps.voice"
        const val CHANNEL_OVERLAY = "sps.overlay"

        @Volatile private var instance: SpsApplication? = null
        fun get(): SpsApplication = instance ?: error("SPS Application not yet created")

        /** Convenience accessor for the SPS client. */
        fun client(): SpsClient = get().spsClient

        /** Convenience accessor for the config. */
        fun config(): SpsConfigManager = get().configManager
    }
}
