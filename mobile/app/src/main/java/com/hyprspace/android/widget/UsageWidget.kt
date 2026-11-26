// The home screen's usage widget: each agent's plan limits as the computer last sent them. It
// draws from what the app saved, since a widget can't hold a connection of its own, so it's as
// fresh as the last time the app was connected and says when that was.

package com.hyprspace.android.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.text.format.DateFormat
import android.view.View
import android.widget.RemoteViews
import com.hyprspace.android.App
import com.hyprspace.android.MainActivity
import com.hyprspace.android.R
import kotlin.math.roundToInt

class UsageWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        for (id in ids) draw(context, manager, id)
    }

    override fun onAppWidgetOptionsChanged(context: Context, manager: AppWidgetManager, id: Int, options: Bundle) {
        draw(context, manager, id)
    }

    companion object {
        private val ROWS = intArrayOf(R.id.row1, R.id.row2, R.id.row3, R.id.row4)
        private val LABELS = intArrayOf(R.id.label1, R.id.label2, R.id.label3, R.id.label4)
        private val PERCENTS = intArrayOf(R.id.percent1, R.id.percent2, R.id.percent3, R.id.percent4)
        private val BARS = intArrayOf(R.id.bar1, R.id.bar2, R.id.bar3, R.id.bar4)

        /** Draws every placed widget again, after the computer sent new numbers. */
        fun refresh(context: Context) {
            val manager = AppWidgetManager.getInstance(context) ?: return
            for (id in manager.getAppWidgetIds(ComponentName(context, UsageWidget::class.java))) draw(context, manager, id)
        }

        private fun draw(context: Context, manager: AppWidgetManager, id: Int) {
            val saved = (context.applicationContext as App).store.saved.value
            val views = RemoteViews(context.packageName, R.layout.widget_usage)
            val tall = manager.getAppWidgetOptions(id).getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT, 110)
            val fit = ((tall - 48) / 30).coerceIn(1, ROWS.size)
            val bars = saved.usage.flatMap { u -> u.bars.map { u.agent.label to it } }.take(fit)
            views.setViewVisibility(R.id.empty, if (bars.isEmpty()) View.VISIBLE else View.GONE)
            views.setTextViewText(
                R.id.asof,
                if (bars.isEmpty()) "" else DateFormat.getTimeFormat(context).format(saved.usageAt),
            )
            for (i in ROWS.indices) {
                val bar = bars.getOrNull(i)
                views.setViewVisibility(ROWS[i], if (bar == null) View.GONE else View.VISIBLE)
                val (agent, b) = bar ?: continue
                views.setTextViewText(LABELS[i], "$agent · ${b.label}")
                views.setTextViewText(PERCENTS[i], "${b.percent.roundToInt()}%")
                views.setProgressBar(BARS[i], 100, b.percent.roundToInt().coerceIn(0, 100), false)
                if (Build.VERSION.SDK_INT >= 31) {
                    val fill = when {
                        b.percent >= 90 -> R.color.widget_full
                        b.percent >= 70 -> R.color.widget_warn
                        else -> R.color.widget_text
                    }
                    // by id, so the launcher picks the light or dark value when the system theme turns
                    views.setColorStateList(BARS[i], "setProgressTintList", fill)
                }
            }
            val open = PendingIntent.getActivity(
                context, 0, Intent(context, MainActivity::class.java),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            views.setOnClickPendingIntent(R.id.widget, open)
            manager.updateAppWidget(id, views)
        }
    }
}
