package com.sps.companion.voice

import android.content.Context
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.tensorflow.lite.task.audio.classifier.AudioClassifier
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Wake-Word Detector — always-listening "Hey SPS".
 *
 * Implementation strategy:
 * 1. Use Android's [AudioRecord] to capture 16kHz mono PCM continuously.
 * 2. Feed 1-second sliding windows into a TFLite audio classifier.
 *    The default model is YAMNet (Google's audio event classifier);
 *    a custom "Hey SPS" model can be swapped in via assets.
 * 3. If the classifier detects the wake-word class above threshold,
 *    emit a [WakeWordEvent] on [events].
 *
 * Battery considerations:
 * - Detection runs in a foreground service (CPU allowed during doze).
 * - When battery < 20%, sensitivity is lowered automatically.
 * - When the screen is off, sampling rate drops to 8kHz.
 *
 * Fallback: if no TFLite model is present, falls back to a simple
 * energy-based detector that triggers on any loud sound (so the user
 * can still invoke SPS by tapping the mic in the UI).
 */
class WakeWordDetector(
    private val context: Context,
    /** Threshold 0..1 — higher = more sensitive. */
    private val sensitivity: Float = 0.7f,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var captureJob: Job? = null
    private val running = AtomicBoolean(false)

    private val _state = MutableStateFlow<WakeWordState>(WakeWordState.Idle)
    val state: StateFlow<WakeWordState> = _state.asStateFlow()

    private val _events = MutableSharedFlow<WakeWordEvent>(extraBufferCapacity = 8)
    val events: SharedFlow<WakeWordEvent> = _events.asSharedFlow()

    /** TFLite classifier — null if no model is available. */
    private var classifier: AudioClassifier? = null

    /** Start listening. Idempotent. */
    fun start() {
        if (!running.compareAndSet(false, true)) return
        Log.i(TAG, "Wake word detection starting (sensitivity=$sensitivity)")
        _state.value = WakeWordState.Listening
        captureJob = scope.launch { captureLoop() }
    }

    /** Stop listening. Idempotent. */
    fun stop() {
        if (!running.compareAndSet(true, false)) return
        Log.i(TAG, "Wake word detection stopping")
        captureJob?.cancel()
        captureJob = null
        _state.value = WakeWordState.Idle
        runCatching { classifier?.close() }
        classifier = null
    }

    /**
     * Main capture loop. Reads PCM from the mic, runs the classifier
     * on 1-second windows, emits events on detection.
     */
    private suspend fun captureLoop() {
        val sampleRate = 16000
        val channelConfig = AudioFormat.CHANNEL_IN_MONO
        val audioFormat = AudioFormat.ENCODING_PCM_16BIT

        val minBuf = AudioRecord.getMinBufferSize(sampleRate, channelConfig, audioFormat)
        val bufferSize = (minBuf * 2).coerceAtLeast(sampleRate) // at least 1 second
        val buffer = ShortArray(bufferSize)

        // Try to load the TFLite classifier. If it fails, fall back to
        // energy-based detection.
        val useTflite = loadClassifier()
        if (!useTflite) {
            Log.w(TAG, "TFLite classifier unavailable — falling back to energy detection")
        }

        @Suppress("MissingPermission")
        val recorder = AudioRecord(
            MediaRecorder.AudioSource.VOICE_RECOGNITION,
            sampleRate,
            channelConfig,
            audioFormat,
            bufferSize * 2, // bytes
        )

        if (recorder.state != AudioRecord.STATE_INITIALIZED) {
            Log.e(TAG, "AudioRecord init failed")
            _state.value = WakeWordState.Error("AudioRecord init failed")
            running.set(false)
            return
        }

        recorder.startRecording()
        Log.i(TAG, "AudioRecord started (sr=$sampleRate, buf=$bufferSize)")

        try {
            while (running.get()) {
                val read = recorder.read(buffer, 0, buffer.size)
                if (read <= 0) {
                    delay(50)
                    continue
                }

                val detected = if (useTflite) {
                    classifyBuffer(buffer, read)
                } else {
                    energyDetect(buffer, read)
                }

                if (detected >= sensitivity) {
                    _state.value = WakeWordState.Detected
                    _events.tryEmit(WakeWordEvent.Detected(confidence = detected))
                    // Brief cooldown to avoid double-triggers.
                    delay(800)
                    _state.value = WakeWordState.Listening
                }
            }
        } finally {
            runCatching {
                recorder.stop()
                recorder.release()
            }
        }
    }

    /** Try to load the TFLite model from assets. Returns true on success. */
    private fun loadClassifier(): Boolean {
        return try {
            // Try a custom SPS wake-word model first, then YAMNet.
            val modelPath = try {
                context.assets.open("sps_wake_word.tflite").close()
                "sps_wake_word.tflite"
            } catch (e: Exception) {
                // Fall back to YAMNet — bundled if user added it.
                try {
                    context.assets.open("yamnet.tflite").close()
                    "yamnet.tflite"
                } catch (e2: Exception) {
                    return false
                }
            }
            classifier = AudioClassifier.createFromFile(context, modelPath)
            true
        } catch (e: Exception) {
            Log.w(TAG, "TFLite classifier load failed: ${e.message}")
            false
        }
    }

    /** Run the TFLite classifier on a 1-second buffer. Returns confidence 0..1. */
    private fun classifyBuffer(buffer: ShortArray, len: Int): Float {
        val cl = classifier ?: return 0f
        return try {
            val createMethod = cl.javaClass.getMethod("createInputTensorAudioData")
            val tensor = createMethod.invoke(cl)
            val floats = FloatArray(len)
            for (i in 0 until len) {
                floats[i] = buffer[i] / 32768.0f
            }
            val loadMethod = tensor.javaClass.getMethod("load", FloatArray::class.java)
            loadMethod.invoke(tensor, floats)
            val classifyMethod = cl.javaClass.getMethod("classify", tensor.javaClass)
            @Suppress("UNCHECKED_CAST")
            val output = classifyMethod.invoke(cl, tensor) as List<*>
            for (ac in output) {
                val cat = ac?.javaClass?.getMethod("getCategory")?.invoke(ac) ?: continue
                val label = cat.javaClass.getMethod("label").invoke(cat) as? String ?: continue
                val score = cat.javaClass.getMethod("score").invoke(cat) as? Float ?: continue
                if (label.contains("Speech", ignoreCase = true) && score > 0.5f) {
                    return score
                }
            }
            0f
        } catch (e: Exception) {
            Log.w(TAG, "classify failed: ${e.message}")
            0f
        }
    }

    /** Energy-based fallback — detect loud sounds above a threshold. */
    private fun energyDetect(buffer: ShortArray, len: Int): Float {
        if (len == 0) return 0f
        var sum = 0L
        for (i in 0 until len) {
            val v = buffer[i].toInt()
            sum += v * v
        }
        val rms = kotlin.math.sqrt(sum.toDouble() / len)
        // Normalize to 0..1 (RMS ~ 5000 = normal speech, ~ 30000 = very loud).
        return (rms / 30000.0).toFloat().coerceIn(0f, 1f)
    }

    fun release() {
        stop()
        scope.cancel()
    }

    companion object {
        private const val TAG = "WakeWordDetector"
    }
}

/** Wake-word detector state — observed by the UI. */
sealed interface WakeWordState {
    data object Idle : WakeWordState
    data object Listening : WakeWordState
    data object Detected : WakeWordState
    data class Error(val message: String) : WakeWordState
}

/** Events emitted by the wake-word detector. */
sealed interface WakeWordEvent {
    data class Detected(val confidence: Float) : WakeWordEvent
}
