package com.lingxi.app.market

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.MsiComponents
import com.lingxi.app.data.model.MsiDiagnostics
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.comboOutlook
import com.lingxi.app.data.model.mttStatusLabel
import com.lingxi.app.data.model.scoreBandLabel
import com.lingxi.app.data.model.scoreDegreeLabel
import com.lingxi.app.viewmodel.MarketViewModel
import kotlin.math.roundToInt

private enum class MsiLayerKind {
    Direction,
    Intensity,
    Constraint,
}

@Composable
fun SentimentScreen(viewModel: MarketViewModel) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val hasData = ui.result != null || ui.mtt != null

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    "短线情绪 · 中期结构",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    buildString {
                        append("每 ${ui.refreshIntervalMinutes} 分钟自动刷新")
                        ui.lastSuccessAt?.let { append(" · 成功 $it") }
                    },
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = { viewModel.refresh() }, enabled = !ui.loading) {
                Icon(Icons.Default.Refresh, contentDescription = "刷新")
            }
        }

        if (ui.loading && !hasData) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        } else if (ui.error != null && !hasData) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(24.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text("加载失败", style = MaterialTheme.typography.titleMedium)
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    ui.error ?: "",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    ui.serverHint,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(16.dp))
                TextButton(onClick = { viewModel.refresh() }) {
                    Text("重试")
                }
            }
        } else if (hasData) {
            if (ui.loading) {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            }
            if (ui.error != null) {
                Text(
                    "刷新失败：${ui.error}",
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
            }
            SentimentContent(msi = ui.result, mtt = ui.mtt)
        }
    }
}

@Composable
private fun SentimentContent(msi: MsiResult?, mtt: MttResult?) {
    var expanded by remember { mutableStateOf<MsiLayerKind?>(null) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        DualScoreHeader(msi = msi, mtt = mtt)

        if (mtt != null) {
            SectionTitle("中期 MTT")
            LayerBar("中期趋势", mtt.value)
            Text(
                "多 ${mtt.counts.bull} · 空 ${mtt.counts.bear} · 纠缠 ${mtt.counts.neutral}",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "MA${mtt.maMid}/${mtt.maLong} · ${mtt.maType.uppercase()}" +
                    if (mtt.useSlopeFilter) " · 斜率过滤" else "",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        if (msi != null) {
            SectionTitle("短线三层")
            Text(
                "点击一层查看内部因子与含义",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            ExpandableMsiLayer(
                title = "方向",
                subtitle = "市场站哪边 · 约占 MSI 一半",
                score = msi.layers.direction,
                summary = layerSummary(MsiLayerKind.Direction, msi.layers.direction),
                factors = directionFactors(msi.components, msi.diagnostics),
                expanded = expanded == MsiLayerKind.Direction,
                onToggle = {
                    expanded = if (expanded == MsiLayerKind.Direction) null else MsiLayerKind.Direction
                },
            )
            ExpandableMsiLayer(
                title = "强度",
                subtitle = "打得有多用力 · 约占四分之一",
                score = msi.layers.intensity,
                summary = layerSummary(MsiLayerKind.Intensity, msi.layers.intensity),
                factors = intensityFactors(msi.components),
                expanded = expanded == MsiLayerKind.Intensity,
                onToggle = {
                    expanded = if (expanded == MsiLayerKind.Intensity) null else MsiLayerKind.Intensity
                },
            )
            ExpandableMsiLayer(
                title = "约束",
                subtitle = "位置与惯性 · 约占四分之一（高分偏多，非扣分）",
                score = msi.layers.constraint,
                summary = layerSummary(MsiLayerKind.Constraint, msi.layers.constraint),
                factors = constraintFactors(msi.components, msi.diagnostics),
                expanded = expanded == MsiLayerKind.Constraint,
                onToggle = {
                    expanded =
                        if (expanded == MsiLayerKind.Constraint) null else MsiLayerKind.Constraint
                },
            )

            SectionTitle("信号上下文")
            Text(
                "量能 ${msi.signal.volumeStatus} · 倍率 ${"%.2f".format(msi.signal.volumeRatio)}",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "位置 ${msi.signal.priceStatus} · ${"%.0f".format(msi.signal.pricePosition * 100)}%",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "${msi.signal.label} · ${msi.signal.advice}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            SectionTitle("诊断")
            val d = msi.diagnostics
            Text(
                "样本 ${d.sampleSize} · 涨 ${d.upCount} / 跌 ${d.downCount} / 平 ${d.flatCount}",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "涨停 ${d.limitUpCount} · 跌停 ${d.limitDownCount} · 新高 ${d.newHighCount} · 新低 ${d.newLowCount}",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "量能倍率 ${"%.2f".format(msi.components.volumeRatio)}",
                style = MaterialTheme.typography.bodyMedium,
            )
            if (mtt != null) {
                Text(
                    "中期偏离中位 ${"%.1f".format(mtt.diagnostics.deviationMedian * 100)}%" +
                        " · 20日新高新低 ${"%.1f".format(mtt.diagnostics.nhNl)}",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}

private data class FactorRow(
    val name: String,
    val score: Double,
    val meaning: String,
    val detail: String? = null,
)

@Composable
private fun ExpandableMsiLayer(
    title: String,
    subtitle: String,
    score: Double,
    summary: String,
    factors: List<FactorRow>,
    expanded: Boolean,
    onToggle: () -> Unit,
) {
    val tone = msiTone(score)
    val shape = RoundedCornerShape(10.dp)
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f))
            .clickable(onClick = onToggle)
            .padding(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
                Text(
                    subtitle,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    "%.1f".format(score),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium,
                    color = tone,
                )
                Text(
                    scoreDegreeLabel(score),
                    style = MaterialTheme.typography.labelSmall,
                    color = tone,
                )
            }
            Icon(
                imageVector = if (expanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                contentDescription = if (expanded) "收起" else "展开",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(modifier = Modifier.height(8.dp))
        ScoreProgress(score)
        if (expanded) {
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                summary,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            val extremal = factorExtremes(factors)
            if (extremal != null) {
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    extremal,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            factors.forEach { factor ->
                FactorDetailRow(factor)
                Spacer(modifier = Modifier.height(8.dp))
            }
        }
    }
}

@Composable
private fun FactorDetailRow(factor: FactorRow) {
    val tone = msiTone(factor.score)
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.55f))
            .padding(10.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(factor.name, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
            Text(
                "${"%.1f".format(factor.score)} · ${scoreDegreeLabel(factor.score)}",
                style = MaterialTheme.typography.bodySmall,
                color = tone,
            )
        }
        Spacer(modifier = Modifier.height(2.dp))
        Text(
            factor.meaning,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (factor.detail != null) {
            Text(
                factor.detail,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(modifier = Modifier.height(4.dp))
        ScoreProgress(factor.score, height = 4.dp)
    }
}

@Composable
private fun ScoreProgress(score: Double, height: Dp = 8.dp) {
    LinearProgressIndicator(
        progress = { (score / 100.0).toFloat().coerceIn(0f, 1f) },
        modifier = Modifier
            .fillMaxWidth()
            .height(height)
            .clip(RoundedCornerShape(4.dp)),
        color = msiTone(score),
        trackColor = MaterialTheme.colorScheme.surfaceVariant,
        strokeCap = StrokeCap.Butt,
        gapSize = 0.dp,
        drawStopIndicator = {},
    )
}

private fun layerSummary(kind: MsiLayerKind, score: Double): String {
    val degree = scoreDegreeLabel(score)
    return when (kind) {
        MsiLayerKind.Direction -> when {
            score > 65 -> "多数维度同向偏多（家数、资金、缺口、结构、新高新低）。$degree。"
            score < 35 -> "多数维度同向偏空。$degree。"
            score >= 60 -> "略偏多头一侧，但尚未形成一致确认。$degree。"
            score <= 40 -> "略偏空头一侧，但尚未形成一致确认。$degree。"
            else -> "多空维度互相抵消或信息不足。$degree。"
        }
        MsiLayerKind.Intensity -> when {
            score > 65 -> "上涨侧参与更用力（有方向量能 / 涨停压力占优）。$degree。"
            score < 35 -> "下跌侧更用力，或放量配合空头宽度。$degree。"
            else -> "力度大致均衡，或量能与涨跌停都不极端。$degree。"
        }
        MsiLayerKind.Constraint -> when {
            score > 65 -> "价格偏高位或连涨/未跌惯性偏强（高分表示偏多位置与惯性，不是风险扣分）。$degree。"
            score < 35 -> "价格偏低位或连跌/未涨惯性偏强。$degree。"
            else -> "位置与惯性接近中性。$degree。"
        }
    }
}

private fun factorExtremes(factors: List<FactorRow>): String? {
    if (factors.size < 2) return null
    val strongest = factors.maxBy { it.score }
    val weakest = factors.minBy { it.score }
    if (strongest.name == weakest.name) return null
    return "相对最强：${strongest.name}（${"%.0f".format(strongest.score)}）；相对最弱：${weakest.name}（${"%.0f".format(weakest.score)}）"
}

private fun directionFactors(c: MsiComponents, d: MsiDiagnostics): List<FactorRow> = listOf(
    FactorRow(
        name = "宽度",
        score = c.breadth,
        meaning = "上涨家数相对下跌家数的净优势（平盘不进分母）",
        detail = "末日 涨 ${d.upCount} / 跌 ${d.downCount} / 平 ${d.flatCount}",
    ),
    FactorRow(
        name = "资金",
        score = c.moneyFlow,
        meaning = "上涨股成交额相对下跌股的净优势",
    ),
    FactorRow(
        name = "缺口",
        score = c.gapBreadth,
        meaning = "高开相对低开的家数优势（开盘缺口宽度）",
    ),
    FactorRow(
        name = "结构",
        score = c.structure,
        meaning = "家数方向与等权均涨跌幅是否同向；严重背离会靠近中性",
    ),
    FactorRow(
        name = "新高新低",
        score = c.nhNl,
        meaning = "窗口内创新高家数相对创新低的净优势",
        detail = "末日 新高 ${d.newHighCount} · 新低 ${d.newLowCount}",
    ),
)

private fun intensityFactors(c: MsiComponents): List<FactorRow> = listOf(
    FactorRow(
        name = "有方向量能",
        score = c.directedVolume,
        meaning = "相对窗口的量能水位 × 涨跌宽度；放量下跌会压低分数",
        detail = "量能倍率 ${"%.2f".format(c.volumeRatio)}",
    ),
    FactorRow(
        name = "涨跌停压",
        score = c.limitPressure,
        meaning = "疑似涨停相对跌停的拥挤压力（启发式阈值，非官方涨跌停）",
    ),
)

private fun constraintFactors(c: MsiComponents, d: MsiDiagnostics): List<FactorRow> = listOf(
    FactorRow(
        name = "区间位置",
        score = c.rangePosition,
        meaning = "收盘价在近 N 日真实高低点区间的相对位置；越高越靠近上沿",
    ),
    FactorRow(
        name = "日内位置",
        score = c.intradayPosition,
        meaning = "收盘在当日最高最低之间的位置；收近最高偏强",
    ),
    FactorRow(
        name = "惯性",
        score = c.inertia,
        meaning = "连涨/未跌 vs 连跌/未涨的市场惯性差",
        detail = "均连涨 ${"%.1f".format(d.avgUpStreak)} · 均连跌 ${"%.1f".format(d.avgDownStreak)}",
    ),
)

@Composable
private fun DualScoreHeader(msi: MsiResult?, mtt: MttResult?) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.Top,
        ) {
            ScoreColumn(
                title = "短线 MSI",
                value = msi?.value,
                subtitle = msi?.let { scoreBandLabel(it.value) } ?: "—",
            )
            ScoreColumn(
                title = "中期 MTT",
                value = mtt?.value,
                subtitle = mtt?.let { mttStatusLabel(it.status) } ?: "—",
            )
        }
        Spacer(modifier = Modifier.height(12.dp))
        Text(
            comboOutlook(msi?.value, mtt?.value),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Medium,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 8.dp),
        )
        Spacer(modifier = Modifier.height(4.dp))
        val meta = buildList {
            msi?.let { add("窗口 ${it.days} 日") }
            mtt?.let { add("MA${it.maMid}/${it.maLong}") }
            val ts = msi?.calculatedAt ?: mtt?.calculatedAt
            if (ts != null) add(ts)
        }.joinToString(" · ")
        Text(
            meta,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun ScoreColumn(title: String, value: Double?, subtitle: String) {
    val tone = value?.let { msiTone(it) } ?: MaterialTheme.colorScheme.onSurfaceVariant
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            text = value?.roundToInt()?.toString() ?: "—",
            fontSize = 44.sp,
            fontWeight = FontWeight.Bold,
            color = tone,
        )
        Text(title, style = MaterialTheme.typography.labelMedium)
        Text(subtitle, style = MaterialTheme.typography.bodySmall, color = tone)
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.padding(top = 4.dp),
    )
}

@Composable
private fun LayerBar(label: String, score: Double) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(label, style = MaterialTheme.typography.bodyMedium)
            Text(
                "${"%.1f".format(score)} · ${scoreDegreeLabel(score)}",
                style = MaterialTheme.typography.bodyMedium,
                color = msiTone(score),
            )
        }
        Spacer(modifier = Modifier.height(4.dp))
        ScoreProgress(score)
    }
}

private fun msiTone(value: Double): Color = when {
    value > 65 -> Color(0xFFC62828)
    value < 35 -> Color(0xFF2E7D32)
    value >= 60 || value <= 40 -> Color(0xFFF9A825)
    else -> Color(0xFF546E7A)
}
