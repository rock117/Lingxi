package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class IntradayAlertPayload(
    val kind: String = "",
    val date: String = "",
    @SerialName("r_mkt") val rMkt: Double = 0.0,
    @SerialName("sample_size") val sampleSize: Int = 0,
    @SerialName("candidate_size") val candidateSize: Int = 0,
    val items: List<IntradayAlertItem> = emptyList(),
)

@Serializable
data class IntradayAlertItem(
    val symbol: String,
    val name: String = "",
    val score: Double = 0.0,
    val rs: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    @SerialName("drawdown_pct") val drawdownPct: Double = 0.0,
)

private val payloadJson = Json { ignoreUnknownKeys = true; isLenient = true }

private val knownAlertKinds = setOf("intraday_alert", "intraday_oversold")

fun NotificationItem.parseIntradayAlert(): IntradayAlertPayload? {
    val raw = payload?.trim().orEmpty()
    if (raw.isEmpty()) return null
    return runCatching { payloadJson.decodeFromString<IntradayAlertPayload>(raw) }
        .getOrNull()
        ?.takeIf { it.kind in knownAlertKinds || it.items.isNotEmpty() }
}
