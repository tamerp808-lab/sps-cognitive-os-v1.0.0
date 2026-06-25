package com.sps.companion.ui.viewmodel

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsCompletionRequest
import com.sps.companion.voice.VoiceManager
import com.sps.companion.voice.VoiceState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Voice Screen ViewModel — owns the [VoiceManager] and orchestrates
 * the listen → SPS → speak pipeline.
 *
 * The VoiceManager is created lazily and released when the screen
 * is destroyed (via [stopListening]).
 */
class VoiceViewModel : ViewModel() {

    private var voice: VoiceManager? = null

    private val _state = MutableStateFlow<VoiceState>(VoiceState.Idle)
    val state: StateFlow<VoiceState> = _state.asStateFlow()

    private val _partialTranscript = MutableStateFlow("")
    val partialTranscript: StateFlow<String> = _partialTranscript.asStateFlow()

    private val _response = MutableStateFlow("")
    val response: StateFlow<String> = _response.asStateFlow()

    init {
        // Observe voice state.
        viewModelScope.launch {
            // We'll collect from voice?.state once it's initialized.
        }
    }

    fun startListening(context: Context) {
        if (voice == null) {
            voice = VoiceManager(context.applicationContext)
        }
        val v = voice ?: return

        // Wire up state + partial updates.
        viewModelScope.launch {
            v.state.collect { _state.value = it }
        }
        viewModelScope.launch {
            v.partialResults.collect { _partialTranscript.value = it }
        }

        _response.value = ""
        _partialTranscript.value = ""

        viewModelScope.launch {
            // Step 1: listen.
            val text = v.listenOnce()
            _partialTranscript.value = text
            if (text.isBlank()) return@launch

            // Step 2: send to SPS.
            _state.value = VoiceState.Processing
            try {
                val resp = SpsApplication.client().complete(
                    SpsCompletionRequest(
                        prompt = text,
                        system = "You are SPS, a personal cognitive AI companion. Respond concisely (1-3 sentences).",
                    )
                )
                _response.value = resp.text

                // Step 3: speak.
                if (SpsApplication.config().config.value.ttsEnabled && resp.text.isNotBlank()) {
                    v.speak(resp.text)
                }

                // Store as memory.
                runCatching {
                    SpsApplication.client().storeMemory(
                        content = "User: $text\nSPS: ${resp.text.take(500)}",
                        type = "episodic",
                    )
                }
            } catch (e: Exception) {
                _response.value = "Error: ${e.message}"
                _state.value = VoiceState.Error(e.message ?: "unknown")
            }
        }
    }

    fun stopListening() {
        voice?.stopListening()
        voice?.stopSpeaking()
    }

    override fun onCleared() {
        super.onCleared()
        voice?.release()
        voice = null
    }
}
