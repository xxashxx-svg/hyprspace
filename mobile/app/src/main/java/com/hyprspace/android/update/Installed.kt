package com.hyprspace.android.update

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build
import com.hyprspace.android.App
import com.hyprspace.android.service.LinkService

class Installed : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext as App
        if (intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
            LinkService.sync(app)
            return
        }
        when (intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                val confirm = if (Build.VERSION.SDK_INT >= 33) {
                    intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableExtra(Intent.EXTRA_INTENT)
                }
                app.updater.held()
                if (!intent.getBooleanExtra(QUIET, false) && confirm != null) {
                    context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                }
            }
            PackageInstaller.STATUS_SUCCESS -> {}
            else -> app.updater.failed(intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE))
        }
    }

    companion object {
        const val QUIET = "quiet"
    }
}
