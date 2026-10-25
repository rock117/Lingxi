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

@Serializable
data class DimAi(
    /** pending | ready | disabled | error */
    val status: String = "pending",
    val text: String = "",
    /** bullish | neutral | bearish | unknown */
    val bias: String = "unknown",
    /** high | medium | low | unknown */
    val confidence: String = "unknown",
    val flags: List<String> = emptyList(),
)

@Serializable
data class DetailSummary(
    @SerialName("tick_count") val tickCount: Int = 0,
    @SerialName("buy_lots") val buyLots: Double = 0.0,
    @SerialName("sell_lots") val sellLots: Double = 0.0,
    @SerialName("neutral_lots") val neutralLots: Double = 0.0,
    @SerialName("buy_ratio") val buyRatio: Double? = null,
    @SerialName("sell_ratio") val sellRatio: Double? = null,
    @SerialName("vwap_est") val vwapEst: Double? = null,
    @SerialName("price_vs_vwap_pct") val priceVsVwapPct: Double? = null,
    @SerialName("large_buy_lots") val largeBuyLots: Double = 0.0,
    @SerialName("large_sell_lots") val largeSellLots: Double = 0.0,
    @SerialName("large_net_lots") val largeNetLots: Double = 0.0,
    @SerialName("open_buy_lots") val openBuyLots: Double = 0.0,
    @SerialName("open_sell_lots") val openSellLots: Double = 0.0,
    @SerialName("close_buy_lots") val closeBuyLots: Double = 0.0,
    @SerialName("close_sell_lots") val closeSellLots: Double = 0.0,
    @SerialName("large_buy_follow_pct") val largeBuyFollowPct: Double? = null,
    @SerialName("large_sell_follow_pct") val largeSellFollowPct: Double? = null,
    val labels: List<String> = emptyList(),
)

@Serializable
data class ShortTermDim(
    val bars: List<StockBar> = emptyList(),
    @SerialName("change_5d_pct") val change5dPct: Double? = null,
    @SerialName("change_10d_pct") val change10dPct: Double? = null,
    @SerialName("change_20d_pct") val change20dPct: Double? = null,
    val ma5: Double? = null,
    val ma10: Double? = null,
    val ma20: Double? = null,
    @SerialName("amount_vs_5d_avg") val amountVs5dAvg: Double? = null,
    @SerialName("price_vs_ma5_pct") val priceVsMa5Pct: Double? = null,
    @SerialName("price_vs_ma10_pct") val priceVsMa10Pct: Double? = null,
    @SerialName("price_vs_ma20_pct") val priceVsMa20Pct: Double? = null,
    @SerialName("ma_alignment") val maAlignment: String = "unknown",
    @SerialName("near_high_20d") val nearHigh20d: Double? = null,
    @SerialName("near_low_20d") val nearLow20d: Double? = null,
    @SerialName("dist_to_high_pct") val distToHighPct: Double? = null,
    @SerialName("dist_to_low_pct") val distToLowPct: Double? = null,
    @SerialName("volume_label") val volumeLabel: String = "未知",
    val labels: List<String> = emptyList(),
    val ai: DimAi = DimAi(),
)

@Serializable
data class FundamentalsDim(
    @SerialName("report_date") val reportDate: String? = null,
    @SerialName("fiscal_period") val fiscalPeriod: String? = null,
    val pe: Double? = null,
    val pb: Double? = null,
    @SerialName("float_mv") val floatMv: Double? = null,
    @SerialName("total_mv") val totalMv: Double? = null,
    val revenue: Double? = null,
    @SerialName("revenue_yoy") val revenueYoy: Double? = null,
    @SerialName("parent_np") val parentNp: Double? = null,
    @SerialName("parent_np_yoy") val parentNpYoy: Double? = null,
    @SerialName("deduct_np") val deductNp: Double? = null,
    @SerialName("deduct_np_yoy") val deductNpYoy: Double? = null,
    @SerialName("operating_cf") val operatingCf: Double? = null,
    @SerialName("operating_cf_yoy") val operatingCfYoy: Double? = null,
    val receivable: Double? = null,
    @SerialName("receivable_yoy") val receivableYoy: Double? = null,
    @SerialName("total_assets") val totalAssets: Double? = null,
    @SerialName("total_liabilities") val totalLiabilities: Double? = null,
    @SerialName("total_equity") val totalEquity: Double? = null,
    @SerialName("ocf_vs_np") val ocfVsNp: Double? = null,
    @SerialName("liability_ratio") val liabilityRatio: Double? = null,
    @SerialName("quality_flags") val qualityFlags: List<String> = emptyList(),
    val ai: DimAi = DimAi(),
)

@Serializable
data class IncomePeriodRow(
    @SerialName("report_date") val reportDate: String = "",
    @SerialName("fiscal_period") val fiscalPeriod: String? = null,
    val revenue: Double? = null,
    @SerialName("parent_np") val parentNp: Double? = null,
    @SerialName("deduct_np") val deductNp: Double? = null,
    val rd: Double? = null,
)

@Serializable
data class CashflowPeriodRow(
    @SerialName("report_date") val reportDate: String = "",
    @SerialName("fiscal_period") val fiscalPeriod: String? = null,
    val operating: Double? = null,
    val investing: Double? = null,
    val financing: Double? = null,
)

@Serializable
data class BalancePeriodRow(
    @SerialName("report_date") val reportDate: String = "",
    @SerialName("fiscal_period") val fiscalPeriod: String? = null,
    @SerialName("total_assets") val totalAssets: Double? = null,
    @SerialName("total_liabilities") val totalLiabilities: Double? = null,
    @SerialName("total_equity") val totalEquity: Double? = null,
    val receivable: Double? = null,
)

@Serializable
data class StatementsDim(
    val income: List<IncomePeriodRow> = emptyList(),
    val cashflow: List<CashflowPeriodRow> = emptyList(),
    val balance: List<BalancePeriodRow> = emptyList(),
    @SerialName("trend_flags") val trendFlags: List<String> = emptyList(),
    val ai: DimAi = DimAi(),
)

/** `GET /api/market/stock/analyze` — 事实；AI 默认 pending */
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
    @SerialName("detail_summary") val detailSummary: DetailSummary = DetailSummary(),
    /** 与 ai.text 同步（兼容） */
    val analysis: String = "",
    val ai: DimAi = DimAi(),
    @SerialName("short_term") val shortTerm: ShortTermDim = ShortTermDim(),
    val fundamentals: FundamentalsDim = FundamentalsDim(),
    val statements: StatementsDim = StatementsDim(),
    @SerialName("ai_enabled") val aiEnabled: Boolean = false,
    @SerialName("ai_model") val aiModel: String? = null,
)

/** `GET /api/market/stock/analyze/ai` */
@Serializable
data class StockAnalyzeAi(
    val symbol: String = "",
    @SerialName("trade_date") val tradeDate: String = "",
    val dim: String = "",
    val ai: DimAi = DimAi(),
    @SerialName("ai_enabled") val aiEnabled: Boolean = false,
    @SerialName("ai_model") val aiModel: String? = null,
    val cached: Boolean = false,
)
