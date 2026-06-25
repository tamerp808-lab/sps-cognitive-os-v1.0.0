package com.sps.companion.network

import kotlinx.coroutines.flow.StateFlow

/**
 * SPS server connection state — observed by the UI + services.
 */
sealed interface SpsConnectionState {
    /** Not yet attempted to connect. */
    data object Disconnected : SpsConnectionState
    /** Connection in progress. */
    data object Connecting : SpsConnectionState
    /** Successfully connected and authenticated. */
    data object Connected : SpsConnectionState
    /** Connection failed — [message] describes why. */
    data class Failed(val message: String) : SpsConnectionState
}
