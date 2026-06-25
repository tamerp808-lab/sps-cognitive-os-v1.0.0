package com.sps.companion.receiver

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import com.sps.companion.MainActivity
import com.sps.companion.R

/**
 * SPS Home Screen Widget — a single tap-to-talk button.
 *
 * Tap the widget → launches MainActivity with action=talk, which
 * immediately opens the voice screen ready to listen.
 */
class SpsWidgetProvider : AppWidgetProvider() {

    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        for (id in ids) {
            val views = RemoteViews(context.packageName, R.layout.widget_sps)
            // Tap → open MainActivity with action=talk.
            val intent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
                putExtra("sps.action", "talk")
            }
            val pi = PendingIntent.getActivity(
                context, id, intent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            views.setOnClickPendingIntent(R.id.widget_icon, pi)
            views.setOnClickPendingIntent(R.id.widget_text, pi)
            manager.updateAppWidget(id, views)
        }
    }
}
