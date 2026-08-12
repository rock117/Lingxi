package com.lingxi.app.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * 开机自启动：手机重启后自动启动后台监听服务
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED ||
            intent.action == "android.intent.action.QUICKBOOT_POWERON"
        ) {
            LingxiService.start(context)
        }
    }
}
