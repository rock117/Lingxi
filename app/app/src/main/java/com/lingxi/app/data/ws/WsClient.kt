package com.lingxi.app.data.ws

import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.model.ServerConfig
import com.lingxi.app.data.model.WsPushMessage
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * WebSocket 推送客户端。
 *
 * TODO: UI 当前用 MockRepository.pushMockNotification() 模拟推送，尚未连接 /ws。
 *  后续：在应用层 observe [connect]，将收到的 NotificationItem 写入通知列表。
 */
class WsClient {
    private val json = Json { ignoreUnknownKeys = true; isLenient = true }
    private val client = OkHttpClient.Builder()
        .pingInterval(30, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .build()

    fun connect(config: ServerConfig): Flow<NotificationItem> = callbackFlow {
        val stopped = AtomicBoolean(false)
        var socket: WebSocket? = null

        fun open() {
            if (stopped.get() || !isActive) return
            val request = Request.Builder().url(config.wsUrl()).build()
            socket = client.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onMessage(webSocket: WebSocket, text: String) {
                        runCatching {
                            val msg = json.decodeFromString<WsPushMessage>(text)
                            if (msg.type == "notification" && msg.data != null) {
                                trySend(msg.data)
                            }
                        }
                    }

                    override fun onFailure(
                        webSocket: WebSocket,
                        t: Throwable,
                        response: Response?,
                    ) {
                        webSocket.cancel()
                    }
                },
            )
        }

        open()

        // TODO: 改为指数退避 + 仅在连接断开时重连，避免周期性强制重建
        val reconnectJob = launch {
            while (isActive && !stopped.get()) {
                delay(5_000)
                if (stopped.get()) break
                socket?.cancel()
                open()
            }
        }

        awaitClose {
            stopped.set(true)
            reconnectJob.cancel()
            socket?.close(1000, "client close")
            socket?.cancel()
        }
    }
}
