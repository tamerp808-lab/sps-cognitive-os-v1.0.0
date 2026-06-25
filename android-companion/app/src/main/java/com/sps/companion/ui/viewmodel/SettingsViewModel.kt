package com.sps.companion.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sps.companion.SpsApplication
import com.sps.companion.data.SpsConfig
import com.sps.companion.network.SpsConnectionState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class SettingsViewModel : ViewModel() {

    private val configManager = SpsApplication.config()

    val config: StateFlow<SpsConfig> = configManager.config

    private val _connectionState = MutableStateFlow<SpsConnectionState>(SpsConnectionState.Disconnected)
    val connectionState: StateFlow<SpsConnectionState> = _connectionState.asStateFlow()

    init {
        viewModelScope.launch {
            SpsApplication.client().connectionState.collect { _connectionState.value = it }
        }
    }

    suspend fun setServerUrl(url: String) {
        configManager.setServerUrl(url)
        SpsApplication.get().reconnect(url)
        SpsApplication.client().healthCheck()
    }
    suspend fun setWakeWordEnabled(b: Boolean) = configManager.setWakeWordEnabled(b)
    suspend fun setTtsEnabled(b: Boolean) = configManager.setTtsEnabled(b)
    suspend fun setOverlayEnabled(b: Boolean) = configManager.setOverlayEnabled(b)
    suspend fun setContinuousMode(b: Boolean) = configManager.setContinuousMode(b)
    suspend fun setVibrateOnWake(b: Boolean) = configManager.update { it.copy(vibrateOnWake = b) }
    suspend fun setStartOnBoot(b: Boolean) = configManager.update { it.copy(startOnBoot = b) }
}
