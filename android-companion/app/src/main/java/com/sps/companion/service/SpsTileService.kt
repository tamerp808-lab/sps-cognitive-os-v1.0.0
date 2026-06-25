package com.sps.companion.service

import android.content.Intent
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log
import com.sps.companion.MainActivity

/**
 * Quick Settings Tile — one-tap access to SPS voice.
 *
 * Pull down the quick settings shade, tap "SPS Voice" → opens the
 * voice screen ready to listen.
 *
 * Tile state: always ACTIVE (we don't toggle, we open the app).
 * For a toggle tile (e.g. enable/disable wake word), implement
 * onStartListening / onClick to flip the state.
 */
class SpsTileService : TileService() {

    override fun onStartListening() {
        super.onStartListening()
        qsTile?.apply {
            state = Tile.STATE_ACTIVE
            label = "SPS Voice"
            updateTile()
        }
    }

    override fun onClick() {
        super.onClick()
        Log.i(TAG, "QS tile tapped — launching voice")
        val intent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK
            putExtra("sps.action", "talk")
        }
        startActivityAndCollapse(intent)
    }

    companion object {
        private const val TAG = "SpsTileService"
    }
}
