package com.lingxi.app.notification

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.net.Uri
import android.provider.Settings
import androidx.core.app.NotificationCompat
import com.lingxi.app.MainActivity
import com.lingxi.app.R
import com.lingxi.app.data.model.NotificationItem
import me.leolin.shortcutbadger.ShortcutBadger

/**
 * 系统通知栏助手：管理通知渠道 + 发送通知 + 桌面图标角标
 */
object NotificationHelper {

    const val CHANNEL_ID = "lingxi_alerts"
    const val CHANNEL_NAME = "条件提醒"
    const val FOREGROUND_CHANNEL_ID = "lingxi_service"
    const val FOREGROUND_CHANNEL_NAME = "后台监听"

    /** 注册通知渠道（API 26+ 必需），应在 Application/Activity 启动时调用 */
    fun ensureChannels(context: Context) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        nm.createNotificationChannel(
            NotificationChannel(
                FOREGROUND_CHANNEL_ID,
                FOREGROUND_CHANNEL_NAME,
                NotificationManager.IMPORTANCE_LOW,
            ).apply { description = "保持 WebSocket 后台连接" },
        )

        nm.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_HIGH,
            ).apply {
                description = "条件满足时弹出提醒"
                enableVibration(true)
                vibrationPattern = longArrayOf(0, 300, 200, 300)
                enableLights(true)
                // 系统默认通知提示音
                setSound(
                    Settings.System.DEFAULT_NOTIFICATION_URI,
                    AudioAttributes.Builder()
                        .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                        .setUsage(AudioAttributes.USAGE_NOTIFICATION)
                        .build(),
                )
            },
        )
    }

    /** 发送一条条件提醒通知到系统通知栏 */
    fun postAlert(context: Context, item: NotificationItem) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        val tapIntent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val pendingTap = PendingIntent.getActivity(
            context,
            item.id.toInt(),
            tapIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val notification = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(item.title)
            .setContentText(item.body)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .setContentIntent(pendingTap)
            .setDefaults(NotificationCompat.DEFAULT_ALL)
            .setSound(Settings.System.DEFAULT_NOTIFICATION_URI)
            .setVibrate(longArrayOf(0, 300, 200, 300))
            .build()

        nm.notify(item.id.toInt(), notification)
    }

    /** 取消单条通知 */
    fun cancel(context: Context, id: Long) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.cancel(id.toInt())
    }

    /** 取消所有提醒通知 */
    fun cancelAll(context: Context) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.cancelAll()
    }

    // ---- 桌面图标角标（微信式未读数） ----

    /**
     * 更新桌面图标上的未读数字角标。
     * @param unreadCount 未读消息数；为 0 时清除角标
     */
    fun updateBadge(context: Context, unreadCount: Int) {
        if (unreadCount <= 0) {
            ShortcutBadger.removeCount(context)
        } else {
            val count = unreadCount.coerceAtMost(99)
            ShortcutBadger.applyCount(context, count)
        }
    }

    /** 清除桌面图标角标 */
    fun clearBadge(context: Context) {
        ShortcutBadger.removeCount(context)
    }
}
