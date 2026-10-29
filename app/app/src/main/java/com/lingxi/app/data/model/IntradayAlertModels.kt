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
    @SerialName("session_phase") val sessionPhase: String = "",
    val items: List<IntradayAlertItem> = emptyList(),
)

@Serializable
data class AlertLayers(
    val a: Double = 0.0,
    val b: Double = 0.0,
    val c: Double = 0.0,
)

@Serializable
data class IntradayAlertItem(
    val symbol: String,
    val name: String = "",
    val score: Double = 0.0,
    val rs: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    @SerialName("drawdown_pct") val drawdownPct: Double = 0.0,
    val amount: Double = 0.0,
    val layers: AlertLayers? = null,
    val reason: String = "",
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

/** 把后端 reason / 字段整理成可读上榜理由。 */
fun IntradayAlertItem.displayReason(kind: String): String {
    val layers = layers
    val parts = mutableListOf<String>()
    when (kind) {
        "intraday_oversold" -> {
            if (drawdownPct != 0.0) parts += "年高回撤 ${"%+.1f%%".format(drawdownPct)}"
            if (changePct != 0.0) parts += "当日 ${"%+.1f%%".format(changePct)}"
            parseVolFromReason(reason)?.let { parts += "放量 ${it}" }
            layers?.let {
                parts += "分项 回撤${it.a.toInt()}/贴新低${it.b.toInt()}/放量${it.c.toInt()}"
            }
        }
        else -> {
            if (rs != 0.0) parts += "相对大盘 ${"%+.1f%%".format(rs)}"
            if (changePct != 0.0) parts += "当日 ${"%+.1f%%".format(changePct)}"
            dominantLayerLabel(reason, layers)?.let { parts += "主因 $it" }
            layers?.let {
                parts += "分项 强度${it.a.toInt()}/五日${it.b.toInt()}/买点${it.c.toInt()}"
            }
        }
    }
    if (parts.isEmpty() && reason.isNotBlank()) return reason
    return parts.joinToString(" · ")
}

private fun parseVolFromReason(reason: String): String? {
    val m = Regex("""vol=([0-9.]+)x""", RegexOption.IGNORE_CASE).find(reason) ?: return null
    return "${m.groupValues[1]}×"
}

private fun dominantLayerLabel(reason: String, layers: AlertLayers?): String? {
    Regex("""main=(\w+)""").find(reason)?.groupValues?.getOrNull(1)?.let { main ->
        return when (main) {
            "RS" -> "相对强度"
            "5D" -> "五日结构"
            "entry" -> "买点/量价"
            else -> main
        }
    }
    val l = layers ?: return null
    return when {
        l.a >= l.b && l.a >= l.c -> "相对强度"
        l.b >= l.c -> "五日结构"
        else -> "买点/量价"
    }
}
