package com.lingxi.app.market

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
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.FaceChangeSummary
import com.lingxi.app.data.model.FundStatsResult
import com.lingxi.app.data.model.RatioStat
import com.lingxi.app.ui.PercentProgress
import com.lingxi.app.viewmodel.MarketViewModel
import java.util.Locale
import kotlin.math.abs

@Composable
fun FundStatsScreen(viewModel: MarketViewModel) {
    val ui by viewModel.fundStats.collectAsStateWithLifecycle()

    LaunchedEffect(Unit) {
        viewModel.loadFundStatsIfNeeded()
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
                    "A股财报统计",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    "水平面 · 增长面 · 同比/环比汇总",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(
                onClick = { viewModel.refreshFundStats(keepSelection = true) },
                enabled = !ui.loading,
            ) {
                Icon(Icons.Default.Refresh, contentDescription = "刷新")
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        Text("期次", style = MaterialTheme.typography.labelMedium)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val periods = ui.meta?.periods.orEmpty().ifEmpty {
                listOf(
                    com.lingxi.app.data.model.PeriodOption("Q1", "一季报"),
                    com.lingxi.app.data.model.PeriodOption("H1", "中报"),
                    com.lingxi.app.data.model.PeriodOption("Q3", "三季报"),
                    com.lingxi.app.data.model.PeriodOption("FY", "年报"),
                )
            }
            periods.forEach { p ->
                FilterChip(
                    selected = ui.selectedPeriod.equals(p.id, ignoreCase = true),
                    onClick = { viewModel.selectFundPeriod(p.id) },
                    label = { Text(p.label) },
                )
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        Text("年份", style = MaterialTheme.typography.labelMedium)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val years = ui.result?.availableYears
                ?: ui.meta?.availableYears
                ?: emptyList()
            if (years.isEmpty()) {
                Text(
                    "暂无年份数据",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                years.forEach { y ->
                    FilterChip(
                        selected = ui.selectedYear == y,
                        onClick = { viewModel.selectFundYear(y) },
                        label = { Text(y.toString()) },
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        when {
            ui.loading && ui.result == null -> {
                Column(
                    modifier = Modifier.fillMaxSize(),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    CircularProgressIndicator()
                }
            }
            ui.error != null && ui.result == null -> {
                Text(
                    ui.error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            else -> {
                val result = ui.result
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (ui.loading) {
                        LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                    }
                    ui.error?.let { err ->
                        Text(
                            err,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    if (result != null) {
                        Text(
                            "${result.year} · ${result.periodLabel}",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Medium,
                        )
                        CompareBases(result)

                        ProfitHero(result.netProfit)

                        SummaryBlock(result.summary)

                        SectionLabel("水平面")
                        StatCard(
                            title = "归母净利赚钱面",
                            subtitle = "归母净利 > 0",
                            stat = result.netProfit,
                        )
                        StatCard(
                            title = "经营现金流为正",
                            subtitle = "经营现金流净额 > 0",
                            stat = result.operatingCf,
                        )

                        SectionLabel("增长面")
                        StatCard(
                            title = "归母净利增长面",
                            subtitle = "归母净利同比 > 0（需上年同口径）",
                            stat = result.netProfitYoy,
                        )
                        StatCard(
                            title = "营收增长面",
                            subtitle = "营收同比 > 0（需上年同口径）",
                            stat = result.revenueYoy,
                        )

                        Text(
                            result.note,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(modifier = Modifier.height(24.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun CompareBases(result: FundStatsResult) {
    val yoy = result.yoyBase
    val qoq = result.qoqBase
    if (yoy == null && qoq == null) return
    val parts = buildList {
        if (yoy != null) add("同比对照 ${yoy.year}${yoy.periodLabel}")
        if (qoq != null) add("环比对照 ${qoq.year}${qoq.periodLabel}")
    }
    Text(
        parts.joinToString(" · "),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun SummaryBlock(summary: FaceChangeSummary) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(
            "同比 / 环比汇总",
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Text(
            "同比 升${summary.yoyUp} · 降${summary.yoyDown} · 平${summary.yoyFlat}",
            style = MaterialTheme.typography.bodyMedium,
        )
        Text(
            "环比 升${summary.qoqUp} · 降${summary.qoqDown} · 平${summary.qoqFlat}",
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun ProfitHero(stat: RatioStat) {
    val pct = stat.positivePct
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            pct?.let { String.format(Locale.CHINA, "%.1f%%", it) } ?: "—",
            style = MaterialTheme.typography.displaySmall,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            "赚钱公司占比",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(4.dp))
        DeltaRow(stat)
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            "赚钱 ${stat.positive} · 亏损/非正 ${stat.nonPositive} · 样本 ${stat.sample}",
            style = MaterialTheme.typography.bodySmall,
        )
        if (stat.sample > 0 && pct != null) {
            Spacer(modifier = Modifier.height(8.dp))
            PercentProgress(percent = pct)
        }
    }
}

@Composable
private fun StatCard(
    title: String,
    subtitle: String,
    stat: RatioStat,
) {
    val pct = stat.positivePct
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
        Text(
            subtitle,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.Bottom,
        ) {
            Text(
                pct?.let { String.format(Locale.CHINA, "%.1f%%", it) } ?: "—",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Medium,
            )
            Text(
                "${stat.positive} / ${stat.sample}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        DeltaRow(stat)
        if (stat.sample > 0 && pct != null) {
            PercentProgress(percent = pct)
        }
    }
}

@Composable
private fun DeltaRow(stat: RatioStat) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        DeltaLabel("同比", stat.yoyDeltaPct)
        DeltaLabel("环比", stat.qoqDeltaPct)
    }
}

@Composable
private fun DeltaLabel(prefix: String, delta: Double?) {
    val scheme = MaterialTheme.colorScheme
    val (text, color) = when {
        delta == null -> "$prefix —" to scheme.onSurfaceVariant
        abs(delta) <= 0.05 -> "$prefix 持平" to scheme.onSurfaceVariant
        delta > 0 ->
            "$prefix +${String.format(Locale.CHINA, "%.1f", delta)}pct" to Color(0xFFC62828)
        else ->
            "$prefix ${String.format(Locale.CHINA, "%.1f", delta)}pct" to Color(0xFF2E7D32)
    }
    Text(text, style = MaterialTheme.typography.bodySmall, color = color)
}
