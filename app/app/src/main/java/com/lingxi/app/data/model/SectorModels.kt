package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class SectorMoverRow(
    val symbol: String = "",
    val name: String = "",
    @SerialName("sector_type") val sectorType: String = "",
    @SerialName("change_pct") val changePct: Double = 0.0,
    val close: Double = 0.0,
    @SerialName("pre_close") val preClose: Double = 0.0,
    val amount: Double = 0.0,
    val volume: Double = 0.0,
)

@Serializable
data class SectorMovers(
    @SerialName("trade_date") val tradeDate: String = "",
    @SerialName("top_n") val topN: Int = 10,
    val types: List<String> = emptyList(),
    val gainers: List<SectorMoverRow> = emptyList(),
    val losers: List<SectorMoverRow> = emptyList(),
)

@Serializable
data class SectorBar(
    val date: String = "",
    val open: Double = 0.0,
    val high: Double = 0.0,
    val low: Double = 0.0,
    val close: Double = 0.0,
    @SerialName("pre_close") val preClose: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    val amount: Double = 0.0,
    val volume: Double = 0.0,
)

@Serializable
data class SectorDetail(
    val symbol: String = "",
    val name: String = "",
    @SerialName("sector_type") val sectorType: String = "",
    @SerialName("trade_date") val tradeDate: String = "",
    val open: Double = 0.0,
    val high: Double = 0.0,
    val low: Double = 0.0,
    val close: Double = 0.0,
    @SerialName("pre_close") val preClose: Double = 0.0,
    @SerialName("change_pct") val changePct: Double = 0.0,
    val amount: Double = 0.0,
    val volume: Double = 0.0,
    @SerialName("change_5d_pct") val change5dPct: Double? = null,
    @SerialName("change_10d_pct") val change10dPct: Double? = null,
    val bars: List<SectorBar> = emptyList(),
)
