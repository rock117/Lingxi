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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.MsiComponents
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.viewmodel.MarketViewModel
import kotlin.math.roundToInt

@Composable
fun SentimentScreen(viewModel: MarketViewModel) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()

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
                "短线情绪参考，非买卖指令",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            IconButton(onClick = { viewModel.refresh() }, enabled = !ui.loading) {
                Icon(Icons.Default.Refresh, contentDescription = "刷新")
            }
        }

        if (ui.loading && ui.result == null) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        } else if (ui.error != null && ui.result == null) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(24.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text("加载失败", style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.height(8.dp))
                Text(
                    ui.error ?: "",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    ui.serverHint,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(16.dp))
                TextButton(onClick = { viewModel.refresh() }) {
                    Text("重试")
                }
            }
        } else if (ui.result != null) {
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
            SentimentContent(result = ui.result!!)
        }
    }
}

@Composable
private fun SentimentContent(result: MsiResult) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        val tone = msiTone(result.value)
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = result.value.roundToInt().toString(),
                fontSize = 64.sp,
                fontWeight = FontWeight.Bold,
                color = tone,
            )
            Text(
                result.signal.label,
                style = MaterialTheme.typography.titleMedium,
                color = tone,
            )
            Text(
                result.signal.advice,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                "窗口 ${result.days} 日 · ${result.calculatedAt}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        SectionTitle("三层")
        LayerBar("方向 Direction", result.layers.direction)
        LayerBar("强度 Intensity", result.layers.intensity)
        LayerBar("约束 Constraint", result.layers.constraint)

        SectionTitle("信号上下文")
        Text(
            "量能 ${result.signal.volumeStatus} · 倍率 ${"%.2f".format(result.signal.volumeRatio)}",
            style = MaterialTheme.typography.bodyMedium,
        )
        Text(
            "位置 ${result.signal.priceStatus} · ${"%.0f".format(result.signal.pricePosition * 100)}%",
            style = MaterialTheme.typography.bodyMedium,
        )

        SectionTitle("分量")
        ComponentGrid(result.components)

        SectionTitle("诊断")
        val d = result.diagnostics
        Text(
            "样本 ${d.sampleSize} · 涨 ${d.upCount} / 跌 ${d.downCount} / 平 ${d.flatCount}",
            style = MaterialTheme.typography.bodyMedium,
        )
        Text(
            "涨停 ${d.limitUpCount} · 跌停 ${d.limitDownCount} · 新高 ${d.newHighCount} · 新低 ${d.newLowCount}",
            style = MaterialTheme.typography.bodyMedium,
        )
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
        Spacer(Modifier.height(4.dp))
        LinearProgressIndicator(
            progress = { (score / 100.0).toFloat().coerceIn(0f, 1f) },
            modifier = Modifier
                .fillMaxWidth()
                .height(8.dp)
                .clip(RoundedCornerShape(4.dp)),
            color = msiTone(score),
            trackColor = MaterialTheme.colorScheme.surfaceVariant,
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
        Spacer(Modifier.height(8.dp))
    }
    Text(
        "量能倍率 ${"%.2f".format(c.volumeRatio)}",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

private fun msiTone(value: Double): Color = when {
    value > 65 -> Color(0xFFC62828) // 偏多偏红（A 股习惯）
    value < 35 -> Color(0xFF2E7D32)
    value >= 60 || value <= 40 -> Color(0xFFF9A825) // 灰区偏黄
    else -> Color(0xFF546E7A)
}
