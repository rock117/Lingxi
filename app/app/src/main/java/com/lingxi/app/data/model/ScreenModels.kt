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

@Serializable
data class StockBar(
    val date: String = "",
    val open: Double = 0.0,
    val high: Double = 0.0,
    val low: Double = 0.0,
    val close: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    val amount: Double = 0.0,
)

@Serializable
data class StockDetail(
    val symbol: String = "",
    val name: String = "",
    val date: String = "",
    val open: Double = 0.0,
    val high: Double = 0.0,
    val low: Double = 0.0,
    val close: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    val amount: Double = 0.0,
    @SerialName("change_5d_pct") val change5dPct: Double? = null,
    @SerialName("change_10d_pct") val change10dPct: Double? = null,
    val bars: List<StockBar> = emptyList(),
)

@Serializable
data class StockSearchHit(
    val symbol: String = "",
    val name: String = "",
    val code: String = "",
    @SerialName("name_pinyin") val namePinyin: String = "",
)

@Serializable
data class StockSearchResult(
    val total: Int = 0,
    val items: List<StockSearchHit> = emptyList(),
)

@Serializable
data class StockAnalyzeQuote(
    val date: String = "",
    val open: Double = 0.0,
    val high: Double = 0.0,
    val low: Double = 0.0,
    val close: Double = 0.0,
    val current: Double = 0.0,
    @SerialName("pre_close") val preClose: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    @SerialName("change_amount") val changeAmount: Double = 0.0,
    val amount: Double = 0.0,
    val volume: Double = 0.0,
    val amplitude: Double = 0.0,
    @SerialName("turnover_rate") val turnoverRate: Double? = null,
)

/** `GET /api/market/stock/analyze` */
@Serializable
data class StockAnalyze(
    val symbol: String = "",
    val name: String = "",
    @SerialName("trade_date") val tradeDate: String = "",
    /** `eastmoney` / `tencent` = 实时拉取（不入库）；`db` = 库内快照 */
    @SerialName("detail_source") val detailSource: String = "",
    /** 明细最新一笔时间，如 `2026-10-24 14:59:57` */
    @SerialName("detail_last_time") val detailLastTime: String? = null,
    @SerialName("detail_tick_count") val detailTickCount: Long = 0,
    @SerialName("detail_updated_at") val detailUpdatedAt: String? = null,
    val quote: StockAnalyzeQuote? = null,
    val bars: List<StockBar> = emptyList(),
    val analysis: String = "",
    @SerialName("ai_enabled") val aiEnabled: Boolean = false,
    @SerialName("ai_model") val aiModel: String? = null,
)
