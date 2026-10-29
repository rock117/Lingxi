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
    "mid_bull_grey" -> "略偏多"
    "mid_bear_confirmed" -> "中期偏空"
    "mid_bear_grey" -> "略偏空"
    else -> "中性"
}

/** 与 MTT/MSI 灰区阈值对齐的短标签（用于双读数副文案）。 */
fun scoreBandLabel(score: Double): String = when {
    score > 65 -> "偏多"
    score >= 60 -> "略偏多"
    score < 35 -> "偏空"
    score <= 40 -> "略偏空"
    else -> "中性"
}

/** 程度说明（展开层 / 因子行用，语义与灰区阈值一致）。 */
fun scoreDegreeLabel(score: Double): String = when {
    score > 65 -> "偏多"
    score >= 60 -> "略偏多"
    score < 35 -> "偏空"
    score <= 40 -> "略偏空"
    else -> "胶着"
}

fun isScoreGrey(score: Double): Boolean =
    (score in 60.0..65.0) || (score in 35.0..40.0)

/**
 * 今日 / 近N日 / MTT 组合结论（口语化盘面描述，非交易指令）。
 * bullish: ≥60；bearish: ≤40；其余中性。短线以「今日」为主，近N日作背景。
 */
fun comboOutlook(today: Double?, recent: Double?, mtt: Double?): String {
    if (today == null && mtt == null) return "暂时还没数据"
    if (today == null) return "中期能看，短线这边暂时空着"
    if (mtt == null) {
        return when {
            recent == null -> "先看短线，中期这边暂时空着"
            today >= 60 && recent < 45 -> "今天偏热，但这几天整体还偏冷"
            today <= 40 && recent > 55 -> "今天偏冷，但这几天整体还偏热"
            else -> "先看短线，中期这边暂时空着"
        }
    }

    val mBull = today >= 60
    val mBear = today <= 40
    val tBull = mtt >= 60
    val tBear = mtt <= 40
    val soft = isScoreGrey(today) || isScoreGrey(mtt)

    val recentNote = when {
        recent == null -> ""
        recent >= 60 && today < 55 -> "，这几天整体偏热"
        recent <= 40 && today > 45 -> "，这几天整体偏冷"
        else -> ""
    }

    val core = when {
        mBull && tBull ->
            if (soft) "今天偏暖，中期也不错"
            else "今天热，中期也偏多"
        mBull && tBear ->
            if (soft) "今天偏热，但中期还是偏空"
            else "今天热闹，中期却还偏空"
        mBear && tBull ->
            if (soft) "中期还偏多，今天先凉了一截"
            else "中期还偏多，今天明显在降温"
        mBear && tBear ->
            if (soft) "今天偏冷，中期也不乐观"
            else "今天冷，中期也偏空"
        tBull -> "中期偏多，今天还在拉锯"
        tBear -> "中期偏空，今天还在拉锯"
        mBull || mBear -> "今天有点脉冲，中期看不出方向"
        else -> "今天和中期都还平静"
    }
    return core + recentNote
}
