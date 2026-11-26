// Allow or Deny tapped on a notification: the answer goes over the open connection, the same
// ask the thread's own card sends.

package com.hyprspace.android.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.hyprspace.android.App
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Ask
import com.hyprspace.android.net.Up

class Answers : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext as App
        val thread = intent.getLongExtra(THREAD, -1)
        val request = intent.getStringExtra(REQUEST) ?: return
        val answer = if (intent.getBooleanExtra(ALLOW, false)) Answer.Allow else Answer.Deny
        if (app.link.send(Up.Do(Ask.Approve(thread, request, answer)))) {
            app.notifier.cancel(thread)
        } else {
            app.notifier.unsent(thread)
        }
    }

    companion object {
        const val THREAD = "thread"
        const val REQUEST = "request"
        const val ALLOW = "allow"
    }
}
