package com.sps.companion.service

import android.content.Context
import android.util.Log
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsCompletionRequest
import com.sps.companion.voice.VoiceManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * Voice Command Handler — the wake-word → action pipeline.
 *
 * When the wake word fires:
 * 1. Speak a short acknowledgment ("Yes?" or a chime).
 * 2. Listen for the user's command via [VoiceManager.listenOnce].
 * 3. Send the text to the SPS kernel via [SpsClient.complete].
 * 4. Speak the response via TTS.
 * 5. If continuous mode is on, loop back to step 2.
 *
 * Special commands (handled locally, not sent to LLM):
 * - "stop" / "cancel" — abort the conversation.
 * - "open <app>" — launch an app via device control.
 * - "what time is it" / "what's the date" — answer locally.
 * - "read my notifications" — trigger the notification reader.
 */
object VoiceCommandHandler {

    private const val TAG = "VoiceCommandHandler"
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** Called by [SpsForegroundService] when the wake word is detected. */
    fun handleWakeWord(context: Context) {
        scope.launch {
            val voice = VoiceManager(context)
            try {
                val config = SpsApplication.config().config.first()
                // Brief acknowledgment.
                if (config.ttsEnabled) {
                    voice.speak("Yes?")
                }

                // Listen for the command (up to ~10s).
                val command = voice.listenOnce()
                Log.i(TAG, "Heard: $command")

                if (command.isBlank()) {
                    Log.i(TAG, "Empty command — ignoring")
                    return@launch
                }

                // Check for local commands first.
                if (handleLocalCommand(context, voice, command, config.ttsEnabled)) {
                    return@launch
                }

                // Send to SPS server.
                val response = SpsApplication.client().complete(
                    SpsCompletionRequest(
                        prompt = command,
                        system = "You are SPS, a personal cognitive AI companion. Respond concisely (1-3 sentences) since the response will be spoken aloud. If the user asks to do something on the device, explain what you would do.",
                    )
                )

                Log.i(TAG, "SPS response: ${response.text.take(200)}")

                // Speak the response.
                if (config.ttsEnabled && response.text.isNotBlank()) {
                    voice.speak(response.text)
                }

                // Store as a memory.
                runCatching {
                    SpsApplication.client().storeMemory(
                        content = "User said: $command\nSPS replied: ${response.text.take(500)}",
                        type = "episodic",
                    )
                }

            } catch (e: Exception) {
                Log.e(TAG, "Voice command failed: ${e.message}", e)
                voice.speak("Sorry, I had a problem with that.")
            } finally {
                voice.release()
            }
        }
    }

    /**
     * Handle commands that don't need the LLM. Returns true if handled.
     */
    private suspend fun handleLocalCommand(
        context: Context,
        voice: VoiceManager,
        command: String,
        ttsEnabled: Boolean,
    ): Boolean {
        val lower = command.lowercase().trim()

        // Stop / cancel.
        if (lower in listOf("stop", "cancel", "nevermind", "never mind")) {
            voice.stopSpeaking()
            return true
        }

        // Time / date.
        if (lower.contains("what time") || lower == "time") {
            val now = java.text.SimpleDateFormat("h:mm a", java.util.Locale.getDefault())
                .format(java.util.Date())
            if (ttsEnabled) voice.speak("It's $now")
            return true
        }
        if (lower.contains("what date") || lower.contains("what day") || lower == "date") {
            val now = java.text.SimpleDateFormat("EEEE, MMMM d", java.util.Locale.getDefault())
                .format(java.util.Date())
            if (ttsEnabled) voice.speak("Today is $now")
            return true
        }

        // Open app — "open whatsapp", "launch spotify".
        if (lower.startsWith("open ") || lower.startsWith("launch ")) {
            val appName = lower.substringAfter("open ").substringAfter("launch ").trim()
            if (appName.isNotEmpty()) {
                runCatching {
                    val apps = SpsApplication.client().listApps()
                    val match = apps.firstOrNull {
                        it.label.lowercase().contains(appName) ||
                        it.packageName.contains(appName, ignoreCase = true)
                    }
                    if (match != null) {
                        SpsApplication.client().launchApp(match.packageName)
                        if (ttsEnabled) voice.speak("Opening ${match.label}")
                    } else {
                        if (ttsEnabled) voice.speak("I couldn't find an app called $appName")
                    }
                }
                return true
            }
        }

        // Read notifications — "read my notifications".
        if (lower.contains("notification")) {
            // Trigger the notification reader service.
            com.sps.companion.service.SpsNotificationListener.readRecent(context, voice)
            return true
        }

        return false
    }
}
