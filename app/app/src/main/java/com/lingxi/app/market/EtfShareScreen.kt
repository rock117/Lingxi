package com.lingxi.app.market

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.foundation.Canvas
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.EtfShareGroup
import com.lingxi.app.data.model.EtfShareMember
import com.lingxi.app.viewmodel.EtfShareMetric
import com.lingxi.app.viewmodel.EtfShareViewMode
import com.lingxi.app.viewmodel.MarketViewModel
import java.util.Locale
import kotlin.math.abs

private val GroupColors = listOf(
    Color(0xFF1565C0),
    Color(0xFF2E7D32),
    Color(0xFF6A1B9A),
    Color(0xFFEF6C00),
)

@Composable
fun EtfShareScreen(viewModel: MarketViewModel) {
    val ui by viewModel.etfShare.collectAsStateWithLifecycle()
    var highlightId by rememberSaveable { mutableStateOf<String?>(null) }
    var expandedId by rememberSaveable { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        viewModel.loadEtfShareIfNeeded()
    }

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
                    "宽基ETF份额",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    "四组合计 · 场内份额 / 估算市值",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(
                onClick = { viewModel.refreshEtfShare() },
                enabled = !ui.loading,
            ) {
                Icon(Icons.Default.Refresh, contentDescription = "刷新")
            }
        }

        Spacer(modifier = Modifier.height(8.dp))
        ChipRow {
            listOf("day" to "日", "week" to "周", "month" to "月").forEach { (id, label) ->
                FilterChip(
                    selected = ui.granularity == id,
                    onClick = { viewModel.setEtfShareGranularity(id) },
                    label = { Text(label) },
                )
            }
        }
        ChipRow {
            FilterChip(
                selected = ui.metric == EtfShareMetric.Share,
                onClick = { viewModel.setEtfShareMetric(EtfShareMetric.Share) },
                label = { Text("份额") },
            )
            FilterChip(
                selected = ui.metric == EtfShareMetric.Mkt,
                onClick = { viewModel.setEtfShareMetric(EtfShareMetric.Mkt) },
                label = { Text("市值") },
            )
            FilterChip(
                selected = ui.viewMode == EtfShareViewMode.Chart,
                onClick = { viewModel.setEtfShareViewMode(EtfShareViewMode.Chart) },
                label = { Text("图表") },
            )
            FilterChip(
                selected = ui.viewMode == EtfShareViewMode.Table,
                onClick = { viewModel.setEtfShareViewMode(EtfShareViewMode.Table) },
                label = { Text("表格") },
            )
        }
        ChipRow {
            listOf(
                "1m" to "近1月",
                "3m" to "近3月",
                "6m" to "近半年",
                "ytd" to "今年",
                "1y" to "近1年",
            ).forEach { (id, label) ->
                FilterChip(
                    selected = ui.range == id,
                    onClick = { viewModel.setEtfShareRange(id) },
                    label = { Text(label) },
                )
            }
        }

        when {
            ui.loading && ui.result == null -> {
                Spacer(modifier = Modifier.height(24.dp))
                CircularProgressIndicator(modifier = Modifier.align(Alignment.CenterHorizontally))
            }
            ui.error != null && ui.result == null -> {
                Text(
                    ui.error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(vertical = 16.dp),
                )
            }
            else -> {
                val result = ui.result
                if (result != null) {
                    Text(
                        "截至 ${result.asOf} · ${if (ui.metric == EtfShareMetric.Share) "亿份" else "亿元（收盘估算）"}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (ui.error != null) {
                        Text(
                            ui.error ?: "",
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .verticalScroll(rememberScrollState()),
                    ) {
                        if (ui.viewMode == EtfShareViewMode.Chart) {
                            EtfShareChart(
                                groups = result.groups,
                                metric = ui.metric,
                                highlightId = highlightId,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(220.dp),
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            LegendRow(
                                groups = result.groups,
                                highlightId = highlightId,
                                onToggle = { id ->
                                    highlightId = if (highlightId == id) null else id
                                },
                            )
                            Spacer(modifier = Modifier.height(12.dp))
                        }
                        result.groups.forEachIndexed { idx, g ->
                            GroupSummaryRow(
                                group = g,
                                metric = ui.metric,
                                color = GroupColors[idx % GroupColors.size],
                                expanded = expandedId == g.id && ui.viewMode == EtfShareViewMode.Table,
                                showExpand = ui.viewMode == EtfShareViewMode.Table,
                                highlighted = highlightId == g.id,
                                onHeaderClick = {
                                    if (ui.viewMode == EtfShareViewMode.Table) {
                                        expandedId = if (expandedId == g.id) null else g.id
                                    }
                                    highlightId = g.id
                                },
                            )
                            if (ui.viewMode == EtfShareViewMode.Table && expandedId == g.id) {
                                val members = remember(g, ui.metric) {
                                    sortedMembers(g, ui.metric)
                                }
                                members.forEach { m ->
                                    MemberRow(member = m, metric = ui.metric)
                                }
                            }
                        }
                        if (result.note.isNotBlank()) {
                            Spacer(modifier = Modifier.height(12.dp))
                            Text(
                                result.note,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Spacer(modifier = Modifier.height(24.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun ChipRow(content: @Composable () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        content()
    }
    Spacer(modifier = Modifier.height(6.dp))
}

@Composable
private fun LegendRow(
    groups: List<EtfShareGroup>,
    highlightId: String?,
    onToggle: (String) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        groups.forEachIndexed { idx, g ->
            val c = GroupColors[idx % GroupColors.size]
            val dim = highlightId != null && highlightId != g.id
            Text(
                g.label,
                color = if (dim) c.copy(alpha = 0.35f) else c,
                style = MaterialTheme.typography.labelMedium,
                fontWeight = if (highlightId == g.id) FontWeight.Bold else FontWeight.Normal,
                modifier = Modifier.clickable { onToggle(g.id) },
            )
        }
    }
}

@Composable
private fun EtfShareChart(
    groups: List<EtfShareGroup>,
    metric: EtfShareMetric,
    highlightId: String?,
    modifier: Modifier = Modifier,
) {
    val grid = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f)
    Canvas(modifier = modifier) {
        val w = size.width
        val h = size.height
        if (w <= 0f || h <= 0f) return@Canvas
        val padL = 8.dp.toPx()
        val padR = 8.dp.toPx()
        val padT = 8.dp.toPx()
        val padB = 8.dp.toPx()
        val plotW = w - padL - padR
        val plotH = h - padT - padB

        var minV = Double.POSITIVE_INFINITY
        var maxV = Double.NEGATIVE_INFINITY
        groups.forEach { g ->
            g.series.forEach { p ->
                val v = if (metric == EtfShareMetric.Share) p.share else p.mkt
                if (v.isFinite()) {
                    minV = minOf(minV, v)
                    maxV = maxOf(maxV, v)
                }
            }
        }
        if (!minV.isFinite() || !maxV.isFinite() || maxV <= minV) {
            minV = 0.0
            maxV = 1.0
        }
        val span = (maxV - minV).coerceAtLeast(1e-9)

        // grid
        for (i in 0..3) {
            val y = padT + plotH * i / 3f
            drawLine(grid, Offset(padL, y), Offset(padL + plotW, y), strokeWidth = 1f)
        }

        val maxLen = groups.maxOfOrNull { it.series.size } ?: 0
        if (maxLen < 2) return@Canvas

        groups.forEachIndexed { idx, g ->
            val series = g.series
            if (series.size < 2) return@forEachIndexed
            val dim = highlightId != null && highlightId != g.id
            val color = GroupColors[idx % GroupColors.size].copy(alpha = if (dim) 0.25f else 1f)
            val path = Path()
            series.forEachIndexed { i, p ->
                val v = if (metric == EtfShareMetric.Share) p.share else p.mkt
                val x = padL + plotW * i / (series.size - 1).coerceAtLeast(1)
                val y = padT + plotH * (1f - ((v - minV) / span).toFloat().coerceIn(0f, 1f))
                if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            drawPath(path, color = color, style = Stroke(width = if (highlightId == g.id) 4f else 2.5f))
        }
    }
}

@Composable
private fun GroupSummaryRow(
    group: EtfShareGroup,
    metric: EtfShareMetric,
    color: Color,
    expanded: Boolean,
    showExpand: Boolean,
    highlighted: Boolean,
    onHeaderClick: () -> Unit,
) {
    val latest = if (metric == EtfShareMetric.Share) {
        formatYi(group.latestShare)
    } else {
        formatYi(group.latestMkt)
    }
    val deltaPct = if (metric == EtfShareMetric.Share) group.deltaSharePct else group.deltaMktPct
    val deltaAbs = if (metric == EtfShareMetric.Share) group.deltaShare else group.deltaMkt
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onHeaderClick)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (showExpand) {
            Icon(
                if (expanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            "●",
            color = color,
            modifier = Modifier.padding(end = 6.dp),
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(
                group.label,
                fontWeight = if (highlighted) FontWeight.Bold else FontWeight.Medium,
            )
            Text(
                "区间 ${formatYiSigned(deltaAbs)}  (${formatPct(deltaPct)})",
                style = MaterialTheme.typography.bodySmall,
                color = deltaColor(deltaPct),
            )
        }
        Text(
            latest,
            fontWeight = FontWeight.SemiBold,
            textAlign = TextAlign.End,
        )
    }
}

@Composable
private fun MemberRow(member: EtfShareMember, metric: EtfShareMetric) {
    val value = if (metric == EtfShareMetric.Share) {
        formatYi(member.share)
    } else {
        formatYi(member.mkt)
    }
    val deltaPct = if (metric == EtfShareMetric.Share) member.deltaSharePct else member.deltaMktPct
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 28.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(member.name, style = MaterialTheme.typography.bodyMedium)
            Text(
                member.symbol,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Column(horizontalAlignment = Alignment.End) {
            Text(value, style = MaterialTheme.typography.bodyMedium)
            Text(
                formatPct(deltaPct),
                style = MaterialTheme.typography.bodySmall,
                color = deltaColor(deltaPct),
            )
        }
    }
}

private fun sortedMembers(group: EtfShareGroup, metric: EtfShareMetric): List<EtfShareMember> {
    return if (metric == EtfShareMetric.Share) {
        group.members.sortedByDescending { it.share }
    } else {
        group.members.sortedByDescending { it.mkt ?: -1.0 }
    }
}

private fun formatYi(v: Double?): String {
    if (v == null || !v.isFinite()) return "—"
    val yi = v / 1e8
    return String.format(Locale.US, "%,.2f亿", yi)
}

private fun formatYiSigned(v: Double?): String {
    if (v == null || !v.isFinite()) return "—"
    val yi = v / 1e8
    val sign = if (yi > 0) "+" else ""
    return String.format(Locale.US, "%s%,.2f亿", sign, yi)
}

private fun formatPct(v: Double?): String {
    if (v == null || !v.isFinite()) return "—"
    val sign = if (v > 0) "+" else ""
    return String.format(Locale.US, "%s%.2f%%", sign, v)
}

@Composable
private fun deltaColor(pct: Double?): Color {
    val p = pct ?: return MaterialTheme.colorScheme.onSurfaceVariant
    return when {
        abs(p) < 1e-9 -> MaterialTheme.colorScheme.onSurfaceVariant
        p > 0 -> Color(0xFFC62828)
        else -> Color(0xFF2E7D32)
    }
}
