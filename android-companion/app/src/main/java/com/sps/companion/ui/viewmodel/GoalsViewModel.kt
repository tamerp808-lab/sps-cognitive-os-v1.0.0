package com.sps.companion.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsCreateGoalRequest
import com.sps.companion.data.SpsGoal
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class GoalsViewModel : ViewModel() {

    private val _goals = MutableStateFlow<List<SpsGoal>>(emptyList())
    val goals: StateFlow<List<SpsGoal>> = _goals.asStateFlow()

    init { refresh() }

    fun refresh() {
        viewModelScope.launch {
            try {
                _goals.value = SpsApplication.client().listGoals()
            } catch (_: Exception) { }
        }
    }

    fun createGoal(title: String, description: String) {
        viewModelScope.launch {
            try {
                SpsApplication.client().createGoal(SpsCreateGoalRequest(title = title, description = description))
                refresh()
            } catch (_: Exception) { }
        }
    }
}
