package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class MsiResult(
    val value: Double,
    val layers: MsiLayers,
    val components: MsiComponents,
    val days: Int,
    val signal: MsiSignal,
    val diagnostics: MsiDiagnostics,
    @SerialName("calculated_at") val calculatedAt: String,
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
    @SerialName("gap_breadth") val gapBreadth: Double,
    val structure: Double,
    @SerialName("nh_nl") val nhNl: Double,
    @SerialName("directed_volume") val directedVolume: Double,
    @SerialName("volume_ratio") val volumeRatio: Double,
    @SerialName("limit_pressure") val limitPressure: Double,
    @SerialName("range_position") val rangePosition: Double,
    @SerialName("intraday_position") val intradayPosition: Double,
    val inertia: Double,
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
    @SerialName("breadth_last") val breadthLast: Double = 0.0,
    @SerialName("money_flow_last") val moneyFlowLast: Double = 0.0,
    @SerialName("gap_breadth_last") val gapBreadthLast: Double = 0.0,
    @SerialName("structure_last") val structureLast: Double = 0.0,
    @SerialName("nh_nl_last") val nhNlLast: Double = 0.0,
    @SerialName("new_high_count") val newHighCount: Int = 0,
    @SerialName("new_low_count") val newLowCount: Int = 0,
    @SerialName("avg_up_streak") val avgUpStreak: Double = 0.0,
    @SerialName("avg_down_streak") val avgDownStreak: Double = 0.0,
    @SerialName("avg_no_up_streak") val avgNoUpStreak: Double = 0.0,
    @SerialName("avg_no_down_streak") val avgNoDownStreak: Double = 0.0,
)
