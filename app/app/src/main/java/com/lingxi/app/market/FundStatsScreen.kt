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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.RatioStat
import com.lingxi.app.ui.PercentProgress
import com.lingxi.app.viewmodel.MarketViewModel
import java.util.Locale

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
                    "全市场赚钱占比 · 增长面 · 经营现金流",
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

                        ProfitHero(result.netProfit)

                        StatCard(
                            title = "归母净利赚钱面",
                            subtitle = "归母净利 > 0",
                            stat = result.netProfit,
                        )
                        StatCard(
                            title = "营收增长面",
                            subtitle = "营收同比 > 0（需上年同口径）",
                            stat = result.revenueYoy,
                        )
                        StatCard(
                            title = "经营现金流为正",
                            subtitle = "经营现金流净额 > 0",
                            stat = result.operatingCf,
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
        if (stat.sample > 0 && pct != null) {
            PercentProgress(percent = pct)
        }
    }
}
