package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class AiStatus(
    @SerialName("ai_enabled") val aiEnabled: Boolean = false,
    val model: String = "",
    @SerialName("max_tool_rounds") val maxToolRounds: Int = 3,
    @SerialName("history_limit") val historyLimit: Int = 12,
)

@Serializable
data class AssistantChatRequest(
    @SerialName("session_id") val sessionId: Long? = null,
    val message: String,
)

@Serializable
data class AssistantStep(
    val id: String = "",
    val phase: String = "",
    val tool: String? = null,
    val label: String = "",
    val ok: Boolean? = null,
    val summary: String? = null,
)

@Serializable
data class AssistantChatResponse(
    @SerialName("session_id") val sessionId: Long = 0,
    val reply: String = "",
    val model: String = "",
    val steps: List<AssistantStep> = emptyList(),
    val degraded: Boolean = false,
    val reason: String? = null,
    val suggestions: List<String> = emptyList(),
)

@Serializable
data class AssistantSession(
    val id: Long,
    val title: String = "",
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
    @SerialName("message_count") val messageCount: Long = 0,
)

@Serializable
data class AssistantMessage(
    val id: Long = 0,
    val role: String,
    val content: String = "",
    @SerialName("tool_name") val toolName: String? = null,
    @SerialName("tool_payload") val toolPayload: JsonElement? = null,
    @SerialName("created_at") val createdAt: String = "",
)

@Serializable
data class AssistantSessionDetail(
    val session: AssistantSession,
    val messages: List<AssistantMessage> = emptyList(),
)

/** 本地 UI 消息（含流式过程）。 */
data class ChatUiMessage(
    val localId: String,
    val role: String,
    val content: String = "",
    val steps: List<AssistantStep> = emptyList(),
    val suggestions: List<String> = emptyList(),
    val streaming: Boolean = false,
    val stepsExpanded: Boolean = false,
)
