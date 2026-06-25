package com.sps.companion.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsMemory
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class MemoryViewModel : ViewModel() {

    private val _memories = MutableStateFlow<List<SpsMemory>>(emptyList())
    val memories: StateFlow<List<SpsMemory>> = _memories.asStateFlow()

    private var searchJob: Job? = null

    init { refresh() }

    fun refresh() {
        viewModelScope.launch {
            try {
                _memories.value = SpsApplication.client().listMemories(50)
            } catch (_: Exception) { }
        }
    }

    /** Debounced search — waits 300ms after the user stops typing. */
    fun search(query: String) {
        searchJob?.cancel()
        if (query.isBlank()) {
            refresh()
            return
        }
        searchJob = viewModelScope.launch {
            delay(300)
            try {
                _memories.value = SpsApplication.client().searchMemories(query)
            } catch (_: Exception) { }
        }
    }
}
