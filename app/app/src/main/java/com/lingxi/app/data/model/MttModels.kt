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

/** 程度说明（展开层 / 因子行用，语义与灰区阈值一致）。 */
fun scoreDegreeLabel(score: Double): String = when {
    score > 65 -> "明显偏多"
    score >= 60 -> "略偏多，未确认"
    score < 35 -> "明显偏空"
    score <= 40 -> "略偏空，未确认"
    else -> "多空胶着"
}

fun isScoreGrey(score: Double): Boolean =
    (score in 60.0..65.0) || (score in 35.0..40.0)

/**
 * 今日 / 近N日 / MTT 组合结论（产品文案，非交易指令）。
 * bullish: ≥60；bearish: ≤40；其余中性。短线以「今日」为主，近N日作背景。
 */
fun comboOutlook(today: Double?, recent: Double?, mtt: Double?): String {
    if (today == null && mtt == null) return "暂无数据"
    if (today == null) return "中期结构参考，短线暂不可用"
    if (mtt == null) {
        return when {
            recent == null -> "短线情绪参考，中期暂不可用"
            today >= 60 && recent < 45 -> "今日偏热，近几日仍偏冷"
            today <= 40 && recent > 55 -> "今日偏冷，近几日仍偏热"
            else -> "短线情绪参考，中期暂不可用"
        }
    }

    val mBull = today >= 60
    val mBear = today <= 40
    val tBull = mtt >= 60
    val tBear = mtt <= 40
    val soft = isScoreGrey(today) || isScoreGrey(mtt)

    val recentNote = when {
        recent == null -> ""
        recent >= 60 && today < 55 -> "；近几日偏热"
        recent <= 40 && today > 45 -> "；近几日偏冷"
        else -> ""
    }

    val core = when {
        mBull && tBull ->
            if (soft) "倾向顺势升温：今日偏热，中期结构也偏多"
            else "顺势升温：今日热，中期结构也偏多"
        mBull && tBear ->
            if (soft) "倾向逆势热闹：今日偏热，中期结构仍偏空"
            else "逆势热闹：今日热，中期结构仍偏空"
        mBear && tBull ->
            if (soft) "倾向趋势回调：中期仍偏多，今日在降温"
            else "趋势回调：中期仍偏多，今日在降温"
        mBear && tBear ->
            if (soft) "倾向顺势走弱：今日偏冷，中期结构也偏空"
            else "顺势走弱：今日冷，中期结构也偏空"
        tBull -> "中期偏多，今日多空胶着"
        tBear -> "中期偏空，今日多空胶着"
        mBull || mBear -> "今日有脉冲，中期尚无结构"
        else -> "今日与中期均处中性"
    }
    return core + recentNote
}
