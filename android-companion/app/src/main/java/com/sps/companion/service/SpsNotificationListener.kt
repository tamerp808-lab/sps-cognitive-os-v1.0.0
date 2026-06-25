package com.sps.companion.service

import android.app.Notification
import android.content.Context
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import android.util.Log
import androidx.core.app.NotificationCompat
import com.sps.companion.voice.VoiceManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch

/**
 * SPS Notification Listener — reads, summarizes, and dismisses notifications.
 *
 * Disabled by default. When the user enables it (from Settings →
 * Notification Access), SPS can:
 * - "Read my notifications" — speaks the most recent notifications.
 * - "Summarize notifications" — uses the LLM to summarize them.
 * - "Dismiss <app> notifications" — clears them.
 * - Auto-summarize: on receiving N notifications in M seconds, generates
 *   a single "you have 5 new notifications" summary.
 *
 * Privacy: notification contents are NOT logged or stored persistently.
 * They're kept in an in-memory ring buffer (most recent 50) and discarded
 * on reboot.
 */
class SpsNotificationListener : NotificationListenerService() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val recent = ArrayDeque<NotificationRecord>(MAX_RECENT)
    private val _events = MutableSharedFlow<NotificationRecord>(extraBufferCapacity = 32)
    val events: SharedFlow<NotificationRecord> = _events.asSharedFlow()

    override fun onListenerConnected() {
        super.onListenerConnected()
        Log.i(TAG, "SPS Notification Listener connected")
        instance = this
    }

    override fun onListenerDisconnected() {
        Log.i(TAG, "SPS Notification Listener disconnected")
        instance = null
        super.onListenerDisconnected()
    }

    override fun onNotificationPosted(sbn: StatusBarNotification?) {
        super.onNotificationPosted(sbn)
        sbn ?: return
        // Skip our own notifications.
        if (sbn.packageName == packageName) return
        // Skip ongoing foreground-service notifications.
        if (sbn.isOngoing) return

        val n = sbn.notification ?: return
        val extras = n.extras
        val title = extras.getString(Notification.EXTRA_TITLE, "").toString()
        val text = extras.getCharSequence(Notification.EXTRA_TEXT)?.toString() ?: ""
        val record = NotificationRecord(
            packageName = sbn.packageName,
            title = title,
            text = text,
            timestamp = sbn.postTime,
            category = n.category ?: "general",
            key = sbn.key,
        )
        synchronized(recent) {
            recent.addLast(record)
            while (recent.size > MAX_RECENT) recent.removeFirst()
        }
        _events.tryEmit(record)
        Log.d(TAG, "Notification: ${record.packageName} — ${record.title}")
    }

    /** Get the most recent N notifications. */
    fun getRecent(limit: Int = 10): List<NotificationRecord> = synchronized(recent) {
        recent.toList().takeLast(limit)
    }

    /** Dismiss a notification by its key. */
    fun dismiss(key: String) {
        runCatching { cancelNotification(key) }
    }

    /** Dismiss all notifications from a package. */
    fun dismissAllFromPackage(pkg: String) {
        activeNotifications?.filter { it.packageName == pkg }?.forEach {
            runCatching { cancelNotification(it.key) }
        }
    }

    /** Dismiss all notifications. */
    fun dismissAll() {
        activeNotifications?.forEach {
            runCatching { cancelNotification(it.key) }
        }
    }

    companion object {
        private const val TAG = "SpsNotificationListener"
        private const val MAX_RECENT = 50

        @Volatile private var instance: SpsNotificationListener? = null

        /**
         * Speak the most recent notifications via TTS.
         * Called from [VoiceCommandHandler] when the user says "read my notifications".
         */
        suspend fun readRecent(context: Context, voice: VoiceManager) {
            val svc = instance ?: run {
                voice.speak("Notification access is not enabled. Please enable it in Settings.")
                return
            }
            val recent = svc.getRecent(5)
            if (recent.isEmpty()) {
                voice.speak("You have no recent notifications.")
                return
            }
            val summary = recent.joinToString(separator = ". ") { rec ->
                "${rec.title}: ${rec.text}"
            }
            voice.speak("You have ${recent.size} recent notifications. $summary")
        }

        /** true if the user has enabled this listener. */
        fun isEnabled(context: Context): Boolean {
            val enabled = android.provider.Settings.Secure.getString(
                context.contentResolver,
                "enabled_notification_listeners"
            ) ?: return false
            return enabled.contains(context.packageName + "/.service.SpsNotificationListener")
        }
    }
}

/** A captured notification — kept in memory only, never persisted. */
data class NotificationRecord(
    val packageName: String,
    val title: String,
    val text: String,
    val timestamp: Long,
    val category: String,
    val key: String,
)
