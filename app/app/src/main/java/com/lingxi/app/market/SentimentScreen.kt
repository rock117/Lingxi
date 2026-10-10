package com.lingxi.app.market

import androidx.compose.foundation.background
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.MsiComponents
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.comboOutlook
import com.lingxi.app.data.model.mttStatusLabel
import com.lingxi.app.data.model.scoreBandLabel
import com.lingxi.app.viewmodel.MarketViewModel
import kotlin.math.roundToInt

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
            Text(
                "短线情绪 · 中期结构",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
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
                    "部分刷新失败：${ui.error}",
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
            LayerBar("方向 Direction", msi.layers.direction)
            LayerBar("强度 Intensity", msi.layers.intensity)
            LayerBar("约束 Constraint", msi.layers.constraint)

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

            SectionTitle("分量")
            ComponentGrid(msi.components)

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
            Text("%.1f".format(score), style = MaterialTheme.typography.bodyMedium)
        }
        Spacer(modifier = Modifier.height(4.dp))
        LinearProgressIndicator(
            progress = { (score / 100.0).toFloat().coerceIn(0f, 1f) },
            modifier = Modifier
                .fillMaxWidth()
                .height(8.dp)
                .clip(RoundedCornerShape(4.dp)),
            color = msiTone(score),
            trackColor = MaterialTheme.colorScheme.surfaceVariant,
            strokeCap = StrokeCap.Butt,
            gapSize = 0.dp,
            drawStopIndicator = {},
        )
    }
}

@Composable
private fun ComponentGrid(c: MsiComponents) {
    val items = listOf(
        "宽度" to c.breadth,
        "资金" to c.moneyFlow,
        "缺口" to c.gapBreadth,
        "结构" to c.structure,
        "新高新低" to c.nhNl,
        "有方向量能" to c.directedVolume,
        "涨跌停压" to c.limitPressure,
        "区间位置" to c.rangePosition,
        "日内位置" to c.intradayPosition,
        "惯性" to c.inertia,
    )
    items.chunked(2).forEach { row ->
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            row.forEach { (name, value) ->
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(8.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
                        .padding(12.dp),
                ) {
                    Column {
                        Text(name, style = MaterialTheme.typography.labelMedium)
                        Text(
                            "%.1f".format(value),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Medium,
                        )
                    }
                }
            }
            if (row.size == 1) {
                Spacer(modifier = Modifier.weight(1f))
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
    }
    Text(
        "量能倍率 ${"%.2f".format(c.volumeRatio)}",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

private fun msiTone(value: Double): Color = when {
    value > 65 -> Color(0xFFC62828)
    value < 35 -> Color(0xFF2E7D32)
    value >= 60 || value <= 40 -> Color(0xFFF9A825)
    else -> Color(0xFF546E7A)
}
