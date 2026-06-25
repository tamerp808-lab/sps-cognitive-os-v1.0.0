package com.sps.companion.voice

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.speech.tts.TextToSpeech
import android.util.Log
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.suspendCancellableCoroutine
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume

/**
 * Voice Manager — wraps Android's [SpeechRecognizer] (STT) and
 * [TextToSpeech] (TTS) into a single coroutine-friendly API.
 *
 * Why native Android APIs instead of a cloud service:
 * - Privacy: audio never leaves the device.
 * - Offline: works without internet (offline recognition models).
 * - Latency: no round-trip to a cloud STT.
 *
 * Trade-off: recognition quality is lower than cloud STT. For higher
 * quality, route the audio through the SPS kernel's LLM-based
 * transcription (via the providers-http crate).
 */
class VoiceManager(private val context: Context) {

    private val _state = MutableStateFlow<VoiceState>(VoiceState.Idle)
    val state: StateFlow<VoiceState> = _state.asStateFlow()

    private val _partialResults = MutableSharedFlow<String>(extraBufferCapacity = 16)
    val partialResults: SharedFlow<String> = _partialResults.asSharedFlow()

    private var recognizer: SpeechRecognizer? = null
    private var tts: TextToSpeech? = null
    private val ttsReady = AtomicBoolean(false)

    init {
        // Initialize TTS engine.
        tts = TextToSpeech(context) { status ->
            if (status == TextToSpeech.SUCCESS) {
                tts?.language = Locale.getDefault()
                ttsReady.set(true)
                Log.i(TAG, "TTS ready")
            } else {
                Log.e(TAG, "TTS init failed: status=$status")
            }
        }
    }

    /** true if STT is available on this device. */
    fun isSttAvailable(): Boolean = SpeechRecognizer.isRecognitionAvailable(context)

    /** true if TTS engine is ready. */
    fun isTtsReady(): Boolean = ttsReady.get()

    /**
     * Listen for a single utterance. Returns the recognized text.
     *
     * Suspends until recognition completes (success, error, or timeout).
     * Cancellation cancels the recognition.
     */
    suspend fun listenOnce(): String = suspendCancellableCoroutine { cont ->
        if (!isSttAvailable()) {
            cont.resume("")
            return@suspendCancellableCoroutine
        }

        ensureRecognizer()

        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, Locale.getDefault().toLanguageTag())
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, context.packageName)
        }

        val listener = object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) {
                _state.value = VoiceState.Listening
            }
            override fun onBeginningOfSpeech() { _state.value = VoiceState.Listening }
            override fun onRmsChanged(rms: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() { _state.value = VoiceState.Processing }

            override fun onError(error: Int) {
                _state.value = VoiceState.Error(errorCodeToMessage(error))
                if (cont.isActive) cont.resume("")
            }

            override fun onResults(results: Bundle?) {
                val list = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                val text = list?.firstOrNull() ?: ""
                _state.value = VoiceState.Idle
                if (cont.isActive) cont.resume(text)
            }

            override fun onPartialResults(partial: Bundle?) {
                val list = partial?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                val text = list?.firstOrNull() ?: ""
                if (text.isNotEmpty()) _partialResults.tryEmit(text)
            }

            override fun onEvent(eventType: Int, params: Bundle?) {}
        }

        recognizer?.setRecognitionListener(listener)
        recognizer?.startListening(intent)

        cont.invokeOnCancellation {
            runCatching { recognizer?.stopListening() }
            _state.value = VoiceState.Idle
        }
    }

    /** Speak text aloud via TTS. Non-blocking. */
    fun speak(text: String) {
        if (!ttsReady.get() || text.isBlank()) return
        _state.value = VoiceState.Speaking
        tts?.speak(text, TextToSpeech.QUEUE_FLUSH, null, "sps_${System.currentTimeMillis()}")
        // Poll for completion. (UtteranceProgressListener is more accurate
        // but requires more setup; this is good enough for a companion.)
        tts?.setOnUtteranceProgressListener(object : android.speech.tts.UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {}
            override fun onDone(utteranceId: String?) {
                _state.value = VoiceState.Idle
            }
            override fun onError(utteranceId: String?) {
                _state.value = VoiceState.Idle
            }
        })
    }

    /** Stop any in-progress TTS. */
    fun stopSpeaking() {
        tts?.stop()
        _state.value = VoiceState.Idle
    }

    /** Stop any in-progress recognition. */
    fun stopListening() {
        runCatching { recognizer?.stopListening() }
        _state.value = VoiceState.Idle
    }

    private fun ensureRecognizer() {
        if (recognizer == null) {
            recognizer = SpeechRecognizer.createSpeechRecognizer(context)
        }
    }

    private fun errorCodeToMessage(code: Int): String = when (code) {
        SpeechRecognizer.ERROR_NETWORK_TIMEOUT -> "network timeout"
        SpeechRecognizer.ERROR_NETWORK -> "network error"
        SpeechRecognizer.ERROR_AUDIO -> "audio error"
        SpeechRecognizer.ERROR_SERVER -> "server error"
        SpeechRecognizer.ERROR_CLIENT -> "client error"
        SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "no speech detected"
        SpeechRecognizer.ERROR_NO_MATCH -> "no match"
        SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "recognizer busy"
        SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "no microphone permission"
        else -> "unknown error ($code)"
    }

    fun release() {
        runCatching { recognizer?.destroy() }
        runCatching { tts?.stop() }
        runCatching { tts?.shutdown() }
        recognizer = null
        tts = null
    }

    companion object {
        private const val TAG = "VoiceManager"
    }
}

/** Voice manager state — observed by the UI. */
sealed interface VoiceState {
    data object Idle : VoiceState
    data object Listening : VoiceState
    data object Processing : VoiceState
    data object Speaking : VoiceState
    data class Error(val message: String) : VoiceState
}
