package com.lingxi.app.market

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.DimAi
import com.lingxi.app.data.model.MsiComponents
import com.lingxi.app.data.model.MsiDiagnostics
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.mttStatusLabel
import com.lingxi.app.data.model.scoreBandLabel
import com.lingxi.app.data.model.scoreDegreeLabel
import com.lingxi.app.ui.PercentProgress
import com.lingxi.app.ui.QuoteColors
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
            SentimentContent(
                msi = ui.result,
                mtt = ui.mtt,
                ai = ui.ai,
                aiEnabled = ui.aiEnabled,
                aiModel = ui.aiModel,
                aiLoading = ui.aiLoading,
                onRetryAi = { viewModel.loadSentimentAi(force = true) },
            )
        }
    }
}

@Composable
private fun SentimentContent(
    msi: MsiResult?,
    mtt: MttResult?,
    ai: DimAi,
    aiEnabled: Boolean,
    aiModel: String?,
    aiLoading: Boolean,
    onRetryAi: () -> Unit,
) {
    var expanded by remember { mutableStateOf<MsiLayerKind?>(null) }
    val today = msi?.today

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        DualScoreHeader(msi = msi, mtt = mtt)

        SentimentAiPanel(
            ai = ai,
            aiEnabled = aiEnabled,
            aiModel = aiModel,
            loading = aiLoading,
            onRetry = onRetryAi,
        )

        if (mtt != null) {
            SectionTitle("中期 MTT")
            LayerBar("中期趋势", mtt.value)
            Text(
                "多头 ${mtt.counts.bull} 家，空头 ${mtt.counts.bear} 家，纠缠 ${mtt.counts.neutral} 家",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "MA${mtt.maMid}/${mtt.maLong} · ${mtt.maType.uppercase()}" +
                    if (mtt.useSlopeFilter) " · 斜率过滤" else "",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        if (today != null) {
            SectionTitle("今日三层")
            Text(
                "看的是今天这一天。点开一层能看具体因子；约束分高说明收得强，不是扣分。",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            ExpandableMsiLayer(
                title = "方向",
                subtitle = "市场站哪边",
                score = today.layers.direction,
                summary = layerSummary(MsiLayerKind.Direction, today.layers.direction),
                factors = directionFactors(today.components, today.diagnostics),
                expanded = expanded == MsiLayerKind.Direction,
                onToggle = {
                    expanded = if (expanded == MsiLayerKind.Direction) null else MsiLayerKind.Direction
                },
            )
            ExpandableMsiLayer(
                title = "强度",
                subtitle = "涨跌打得有多用力",
                score = today.layers.intensity,
                summary = layerSummary(MsiLayerKind.Intensity, today.layers.intensity),
                factors = intensityFactors(today.components, today.diagnostics),
                expanded = expanded == MsiLayerKind.Intensity,
                onToggle = {
                    expanded = if (expanded == MsiLayerKind.Intensity) null else MsiLayerKind.Intensity
                },
            )
            ExpandableMsiLayer(
                title = "约束",
                subtitle = "收在当天高低的什么位置",
                score = today.layers.constraint,
                summary = layerSummary(MsiLayerKind.Constraint, today.layers.constraint),
                factors = constraintFactors(today.components),
                expanded = expanded == MsiLayerKind.Constraint,
                onToggle = {
                    expanded =
                        if (expanded == MsiLayerKind.Constraint) null else MsiLayerKind.Constraint
                },
            )

            if (msi.recent.series.isNotEmpty()) {
                SectionTitle("近 ${msi.recent.days} 日日分")
                Text(
                    msi.recent.series.joinToString(" · ") {
                        val d = it.date.takeLast(5)
                        "$d ${"%.0f".format(it.value)}"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            SectionTitle("信号")
            Text(
                "收在当天${priceStatusLabel(today.signal.priceStatus)}" +
                    "（${"%.0f".format(today.signal.pricePosition * 100)}%）",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                listOf(today.signal.label, today.signal.advice)
                    .filter { it.isNotBlank() }
                    .joinToString("，"),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            SectionTitle("盘面一览")
            val d = today.diagnostics
            Text(
                "共看 ${d.sampleSize} 只，涨 ${d.upCount}、跌 ${d.downCount}、平 ${d.flatCount}",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                "涨停 ${d.limitUpCount}，跌停 ${d.limitDownCount}",
                style = MaterialTheme.typography.bodyMedium,
            )
            if (mtt != null) {
                Text(
                    "中期相对均线中位偏离 ${"%.1f".format(mtt.diagnostics.deviationMedian * 100)}%，" +
                        "近20日新高相对新低 ${"%.1f".format(mtt.diagnostics.nhNl)}",
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
    PercentProgress(
        percent = score,
        height = height,
        color = msiTone(score),
    )
}

private fun layerSummary(kind: MsiLayerKind, score: Double): String = when (kind) {
    MsiLayerKind.Direction -> when {
        score > 65 -> "涨的家数和资金都在往上靠，市场站在多头这边。"
        score < 35 -> "跌的家数和资金占上风，市场站在空头这边。"
        score >= 60 -> "略微偏多，但还没拧成一股劲。"
        score <= 40 -> "略微偏空，但还没拧成一股劲。"
        else -> "多空差不多，谁也没占稳。"
    }
    MsiLayerKind.Intensity -> when {
        score > 65 -> "涨的时候打得比较狠。"
        score < 35 -> "跌的时候砸得比较狠。"
        score >= 60 -> "多头用力稍大一点，还不算猛。"
        score <= 40 -> "空头用力稍大一点，还不算猛。"
        else -> "力度一般，方向也不清晰。"
    }
    MsiLayerKind.Constraint -> when {
        score > 65 -> "多数票收在当天偏高的位置，收得挺强。"
        score < 35 -> "多数票收在当天偏低的位置，收得偏弱。"
        else -> "收盘位置不偏不倚。"
    }
}

private fun factorExtremes(factors: List<FactorRow>): String? {
    if (factors.size < 2) return null
    val strongest = factors.maxBy { it.score }
    val weakest = factors.minBy { it.score }
    if (strongest.name == weakest.name) return null
    return "这一层里，${strongest.name}最撑场面（${"%.0f".format(strongest.score)}），" +
        "${weakest.name}偏拖后腿（${"%.0f".format(weakest.score)}）。"
}

private fun directionFactors(c: MsiComponents, d: MsiDiagnostics): List<FactorRow> = listOf(
    FactorRow(
        name = "宽度",
        score = c.breadth,
        meaning = "涨的家数多，还是跌的多",
        detail = "今天涨 ${d.upCount}、跌 ${d.downCount}、平 ${d.flatCount}",
    ),
    FactorRow(
        name = "资金",
        score = c.moneyFlow,
        meaning = "钱更多流向上涨的票，还是下跌的票",
    ),
    FactorRow(
        name = "结构",
        score = c.structure,
        meaning = "家数方向和平均涨跌是否对得上；对不上就往中间靠",
    ),
)

private fun intensityFactors(c: MsiComponents, d: MsiDiagnostics): List<FactorRow> = listOf(
    FactorRow(
        name = "有方向波动",
        score = c.directedMove,
        meaning = "涨跌面叠上波动幅度；跌得多又跌得深，分数会下来",
        detail = "平均绝对涨跌约 ${"%.2f".format(d.meanAbsRet)}%",
    ),
)

private fun constraintFactors(c: MsiComponents): List<FactorRow> = listOf(
    FactorRow(
        name = "日内位置",
        score = c.intradayPosition,
        meaning = "收盘落在当天高低之间哪儿；越靠近最高越强",
    ),
)

private fun priceStatusLabel(raw: String): String = when (raw) {
    "high_zone" -> "偏高位"
    "upper_mid" -> "中上"
    "mid_zone" -> "中部"
    "lower_mid" -> "中下"
    "low_zone" -> "偏低位"
    else -> raw
}

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
                title = "今日",
                value = msi?.today?.value,
                subtitle = msi?.today?.let { scoreBandLabel(it.value) } ?: "—",
            )
            ScoreColumn(
                title = "近${msi?.recent?.days ?: "N"}日",
                value = msi?.recent?.value,
                subtitle = msi?.recent?.let { scoreBandLabel(it.value) } ?: "—",
            )
            ScoreColumn(
                title = "中期 MTT",
                value = mtt?.value,
                subtitle = mtt?.let { mttStatusLabel(it.status) } ?: "—",
            )
        }
        Spacer(modifier = Modifier.height(8.dp))
        val meta = buildList {
            msi?.today?.date?.let { add(it) }
            msi?.let { add("近 ${it.recent.days} 日") }
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
            fontSize = 36.sp,
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

@Composable
private fun SentimentAiPanel(
    ai: DimAi,
    aiEnabled: Boolean,
    aiModel: String?,
    loading: Boolean,
    onRetry: () -> Unit,
) {
    val biasLabel = when (ai.bias) {
        "bullish" -> "偏多"
        "bearish" -> "偏空"
        "neutral" -> "中性"
        else -> null
    }
    val confLabel = when (ai.confidence) {
        "high" -> "高置信"
        "medium" -> "中置信"
        "low" -> "低置信"
        else -> null
    }
    val body = when {
        loading && ai.text.isBlank() -> "正在综合当日与近5日数据…"
        ai.status == "pending" && aiEnabled -> "准备生成综合解读…"
        ai.status == "disabled" -> ai.text.ifBlank { "AI 未启用" }
        ai.status == "error" -> ai.text.ifBlank { "AI 失败" }
        else -> ai.text.ifBlank { "暂无分析内容" }
    }
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "综合解读",
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                if (loading) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                } else if (!aiEnabled) {
                    Text(
                        "未启用",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                } else if (!aiModel.isNullOrBlank()) {
                    Text(
                        aiModel,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            if (biasLabel != null || confLabel != null || ai.flags.isNotEmpty()) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    biasLabel?.let {
                        val c = when (ai.bias) {
                            "bullish" -> QuoteColors.Up
                            "bearish" -> QuoteColors.Down
                            else -> MaterialTheme.colorScheme.onSurfaceVariant
                        }
                        Text(
                            it,
                            style = MaterialTheme.typography.labelLarge,
                            fontWeight = FontWeight.SemiBold,
                            color = c,
                        )
                    }
                    confLabel?.let {
                        Text(
                            it,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    ai.flags.take(4).forEach { flag ->
                        Text(
                            flag,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier
                                .clip(RoundedCornerShape(6.dp))
                                .background(
                                    MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f),
                                )
                                .padding(horizontal = 8.dp, vertical = 4.dp),
                        )
                    }
                }
            }
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
            )
            Text(
                body,
                style = MaterialTheme.typography.bodyMedium.copy(lineHeight = 22.sp),
            )
            if (ai.status == "error" || (ai.status == "ready" && aiEnabled)) {
                Text(
                    if (ai.status == "error") "重试" else "重新生成",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable(enabled = !loading, onClick = onRetry),
                )
            }
        }
    }
}
