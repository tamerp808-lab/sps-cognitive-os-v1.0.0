package com.sps.companion.overlay

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.os.Build
import android.os.IBinder
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.app.NotificationCompat
import com.sps.companion.R
import com.sps.companion.SpsApplication
import com.sps.companion.service.SpsForegroundService
import com.sps.companion.service.VoiceCommandHandler
import kotlin.math.abs

/**
 * Overlay Bubble Service — the floating SPS assistant.
 *
 * Shows a draggable circular bubble over any app. Tap to invoke voice
 * command. Long-press to dismiss.
 *
 * Requires [Settings.canDrawOverlays] permission — requested via the
 * Permissions screen.
 *
 * Layout:
 * - The bubble is a 64dp circle with the SPS logo.
 * - Tapping triggers [VoiceCommandHandler.handleWakeWord] (same as the
 *   wake word, but without needing to say "Hey SPS").
 * - Dragging moves the bubble around the screen edge.
 * - The bubble snaps to the nearest screen edge when released.
 */
class OverlayBubbleService : Service() {

    private lateinit var windowManager: WindowManager
    private var bubbleView: View? = null
    private var layoutParams: WindowManager.LayoutParams? = null

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "Overlay service creating")
        windowManager = getSystemService(WINDOW_SERVICE) as WindowManager
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(SpsForegroundService.NOTIFICATION_ID, buildNotification())

        if (!canDrawOverlays()) {
            Log.w(TAG, "No overlay permission — stopping")
            stopSelf()
            return START_NOT_STICKY
        }

        if (bubbleView == null) {
            showBubble()
        }

        return START_STICKY
    }

    /** Check if we have SYSTEM_ALERT_WINDOW permission. */
    private fun canDrawOverlays(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(this)

    /** Build a minimal foreground notification. */
    private fun buildNotification(): Notification =
        NotificationCompat.Builder(this, SpsApplication.CHANNEL_OVERLAY)
            .setContentTitle("SPS Overlay Active")
            .setContentText("Floating bubble is showing")
            .setSmallIcon(R.drawable.ic_sps)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .build()

    /** Inflate and add the bubble view to the window. */
    private fun showBubble() {
        // Build the bubble view programmatically — simpler than XML for an overlay.
        val size = (64 * resources.displayMetrics.density).toInt()
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setBackgroundResource(R.drawable.overlay_bubble)
        }
        val icon = ImageView(this).apply {
            setImageResource(R.drawable.ic_sps)
            layoutParams = LinearLayout.LayoutParams(size, size).apply {
                gravity = Gravity.CENTER
            }
            scaleType = ImageView.ScaleType.FIT_CENTER
            setPadding((8 * resources.displayMetrics.density).toInt(), (8 * resources.displayMetrics.density).toInt(), (8 * resources.displayMetrics.density).toInt(), (8 * resources.displayMetrics.density).toInt())
        }
        container.addView(icon)

        val params = WindowManager.LayoutParams(
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O)
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
            else
                @Suppress("DEPRECATION")
                WindowManager.LayoutParams.TYPE_PHONE,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
                WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = 0
            y = 200
        }

        layoutParams = params
        bubbleView = container

        // Drag + tap handling.
        var initialX = 0
        var initialY = 0
        var initialTouchX = 0f
        var initialTouchY = 0f
        var isDragging = false

        container.setOnTouchListener { _, event ->
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    initialX = params.x
                    initialY = params.y
                    initialTouchX = event.rawX
                    initialTouchY = event.rawY
                    isDragging = false
                    true
                }
                MotionEvent.ACTION_MOVE -> {
                    val dx = event.rawX - initialTouchX
                    val dy = event.rawY - initialTouchY
                    if (abs(dx) > 10 || abs(dy) > 10) isDragging = true
                    params.x = initialX + dx.toInt()
                    params.y = initialY + dy.toInt()
                    windowManager.updateViewLayout(container, params)
                    true
                }
                MotionEvent.ACTION_UP -> {
                    if (!isDragging) {
                        // Tap → trigger voice command.
                        Log.i(TAG, "Bubble tapped — triggering voice command")
                        VoiceCommandHandler.handleWakeWord(this)
                    } else {
                        // Snap to nearest edge.
                        snapToEdge(params)
                        windowManager.updateViewLayout(container, params)
                    }
                    true
                }
                else -> false
            }
        }

        try {
            windowManager.addView(container, params)
            Log.i(TAG, "Bubble added to window")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to add bubble view: ${e.message}", e)
            stopSelf()
        }
    }

    /** Snap the bubble to the nearest horizontal screen edge. */
    private fun snapToEdge(params: WindowManager.LayoutParams) {
        val displayWidth = resources.displayMetrics.widthPixels
        params.x = if (params.x < displayWidth / 2) 0 else displayWidth - (64 * resources.displayMetrics.density).toInt()
    }

    override fun onDestroy() {
        super.onDestroy()
        bubbleView?.let {
            runCatching { windowManager.removeView(it) }
        }
        bubbleView = null
        Log.i(TAG, "Overlay service destroyed")
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val TAG = "OverlayBubbleService"

        /** Start the overlay (no-op if no permission). */
        fun start(context: Context) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M && !Settings.canDrawOverlays(context)) {
                Log.w(TAG, "Cannot start — no overlay permission")
                return
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(Intent(context, OverlayBubbleService::class.java))
            } else {
                context.startService(Intent(context, OverlayBubbleService::class.java))
            }
        }

        /** Stop the overlay. */
        fun stop(context: Context) {
            context.stopService(Intent(context, OverlayBubbleService::class.java))
        }
    }
}
