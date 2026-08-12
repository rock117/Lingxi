package com.lingxi.app.service

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import com.lingxi.app.MainActivity
import com.lingxi.app.R
import com.lingxi.app.data.mock.MockRepository
import com.lingxi.app.notification.NotificationHelper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch

/**
 * 前台服务：保持 App 在后台时仍能接收 Mock 推送并发系统通知。
 *
 * 保活策略：
 * - START_STICKY：系统杀掉后自动重启
 * - onTaskRemoved：用户从最近任务划掉时延迟重启
 * - BootReceiver：手机重启后自动启动
 */
class LingxiService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var collectJob: kotlinx.coroutines.Job? = null

    override fun onCreate() {
        super.onCreate()
        NotificationHelper.ensureChannels(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                return START_NOT_STICKY
            }
        }

        startForeground(NOTIF_ID_FOREGROUND, buildForegroundNotification())

        // 启动 Mock 定时推送
        MockRepository.startAutoPush(scope)

        // 监听新通知 -> 发系统通知 + 更新桌面角标
        if (collectJob?.isActive != true) {
            collectJob = scope.launch {
                MockRepository.newNotification
                    .onEach { item ->
                        NotificationHelper.postAlert(this@LingxiService, item)
                        // 更新桌面图标角标为当前未读数
                        val unread = MockRepository.notifications.value.count { !it.read }
                        NotificationHelper.updateBadge(this@LingxiService, unread)
                    }
                    .launchIn(this)
            }
        }

        return START_STICKY
    }

    /**
     * 用户从最近任务列表划掉 App 时调用。
     * 延迟 1 秒后重启 Service，保持后台运行。
     */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val restartIntent = Intent(applicationContext, LingxiService::class.java).apply {
            setPackage(packageName)
        }
        val pendingIntent = PendingIntent.getService(
            this,
            1,
            restartIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_ONE_SHOT,
        )
        val alarmManager = getSystemService(Context.ALARM_SERVICE) as android.app.AlarmManager
        val triggerAt = SystemClock.elapsedRealtime() + 1000L
        alarmManager.set(
            android.app.AlarmManager.ELAPSED_REALTIME_WAKEUP,
            triggerAt,
            pendingIntent,
        )
        super.onTaskRemoved(rootIntent)
    }

    override fun onDestroy() {
        MockRepository.stopAutoPush()
        collectJob?.cancel()
        scope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun buildForegroundNotification(): Notification {
        val tapIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val pendingTap = PendingIntent.getActivity(
            this,
            0,
            tapIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        return NotificationCompat.Builder(this, NotificationHelper.FOREGROUND_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("灵犀 · 后台监听中")
            .setContentText("正在监听条件触发")
            .setOngoing(true)
            .setContentIntent(pendingTap)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    companion object {
        private const val NOTIF_ID_FOREGROUND = 1
        private const val ACTION_STOP = "com.lingxi.app.STOP_SERVICE"

        fun start(context: Context) {
            val intent = Intent(context, LingxiService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            val intent = Intent(context, LingxiService::class.java).apply {
                action = ACTION_STOP
            }
            context.startService(intent)
        }
    }
}
