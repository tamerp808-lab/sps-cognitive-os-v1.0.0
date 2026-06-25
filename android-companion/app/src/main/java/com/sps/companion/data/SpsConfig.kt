package com.sps.companion.data

import kotlinx.serialization.Serializable

/**
 * SPS Companion configuration — persisted in DataStore.
 *
 * Everything the user can tweak from the Settings screen lives here.
 * Changes are observed as a Flow so services + UI react in real-time.
 */
@Serializable
data class SpsConfig(
    /** SPS Rust kernel URL (default: 127.0.0.1:7780 — same machine, via Termux). */
    val serverUrl: String = "http://127.0.0.1:7780",
    /** Wake-word detection enabled (always-listening "Hey SPS"). */
    val wakeWordEnabled: Boolean = true,
    /** Wake-word phrase (only "Hey SPS" is supported out of the box; the
     *  TFLite model can be swapped for custom phrases). */
    val wakeWordPhrase: String = "Hey SPS",
    /** Wake-word sensitivity (0.0–1.0; higher = more sensitive, more false positives). */
    val wakeWordSensitivity: Float = 0.7f,
    /** TTS enabled — speak SPS responses out loud. */
    val ttsEnabled: Boolean = true,
    /** Continuous conversation mode — keep mic open after SPS responds. */
    val continuousMode: Boolean = false,
    /** Vibrate when wake word detected. */
    val vibrateOnWake: Boolean = true,
    /** Overlay bubble enabled — floating SPS over other apps. */
    val overlayEnabled: Boolean = false,
    /** Start service on device boot. */
    val startOnBoot: Boolean = true,
    /** Default LLM provider id (mirrors the server's configured providers). */
    val defaultProvider: String? = null,
    /** Theme mode: "system" | "light" | "dark". */
    val themeMode: String = "system",
    /** UI language (null = follow system). */
    val language: String? = null,
)
