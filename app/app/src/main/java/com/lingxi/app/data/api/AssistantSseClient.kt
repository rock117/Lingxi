package com.lingxi.app.data.api

import com.lingxi.app.data.model.AssistantChatRequest
import com.lingxi.app.data.model.AssistantStep
import com.lingxi.app.data.model.ServerConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import okhttp3.Call
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

sealed class AssistantSseEvent {
    data class Session(val sessionId: Long) : AssistantSseEvent()
    data class Step(val step: AssistantStep) : AssistantSseEvent()
    data class Delta(val text: String) : AssistantSseEvent()
    data class Done(
        val sessionId: Long,
        val model: String,
        val steps: List<AssistantStep>,
        val degraded: Boolean,
        val reason: String?,
        val suggestions: List<String>,
    ) : AssistantSseEvent()
    data class Error(val message: String) : AssistantSseEvent()
}

object AssistantSseClient {
    private val json = Json { ignoreUnknownKeys = true; isLenient = true }
    private val client = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS) // SSE 长连接
        .writeTimeout(60, TimeUnit.SECONDS)
        .callTimeout(0, TimeUnit.SECONDS)
        .build()

    private val activeCall = AtomicReference<Call?>(null)

    fun cancel() {
        activeCall.getAndSet(null)?.cancel()
    }

    fun stream(config: ServerConfig, body: AssistantChatRequest): Flow<AssistantSseEvent> =
        callbackFlow {
            val url = config.httpBaseUrl().trimEnd('/') + "/api/ai/chat/stream"
            val payload = json.encodeToString(AssistantChatRequest.serializer(), body)
            val req = Request.Builder()
                .url(url)
                .post(payload.toRequestBody("application/json".toMediaType()))
                .header("Accept", "text/event-stream")
                .build()
            val call = client.newCall(req)
            activeCall.set(call)

            withContext(Dispatchers.IO) {
                try {
                    call.execute().use { resp ->
                        if (!resp.isSuccessful) {
                            val err = resp.body?.string().orEmpty().take(300)
                            trySend(
                                AssistantSseEvent.Error(
                                    "HTTP ${resp.code}${if (err.isNotBlank()) ": $err" else ""}",
                                ),
                            )
                            return@use
                        }
                        val source = resp.body?.source() ?: return@use
                        var eventName = "message"
                        val dataBuf = StringBuilder()
                        while (!source.exhausted()) {
                            val line = source.readUtf8Line() ?: break
                            when {
                                line.startsWith("event:") -> {
                                    eventName = line.removePrefix("event:").trim()
                                }
                                line.startsWith("data:") -> {
                                    if (dataBuf.isNotEmpty()) dataBuf.append('\n')
                                    dataBuf.append(line.removePrefix("data:").trim())
                                }
                                line.isEmpty() -> {
                                    if (dataBuf.isNotEmpty()) {
                                        parseEvent(eventName, dataBuf.toString())?.let {
                                            trySend(it)
                                        }
                                    }
                                    eventName = "message"
                                    dataBuf.clear()
                                }
                            }
                        }
                        if (dataBuf.isNotEmpty()) {
                            parseEvent(eventName, dataBuf.toString())?.let { trySend(it) }
                        }
                    }
                } catch (e: Exception) {
                    if (call.isCanceled()) {
                        trySend(AssistantSseEvent.Error("已停止生成"))
                    } else {
                        trySend(AssistantSseEvent.Error(e.message ?: "连接失败"))
                    }
                } finally {
                    activeCall.compareAndSet(call, null)
                    close()
                }
            }
            awaitClose { call.cancel() }
        }

    private fun parseEvent(name: String, data: String): AssistantSseEvent? {
        return runCatching {
            val el = json.parseToJsonElement(data).jsonObject
            when (name) {
                "session" -> AssistantSseEvent.Session(
                    el["session_id"]?.jsonPrimitive?.longOrNull ?: 0L,
                )
                "step" -> AssistantSseEvent.Step(
                    json.decodeFromJsonElement<AssistantStep>(el),
                )
                "delta" -> AssistantSseEvent.Delta(
                    el["text"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                )
                "done" -> {
                    val steps = el["steps"]?.jsonArray?.mapNotNull { item ->
                        runCatching { json.decodeFromJsonElement<AssistantStep>(item) }.getOrNull()
                    }.orEmpty()
                    val suggestions = el["suggestions"]?.jsonArray?.mapNotNull {
                        it.jsonPrimitive.contentOrNull
                    }.orEmpty()
                    AssistantSseEvent.Done(
                        sessionId = el["session_id"]?.jsonPrimitive?.longOrNull ?: 0L,
                        model = el["model"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                        steps = steps,
                        degraded = el["degraded"]?.jsonPrimitive?.booleanOrNull ?: false,
                        reason = el["reason"]?.jsonPrimitive?.contentOrNull,
                        suggestions = suggestions,
                    )
                }
                "error" -> AssistantSseEvent.Error(
                    el["message"]?.jsonPrimitive?.contentOrNull
                        ?: el["error"]?.jsonPrimitive?.contentOrNull
                        ?: data,
                )
                else -> null
            }
        }.getOrNull()
    }
}
