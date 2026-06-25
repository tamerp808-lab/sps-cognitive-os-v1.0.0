package com.sps.companion.service

import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import com.sps.companion.MainActivity
import com.sps.companion.R
import com.sps.companion.SpsApplication
import com.sps.companion.network.SpsConnectionState
import com.sps.companion.voice.WakeWordDetector
import com.sps.companion.voice.WakeWordEvent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * SPS Foreground Service — the persistent background companion.
 *
 * This is the single most important component of the Android Companion.
 * It's what makes "Hey SPS" work from any app, any time, even when the
 * SPS app is closed.
 *
 * Responsibilities:
 * 1. Show a persistent low-importance notification (so Android doesn't kill us).
 * 2. Hold a wake lock during voice activity.
 * 3. Run the [WakeWordDetector] (always-listening).
 * 4. Maintain the WebSocket connection to the SPS server.
 * 5. On wake-word detection, hand off to [VoiceCommandHandler].
 *
 * The service is `START_STICKY` — Android will restart it if killed.
 * On boot, [com.sps.companion.receiver.BootReceiver] starts it.
 *
 * Foreground service types:
 * - microphone: needed for wake-word listening.
 * - specialUse: companion AI use case (declared in manifest).
 */
class SpsForegroundService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var wakeWordDetector: WakeWordDetector? = null
    private var connectionMonitor: Job? = null
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "SPS Foreground Service created")

        // Acquire a partial wake lock — keeps CPU alive during wake-word
        // detection. The lock is held only while listening (not 24/7).
        val pm = getSystemService(POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "sps:wakeword")
        wakeLock?.setReferenceCounted(false)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.i(TAG, "SPS Foreground Service starting")

        // Start as foreground immediately (Android 12+ requires this within 5s).
        startForeground(NOTIFICATION_ID, buildNotification(), foregroundServiceType())

        // Start the wake-word detector + connection monitor.
        scope.launch { startWakeWordLoop() }
        scope.launch { startConnectionMonitor() }

        // START_STICKY — restart if killed.
        return START_STICKY
    }

    /**
     * Start listening for the wake word. Disabled if the user turned it
     * off in Settings. Re-checks the config every 30 seconds.
     */
    private suspend fun startWakeWordLoop() {
        val config = SpsApplication.config()
        while (true) {
            val enabled = config.config.value.wakeWordEnabled
            if (enabled && wakeWordDetector == null) {
                Log.i(TAG, "Enabling wake-word detection")
                wakeWordDetector = WakeWordDetector(
                    context = this,
                    sensitivity = config.config.value.wakeWordSensitivity,
                ).also { detector ->
                    detector.start()
                    scope.launch {
                        detector.events.collectLatest { event ->
                            when (event) {
                                is WakeWordEvent.Detected -> handleWakeWord(event.confidence)
                            }
                        }
                    }
                }
            } else if (!enabled && wakeWordDetector != null) {
                Log.i(TAG, "Disabling wake-word detection")
                wakeWordDetector?.release()
                wakeWordDetector = null
            }
            delay(30_000) // Re-check config every 30s.
        }
    }

    /** Called when the wake word is detected. Triggers voice command flow. */
    private suspend fun handleWakeWord(confidence: Float) {
        Log.i(TAG, "Wake word detected (confidence=$confidence)")
        // Vibrate.
        val vibrator = getSystemService(VIBRATOR_SERVICE) as android.os.Vibrator
        if (SpsApplication.config().config.value.vibrateOnWake) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                vibrator.vibrate(android.os.VibrationEffect.createOneShot(80, 120))
            } else {
                @Suppress("DEPRECATION")
                vibrator.vibrate(80)
            }
        }

        // Update notification to "listening".
        updateNotification(getString(R.string.notification_voice_active))

        // Hand off to the voice command handler (in a separate process? no — same service).
        VoiceCommandHandler.handleWakeWord(this)

        // After processing, restore the persistent notification.
        delay(2000)
        updateNotification(getString(R.string.notification_text))
    }

    /**
     * Maintain the SPS server connection. Reconnects on failure with
     * exponential backoff.
     */
    private suspend fun startConnectionMonitor() {
        val client = SpsApplication.client()
        var backoffMs = 1000L
        while (true) {
            try {
                client.healthCheck()
                if (client.connectionState.value is SpsConnectionState.Connected) {
                    backoffMs = 1000L
                    // Wait for a disconnect.
                    client.connectionState.first { it !is SpsConnectionState.Connected }
                }
            } catch (e: Exception) {
                Log.w(TAG, "Connection failed: ${e.message}")
            }
            delay(backoffMs)
            backoffMs = (backoffMs * 2).coerceAtMost(60_000)
        }
    }

    private fun buildNotification(textOverride: String? = null): Notification {
        val mainIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val pi = PendingIntent.getActivity(
            this, 0, mainIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return NotificationCompat.Builder(this, SpsApplication.CHANNEL_PERSISTENT)
            .setContentTitle(getString(R.string.notification_title))
            .setContentText(textOverride ?: getString(R.string.notification_text))
            .setSmallIcon(R.drawable.ic_sps)
            .setContentIntent(pi)
            .setOngoing(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
    }

    private fun updateNotification(text: String) {
        val nm = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        nm.notify(NOTIFICATION_ID, buildNotification(text))
    }

    private fun foregroundServiceType(): Int {
        // Android 14+ requires explicit FGS type.
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE or
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
        } else {
            0
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        Log.i(TAG, "SPS Foreground Service destroyed")
        wakeWordDetector?.release()
        wakeWordDetector = null
        wakeLock?.release()
        scope.cancel()
        super.onDestroy()
    }

    companion object {
        private const val TAG = "SpsForegroundService"
        const val NOTIFICATION_ID = 5470

        /** Start the service. */
        fun start(context: Context) {
            val intent = Intent(context, SpsForegroundService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /** Stop the service. */
        fun stop(context: Context) {
            context.stopService(Intent(context, SpsForegroundService::class.java))
        }
    }
}
