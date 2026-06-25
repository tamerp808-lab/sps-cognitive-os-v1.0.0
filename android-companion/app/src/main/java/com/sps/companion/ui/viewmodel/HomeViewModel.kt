package com.sps.companion.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsBriefing
import com.sps.companion.data.SpsGoal
import com.sps.companion.data.SpsMemory
import com.sps.companion.network.SpsConnectionState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Home Screen ViewModel — loads the daily briefing, active goals,
 * and recent memories from the SPS kernel.
 *
 * All errors are swallowed and reported as empty state — the UI should
 * gracefully handle the disconnected case.
 */
class HomeViewModel : ViewModel() {

    private val _briefing = MutableStateFlow(SpsBriefing())
    val briefing: StateFlow<SpsBriefing> = _briefing.asStateFlow()

    private val _goals = MutableStateFlow<List<SpsGoal>>(emptyList())
    val goals: StateFlow<List<SpsGoal>> = _goals.asStateFlow()

    private val _memories = MutableStateFlow<List<SpsMemory>>(emptyList())
    val memories: StateFlow<List<SpsMemory>> = _memories.asStateFlow()

    private val _connectionState = MutableStateFlow<SpsConnectionState>(SpsConnectionState.Disconnected)
    val connectionState: StateFlow<SpsConnectionState> = _connectionState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            try {
                _connectionState.value = SpsApplication.client().connectionState.value
                _briefing.value = SpsApplication.client().getBriefing()
            } catch (_: Exception) { }
            try {
                _goals.value = SpsApplication.client().listGoals()
            } catch (_: Exception) { }
            try {
                _memories.value = SpsApplication.client().listMemories(10)
            } catch (_: Exception) { }
        }
    }
}
