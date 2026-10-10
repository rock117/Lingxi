package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class MttResult(
    val value: Double,
    val status: String,
    @SerialName("trend_raw") val trendRaw: Double,
    @SerialName("ma_type") val maType: String,
    @SerialName("ma_mid") val maMid: Int,
    @SerialName("ma_long") val maLong: Int,
    @SerialName("use_slope_filter") val useSlopeFilter: Boolean,
    val counts: MttCounts,
    val diagnostics: MttDiagnostics,
    @SerialName("calculated_at") val calculatedAt: String,
)

@Serializable
data class MttCounts(
    val bull: Int,
    val bear: Int,
    val neutral: Int,
    val classified: Int,
)

@Serializable
data class MttDiagnostics(
    @SerialName("deviation_median") val deviationMedian: Double,
    @SerialName("nh_nl") val nhNl: Double,
)

fun mttStatusLabel(status: String): String = when (status) {
    "mid_bull_confirmed" -> "中期偏多"
    "mid_bull_grey" -> "偏多灰区"
    "mid_bear_confirmed" -> "中期偏空"
    "mid_bear_grey" -> "偏空灰区"
    else -> "中性"
}

/** 与 MTT/MSI 灰区阈值对齐的短标签（用于双读数副文案）。 */
fun scoreBandLabel(score: Double): String = when {
    score > 65 -> "偏多确认"
    score >= 60 -> "偏多灰区"
    score < 35 -> "偏空确认"
    score <= 40 -> "偏空灰区"
    else -> "中性"
}

fun isScoreGrey(score: Double): Boolean =
    (score in 60.0..65.0) || (score in 35.0..40.0)

/**
 * MSI × MTT 组合结论（产品文案，非交易指令）。
 * bullish: ≥60；bearish: ≤40；其余中性。
 */
fun comboOutlook(msi: Double?, mtt: Double?): String {
    if (msi == null && mtt == null) return "暂无数据"
    if (msi == null) return "中期结构参考，短线暂不可用"
    if (mtt == null) return "短线情绪参考，中期暂不可用"

    val mBull = msi >= 60
    val mBear = msi <= 40
    val tBull = mtt >= 60
    val tBear = mtt <= 40
    val soft = isScoreGrey(msi) || isScoreGrey(mtt)

    return when {
        mBull && tBull ->
            if (soft) "倾向顺势升温：短线偏热，中期结构也偏多"
            else "顺势升温：短线热，中期结构也偏多"
        mBull && tBear ->
            if (soft) "倾向逆势热闹：短线偏热，中期结构仍偏空"
            else "逆势热闹：短线热，中期结构仍偏空"
        mBear && tBull ->
            if (soft) "倾向趋势回调：中期仍偏多，短线在降温"
            else "趋势回调：中期仍偏多，短线在降温"
        mBear && tBear ->
            if (soft) "倾向顺势走弱：短线偏冷，中期结构也偏空"
            else "顺势走弱：短线冷，中期结构也偏空"
        tBull -> "中期偏多，短线多空胶着"
        tBear -> "中期偏空，短线多空胶着"
        mBull || mBear -> "短线有脉冲，中期尚无结构"
        else -> "短线与中期均处中性"
    }
}
