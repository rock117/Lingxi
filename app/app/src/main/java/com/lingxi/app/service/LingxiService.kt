package com.lingxi.app.service

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import com.lingxi.app.MainActivity
import com.lingxi.app.R
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.prefs.ServerPrefs
import com.lingxi.app.data.repo.NotificationStore
import com.lingxi.app.data.ws.WsClient
import com.lingxi.app.notification.NotificationHelper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch

/**
 * 前台服务：保持 WebSocket 连接，接收后端日内强势等推送。
 */
class LingxiService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var collectJob: kotlinx.coroutines.Job? = null
    private var wsJob: kotlinx.coroutines.Job? = null
    private val wsClient = WsClient()
    private lateinit var prefs: ServerPrefs

    override fun onCreate() {
        super.onCreate()
        prefs = ServerPrefs(this)
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

        if (collectJob?.isActive != true) {
            collectJob = scope.launch {
                NotificationStore.newNotification
                    .onEach { item ->
                        NotificationHelper.postAlert(this@LingxiService, item)
                        val unread = NotificationStore.notifications.value.count { !it.read }
                        NotificationHelper.updateBadge(this@LingxiService, unread)
                    }
                    .launchIn(this)
            }
        }

        if (wsJob?.isActive != true) {
            wsJob = scope.launch {
                prefs.configFlow.collectLatest { cfg ->
                    launch {
                        runCatching { NotificationStore.refresh(ApiClient.create(cfg)) }
                    }
                    wsClient.connect(cfg).collect { item ->
                        NotificationStore.ingest(item)
                    }
                }
            }
        }

        return START_STICKY
    }

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
        collectJob?.cancel()
        wsJob?.cancel()
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
            .setContentText("WebSocket 等待日内强势推送")
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
