package com.sps.companion.service

import android.content.Context
import android.content.Intent

/**
 * Wake-Word Service — thin alias for [SpsForegroundService].
 *
 * Historically wake-word listening was a separate service; it's now
 * integrated into the foreground service (which holds the wake lock +
 * persistent notification anyway). This class exists for backwards
 * compatibility with the manifest declaration and any external
 * components that explicitly target it.
 *
 * Calling [start] / [stop] here is equivalent to calling them on
 * [SpsForegroundService].
 */
class WakeWordService {
    companion object {
        fun start(context: Context) = SpsForegroundService.start(context)
        fun stop(context: Context) = SpsForegroundService.stop(context)
    }
}
