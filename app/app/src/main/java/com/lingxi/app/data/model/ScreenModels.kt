package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class StrategyEnumOption(
    val value: String,
    val label: String,
)

@Serializable
data class StrategyParamDef(
    val key: String,
    val label: String,
    val kind: String,
    val default: JsonElement,
    val min: Double? = null,
    val max: Double? = null,
    val options: List<StrategyEnumOption>? = null,
)

@Serializable
data class StrategyDef(
    @SerialName("type_id") val typeId: String,
    val name: String,
    val summary: String,
    val params: List<StrategyParamDef> = emptyList(),
    @SerialName("default_rule") val defaultRule: JsonElement,
)

@Serializable
data class ScreenMatch(
    val symbol: String,
    val name: String = "",
    @SerialName("change_pct") val changePct: Double = 0.0,
    val amount: Double = 0.0,
    @SerialName("close_price") val closePrice: Double = 0.0,
    @SerialName("hit_rules") val hitRules: List<String> = emptyList(),
)

@Serializable
data class ScreenResult(
    val date: String = "",
    @SerialName("sample_size") val sampleSize: Int = 0,
    @SerialName("match_count") val matchCount: Int = 0,
    val items: List<ScreenMatch> = emptyList(),
)

@Serializable
data class ScreenRequest(
    val expression: JsonElement,
    @SerialName("top_n") val topN: Int = 50,
)
