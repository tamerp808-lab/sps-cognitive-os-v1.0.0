package com.sps.companion.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsChatMessage
import com.sps.companion.data.SpsCompletionRequest
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Chat Screen ViewModel — manages the conversation history and streams
 * LLM responses token-by-token.
 */
class ChatViewModel : ViewModel() {

    private val _messages = MutableStateFlow<List<SpsChatMessage>>(emptyList())
    val messages: StateFlow<List<SpsChatMessage>> = _messages.asStateFlow()

    private val _streaming = MutableStateFlow(false)
    val streaming: StateFlow<Boolean> = _streaming.asStateFlow()

    fun send(text: String) {
        val userMsg = SpsChatMessage(role = "user", content = text, timestamp = System.currentTimeMillis())
        _messages.value = _messages.value + userMsg

        _streaming.value = true
        // Add an empty assistant message that we'll fill as tokens arrive.
        val assistantMsg = SpsChatMessage(role = "assistant", content = "", timestamp = System.currentTimeMillis(), streaming = true)
        _messages.value = _messages.value + assistantMsg

        viewModelScope.launch {
            try {
                val req = SpsCompletionRequest(
                    prompt = text,
                    system = "You are SPS, a personal cognitive AI companion running locally. Be concise and helpful.",
                )
                val sb = StringBuilder()
                SpsApplication.client().streamComplete(req).collect { token ->
                    if (token.isDone) {
                        _streaming.value = false
                        val list = _messages.value.toMutableList()
                        val idx = list.indexOfLast { it.role == "assistant" && it.streaming }
                        if (idx >= 0) list[idx] = list[idx].copy(streaming = false)
                        _messages.value = list
                    } else {
                        sb.append(token.delta)
                        val list = _messages.value.toMutableList()
                        val idx = list.indexOfLast { it.role == "assistant" && it.streaming }
                        if (idx >= 0) list[idx] = list[idx].copy(content = sb.toString())
                        _messages.value = list
                    }
                }
            } catch (e: Exception) {
                val list = _messages.value.toMutableList()
                val idx = list.indexOfLast { it.role == "assistant" && it.streaming }
                if (idx >= 0) {
                    list[idx] = list[idx].copy(
                        content = "Error: ${e.message}",
                        streaming = false,
                    )
                }
                _messages.value = list
                _streaming.value = false
            }
        }
    }
}
