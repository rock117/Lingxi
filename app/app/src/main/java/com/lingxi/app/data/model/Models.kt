package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class Condition(
    val id: Long,
    val name: String,
    val kind: String,
    val symbol: String? = null,
    val expression: String,
    val enabled: Boolean,
    @SerialName("created_at") val createdAt: String,
    @SerialName("updated_at") val updatedAt: String,
)

@Serializable
data class CreateConditionRequest(
    val name: String,
    val kind: String,
    val symbol: String? = null,
    val expression: String,
    val enabled: Boolean = true,
)

@Serializable
data class UpdateConditionRequest(
    val name: String? = null,
    val kind: String? = null,
    val symbol: String? = null,
    val expression: String? = null,
    val enabled: Boolean? = null,
)

@Serializable
data class NotificationItem(
    val id: Long,
    @SerialName("condition_id") val conditionId: Long,
    val title: String,
    val body: String,
    val payload: String? = null,
    val read: Boolean,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class HealthResponse(
    val status: String = "ok",
)

@Serializable
data class WsPushMessage(
    val type: String,
    val data: NotificationItem? = null,
)

data class ServerConfig(
    val host: String = "10.0.2.2",
    val httpPort: Int = 8000,
    val wsPort: Int = 8000,
    val useTls: Boolean = false,
    val wsPath: String = "/ws",
) {
    fun httpBaseUrl(): String {
        val scheme = if (useTls) "https" else "http"
        return "$scheme://$host:$httpPort/"
    }

    fun wsUrl(): String {
        val scheme = if (useTls) "wss" else "ws"
        val path = if (wsPath.startsWith("/")) wsPath else "/$wsPath"
        return "$scheme://$host:$wsPort$path"
    }
}
