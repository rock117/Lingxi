package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class MsiResult(
    val today: MsiDayResult,
    val recent: MsiRecentResult,
    val days: Int,
    @SerialName("calculated_at") val calculatedAt: String,
)

@Serializable
data class MsiDayResult(
    val date: String,
    val value: Double,
    val layers: MsiLayers,
    val components: MsiComponents,
    val signal: MsiSignal,
    val diagnostics: MsiDiagnostics,
)

@Serializable
data class MsiRecentResult(
    val value: Double,
    val days: Int,
    val series: List<MsiDayPoint> = emptyList(),
)

@Serializable
data class MsiDayPoint(
    val date: String,
    val value: Double,
)

@Serializable
data class MsiLayers(
    val direction: Double,
    val intensity: Double,
    val constraint: Double,
)

@Serializable
data class MsiComponents(
    val breadth: Double,
    @SerialName("money_flow") val moneyFlow: Double,
    val structure: Double,
    @SerialName("limit_pressure") val limitPressure: Double,
    @SerialName("intraday_position") val intradayPosition: Double,
)

@Serializable
data class MsiSignal(
    @SerialName("signal_type") val signalType: String,
    val label: String,
    val advice: String,
    @SerialName("volume_status") val volumeStatus: String,
    @SerialName("volume_ratio") val volumeRatio: Double,
    @SerialName("price_status") val priceStatus: String,
    @SerialName("price_position") val pricePosition: Double,
)

@Serializable
data class MsiDiagnostics(
    @SerialName("sample_size") val sampleSize: Int,
    @SerialName("up_count") val upCount: Int,
    @SerialName("down_count") val downCount: Int,
    @SerialName("flat_count") val flatCount: Int,
    @SerialName("limit_up_count") val limitUpCount: Int,
    @SerialName("limit_down_count") val limitDownCount: Int,
    @SerialName("breadth_raw") val breadthRaw: Double = 0.0,
    @SerialName("money_flow_raw") val moneyFlowRaw: Double = 0.0,
    @SerialName("structure_raw") val structureRaw: Double = 0.0,
)
