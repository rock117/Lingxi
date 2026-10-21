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
)

@Serializable
data class FundStatsResult(
    val year: Int,
    val period: String,
    @SerialName("period_label") val periodLabel: String = "",
    @SerialName("available_years") val availableYears: List<Int> = emptyList(),
    @SerialName("net_profit") val netProfit: RatioStat = RatioStat(),
    @SerialName("revenue_yoy") val revenueYoy: RatioStat = RatioStat(),
    @SerialName("operating_cf") val operatingCf: RatioStat = RatioStat(),
    val note: String = "",
)
