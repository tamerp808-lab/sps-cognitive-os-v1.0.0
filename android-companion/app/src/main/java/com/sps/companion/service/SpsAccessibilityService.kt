package com.sps.companion.service

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.Context
import android.content.Intent
import android.graphics.Path
import android.provider.Settings
import android.os.Bundle
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import com.sps.companion.SpsApplication
import com.sps.companion.voice.VoiceManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * SPS Accessibility Service — screen reading + action automation.
 *
 * Capabilities (when enabled by the user):
 * - "Read what's on the screen" — speaks the visible text on the current app.
 * - "Tap the <X> button" — finds a UI element by label and taps it.
 * - "Scroll down" — performs a swipe-up gesture.
 * - "Go back" — presses the BACK button.
 * - "Go home" — presses the HOME button.
 * - "Find <X> on the screen" — searches for text, speaks the context.
 *
 * Privacy: this service does NOT log or transmit screen contents
 * continuously. It only acts on explicit user commands. When idle, it
 * receives events but discards them immediately.
 */
class SpsAccessibilityService : AccessibilityService() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // We deliberately don't process events continuously — only on-demand.
        // This is a privacy-first design.
    }

    override fun onInterrupt() {
        Log.w(TAG, "Accessibility service interrupted")
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        Log.i(TAG, "SPS Accessibility service connected")
        instance = this
    }

    override fun onDestroy() {
        super.onDestroy()
        instance = null
    }

    /**
     * Read all visible text on the current screen.
     * Returns the concatenated text (also speaks it via TTS).
     */
    suspend fun readScreen(voice: VoiceManager? = null): String {
        val root = rootInActiveWindow ?: return "Screen is empty."
        val text = collectText(root)
        voice?.speak(text.take(2000))
        return text
    }

    /** Recursively collect all text from a node and its descendants. */
    private fun collectText(node: AccessibilityNodeInfo): String {
        val sb = StringBuilder()
        node.text?.let { sb.append(it).append(' ') }
        node.contentDescription?.let { sb.append(it).append(' ') }
        for (i in 0 until node.childCount) {
            node.getChild(i)?.let { sb.append(collectText(it)).append(' ') }
        }
        return sb.toString().trim()
    }

    /**
     * Find a UI element by text or content description, tap it.
     * Returns true if found and tapped.
     */
    suspend fun tapElement(label: String): Boolean {
        val root = rootInActiveWindow ?: return false
        val target = label.lowercase().trim()
        // Search by text.
        val byText = root.findAccessibilityNodeInfosByText(label)
        val node = byText.firstOrNull { it.isClickable } ?: byText.firstOrNull()
        if (node != null) {
            return performClick(node)
        }
        // Search by content description.
        val byDesc = findByContentDescription(root, target)
        if (byDesc != null) {
            return performClick(byDesc)
        }
        return false
    }

    /** Recursively find a node whose content description matches. */
    private fun findByContentDescription(
        node: AccessibilityNodeInfo,
        target: String,
    ): AccessibilityNodeInfo? {
        node.contentDescription?.toString()?.lowercase()?.let { desc ->
            if (desc.contains(target) && node.isClickable) return node
        }
        for (i in 0 until node.childCount) {
            node.getChild(i)?.let { child ->
                findByContentDescription(child, target)?.let { return it }
            }
        }
        return null
    }

    /** Click a node (or its parent if the node itself isn't clickable). */
    private fun performClick(node: AccessibilityNodeInfo): Boolean {
        var n: AccessibilityNodeInfo? = node
        while (n != null && !n.isClickable) {
            n = n.parent
        }
        return n?.performAction(AccessibilityNodeInfo.ACTION_CLICK) ?: false
    }

    /** Swipe up — scrolls down. */
    fun scrollDown() = performSwipe(startY = 0.7f, endY = 0.3f)

    /** Swipe down — scrolls up. */
    fun scrollUp() = performSwipe(startY = 0.3f, endY = 0.7f)

    /** Swipe left — go to next page/tab. */
    fun swipeLeft() = performSwipe(startX = 0.8f, endX = 0.2f, axis = "x")

    /** Swipe right — go to previous page/tab. */
    fun swipeRight() = performSwipe(startX = 0.2f, endX = 0.8f, axis = "x")

    private fun performSwipe(
        startX: Float = 0.5f,
        endX: Float = 0.5f,
        startY: Float = 0.5f,
        endY: Float = 0.5f,
        axis: String = "y",
    ): Boolean {
        val w = resources.displayMetrics.widthPixels
        val h = resources.displayMetrics.heightPixels
        val path = Path().apply {
            moveTo(startX * w, startY * h)
            lineTo(endX * w, endY * h)
        }
        val stroke = GestureDescription.StrokeDescription(path, 0, 300)
        return dispatchGesture(GestureDescription.Builder().addStroke(stroke).build(), null, null)
    }

    /** Press the BACK button. */
    fun goBack() = performGlobalAction(GLOBAL_ACTION_BACK)

    /** Press the HOME button. */
    fun goHome() = performGlobalAction(GLOBAL_ACTION_HOME)

    /** Press the RECENTS button. */
    fun goRecents() = performGlobalAction(GLOBAL_ACTION_RECENTS)

    /** Open the notification shade. */
    fun openNotifications() = performGlobalAction(GLOBAL_ACTION_NOTIFICATIONS)

    /** Open the quick settings shade. */
    fun openQuickSettings() = performGlobalAction(GLOBAL_ACTION_QUICK_SETTINGS)

    companion object {
        private const val TAG = "SpsAccessibilityService"

        @Volatile private var instance: SpsAccessibilityService? = null

        /** Run an action with the live accessibility service (if enabled). */
        suspend fun with(action: suspend (SpsAccessibilityService) -> Unit): Boolean {
            val svc = instance ?: return false
            action(svc)
            return true
        }

        /** true if the user has enabled this service. */
        fun isEnabled(context: Context): Boolean {
            val enabled = Settings.Secure.getString(
                context.contentResolver,
                android.provider.Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
            ) ?: return false
            return enabled.contains(context.packageName + "/.service.SpsAccessibilityService")
        }
    }
}
