package com.sps.companion.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update

private val Context.spsDataStore: DataStore<Preferences> by preferencesDataStore(name = "sps_settings")

/**
 * DataStore-backed configuration manager.
 *
 * Exposes the current [SpsConfig] as a [StateFlow] so observers
 * (services, UI) get reactive updates when settings change.
 */
class SpsConfigManager(private val context: Context) {

    private val _config = MutableStateFlow(SpsConfig())
    val config: StateFlow<SpsConfig> = _config.asStateFlow()

    init {
        // Load initial state synchronously into the StateFlow so services
        // that start on boot see the persisted config immediately.
        // (Real load happens below; this is the fallback default.)
        kotlinx.coroutines.runBlocking { reload() }
    }

    /** Reload config from DataStore into the StateFlow. */
    suspend fun reload() {
        val prefs = context.spsDataStore.data.first()
        _config.value = SpsConfig(
            serverUrl = prefs[KEY_SERVER_URL] ?: "http://127.0.0.1:7780",
            wakeWordEnabled = prefs[KEY_WAKE_WORD_ENABLED] ?: true,
            wakeWordPhrase = prefs[KEY_WAKE_WORD_PHRASE] ?: "Hey SPS",
            wakeWordSensitivity = prefs[KEY_WAKE_WORD_SENSITIVITY] ?: 0.7f,
            ttsEnabled = prefs[KEY_TTS_ENABLED] ?: true,
            continuousMode = prefs[KEY_CONTINUOUS_MODE] ?: false,
            vibrateOnWake = prefs[KEY_VIBRATE_ON_WAKE] ?: true,
            overlayEnabled = prefs[KEY_OVERLAY_ENABLED] ?: false,
            startOnBoot = prefs[KEY_START_ON_BOOT] ?: true,
            defaultProvider = prefs[KEY_DEFAULT_PROVIDER],
            themeMode = prefs[KEY_THEME_MODE] ?: "system",
            language = prefs[KEY_LANGUAGE],
        )
    }

    /** Update a single field. Persists to DataStore + emits to the StateFlow. */
    suspend fun update(transform: (SpsConfig) -> SpsConfig) {
        context.spsDataStore.edit { prefs ->
            val current = _config.value
            val next = transform(current)
            prefs[KEY_SERVER_URL] = next.serverUrl
            prefs[KEY_WAKE_WORD_ENABLED] = next.wakeWordEnabled
            prefs[KEY_WAKE_WORD_PHRASE] = next.wakeWordPhrase
            prefs[KEY_WAKE_WORD_SENSITIVITY] = next.wakeWordSensitivity
            prefs[KEY_TTS_ENABLED] = next.ttsEnabled
            prefs[KEY_CONTINUOUS_MODE] = next.continuousMode
            prefs[KEY_VIBRATE_ON_WAKE] = next.vibrateOnWake
            prefs[KEY_OVERLAY_ENABLED] = next.overlayEnabled
            prefs[KEY_START_ON_BOOT] = next.startOnBoot
            prefs[KEY_THEME_MODE] = next.themeMode
            next.defaultProvider?.let { prefs[KEY_DEFAULT_PROVIDER] = it }
            next.language?.let { prefs[KEY_LANGUAGE] = it }
            _config.value = next
        }
    }

    suspend fun setServerUrl(url: String) = update { it.copy(serverUrl = url) }
    suspend fun setWakeWordEnabled(enabled: Boolean) = update { it.copy(wakeWordEnabled = enabled) }
    suspend fun setTtsEnabled(enabled: Boolean) = update { it.copy(ttsEnabled = enabled) }
    suspend fun setOverlayEnabled(enabled: Boolean) = update { it.copy(overlayEnabled = enabled) }
    suspend fun setContinuousMode(enabled: Boolean) = update { it.copy(continuousMode = enabled) }

    companion object {
        private val KEY_SERVER_URL = stringPreferencesKey("server_url")
        private val KEY_WAKE_WORD_ENABLED = booleanPreferencesKey("wake_word_enabled")
        private val KEY_WAKE_WORD_PHRASE = stringPreferencesKey("wake_word_phrase")
        private val KEY_WAKE_WORD_SENSITIVITY = floatPreferencesKey("wake_word_sensitivity")
        private val KEY_TTS_ENABLED = booleanPreferencesKey("tts_enabled")
        private val KEY_CONTINUOUS_MODE = booleanPreferencesKey("continuous_mode")
        private val KEY_VIBRATE_ON_WAKE = booleanPreferencesKey("vibrate_on_wake")
        private val KEY_OVERLAY_ENABLED = booleanPreferencesKey("overlay_enabled")
        private val KEY_START_ON_BOOT = booleanPreferencesKey("start_on_boot")
        private val KEY_DEFAULT_PROVIDER = stringPreferencesKey("default_provider")
        private val KEY_THEME_MODE = stringPreferencesKey("theme_mode")
        private val KEY_LANGUAGE = stringPreferencesKey("language")
    }
}
