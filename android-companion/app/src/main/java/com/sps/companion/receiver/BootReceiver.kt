package com.sps.companion.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.sps.companion.data.SpsConfigManager
import com.sps.companion.service.SpsForegroundService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * Boot Receiver — restarts SPS on device boot.
 *
 * If the user has "Start on Boot" enabled (default), this receiver
 * starts [SpsForegroundService] as soon as the device finishes booting.
 * This is what makes "Hey SPS" work immediately after the phone turns on.
 *
 * Note: on Android 10+, BOOT_COMPLETED is delivered after the user
 * unlocks the device for the first time. For truly pre-unlock start,
 * use LOCKED_BOOT_COMPLETED (delivered before unlock) — but only
 * Direct Boot-aware apps can run then. We fall back to BOOT_COMPLETED.
 */
class BootReceiver : BroadcastReceiver() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED,
            Intent.ACTION_LOCKED_BOOT_COMPLETED,
            Intent.ACTION_MY_PACKAGE_REPLACED -> {
                Log.i(TAG, "Boot received — checking start-on-boot setting")
                scope.launch {
                    val config = SpsConfigManager(context).config.first()
                    if (config.startOnBoot) {
                        Log.i(TAG, "Start-on-boot enabled — starting SPS service")
                        SpsForegroundService.start(context)
                    } else {
                        Log.i(TAG, "Start-on-boot disabled — skipping")
                    }
                }
            }
        }
    }

    companion object {
        private const val TAG = "BootReceiver"
    }
}
