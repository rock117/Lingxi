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
 * 连接 [ServerConfig.wsUrl]，收到 notification 类型消息后通过 Flow 发射。
 * 断线自动重连（指数退避，最大 30 秒）。
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
        var backoffMs = 1_000L

        fun open() {
            if (stopped.get() || !isActive) return
            val request = Request.Builder().url(config.wsUrl()).build()
            socket = client.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: Response) {
                        backoffMs = 1_000L
                    }

                    override fun onMessage(webSocket: WebSocket, text: String) {
                        runCatching {
                            val msg = json.decodeFromString<WsPushMessage>(text)
                            if (msg.type == "notification" && msg.data != null) {
                                trySend(msg.data)
                            }
                        }
                    }

                    override fun onClosing(
                        webSocket: WebSocket,
                        code: Int,
                        reason: String,
                    ) {
                        webSocket.close(code, reason)
                    }

                    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                        scheduleReconnect()
                    }

                    override fun onFailure(
                        webSocket: WebSocket,
                        t: Throwable,
                        response: Response?,
                    ) {
                        webSocket.cancel()
                        scheduleReconnect()
                    }

                    private fun scheduleReconnect() {
                        if (stopped.get() || !isActive) return
                        val wait = backoffMs
                        backoffMs = (backoffMs * 2).coerceAtMost(30_000L)
                        launch {
                            delay(wait)
                            if (!stopped.get() && isActive) open()
                        }
                    }
                },
            )
        }

        open()

        awaitClose {
            stopped.set(true)
            socket?.close(1000, "client close")
            socket?.cancel()
        }
    }
}
