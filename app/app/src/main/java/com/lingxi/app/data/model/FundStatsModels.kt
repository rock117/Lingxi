package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class PeriodOption(
    val id: String,
    val label: String,
)

@Serializable
data class FundStatsMeta(
    val periods: List<PeriodOption> = emptyList(),
    @SerialName("default_year") val defaultYear: Int? = null,
    @SerialName("default_period") val defaultPeriod: String = "FY",
    @SerialName("available_years") val availableYears: List<Int> = emptyList(),
)

@Serializable
data class RatioStat(
    val sample: Long = 0,
    val positive: Long = 0,
    @SerialName("non_positive") val nonPositive: Long = 0,
    @SerialName("positive_pct") val positivePct: Double? = null,
    /** 相对上年同口径「面」的百分点差 */
    @SerialName("yoy_delta_pct") val yoyDeltaPct: Double? = null,
    /** 相对上一报告期「面」的百分点差 */
    @SerialName("qoq_delta_pct") val qoqDeltaPct: Double? = null,
)

@Serializable
data class PeriodRef(
    val year: Int,
    val period: String = "",
    @SerialName("period_label") val periodLabel: String = "",
)

@Serializable
data class FaceChangeSummary(
    @SerialName("yoy_up") val yoyUp: Int = 0,
    @SerialName("yoy_down") val yoyDown: Int = 0,
    @SerialName("yoy_flat") val yoyFlat: Int = 0,
    @SerialName("qoq_up") val qoqUp: Int = 0,
    @SerialName("qoq_down") val qoqDown: Int = 0,
    @SerialName("qoq_flat") val qoqFlat: Int = 0,
)

@Serializable
data class FundStatsResult(
    val year: Int,
    val period: String,
    @SerialName("period_label") val periodLabel: String = "",
    @SerialName("available_years") val availableYears: List<Int> = emptyList(),
    @SerialName("yoy_base") val yoyBase: PeriodRef? = null,
    @SerialName("qoq_base") val qoqBase: PeriodRef? = null,
    @SerialName("net_profit") val netProfit: RatioStat = RatioStat(),
    @SerialName("net_profit_yoy") val netProfitYoy: RatioStat = RatioStat(),
    @SerialName("revenue_yoy") val revenueYoy: RatioStat = RatioStat(),
    @SerialName("operating_cf") val operatingCf: RatioStat = RatioStat(),
    val summary: FaceChangeSummary = FaceChangeSummary(),
    val note: String = "",
)
