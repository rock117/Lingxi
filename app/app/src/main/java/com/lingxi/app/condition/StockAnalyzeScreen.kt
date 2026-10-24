package com.lingxi.app.condition

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.ui.QuoteColors
import com.lingxi.app.viewmodel.ScreenViewModel
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StockAnalyzeScreen(
    viewModel: ScreenViewModel,
    symbol: String,
    onBack: () -> Unit,
) {
    val result by viewModel.stockAnalyze.collectAsStateWithLifecycle()
    val loading by viewModel.stockAnalyzeLoading.collectAsStateWithLifecycle()
    val error by viewModel.stockAnalyzeError.collectAsStateWithLifecycle()

    LaunchedEffect(symbol) {
        viewModel.loadStockAnalyze(symbol)
    }

    val title = result?.let { r ->
        val n = r.name.ifBlank { r.symbol }.ifBlank { symbol }
        "分析 · $n"
    } ?: "分析个股"

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                    }
                },
                actions = {
                    TextButton(
                        onClick = { viewModel.loadStockAnalyze(symbol) },
                        enabled = !loading,
                    ) {
                        Text("重新分析")
                    }
                },
            )
        },
    ) { padding ->
        when {
            loading && result == null -> {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    CircularProgressIndicator()
                    Spacer(modifier = Modifier.height(12.dp))
                    Text(
                        "正在拉取成交明细并分析…",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            error != null && result == null -> {
                Text(
                    error.orEmpty(),
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(padding).padding(24.dp),
                )
            }
            result == null -> {
                Text("暂无数据", modifier = Modifier.padding(padding).padding(24.dp))
            }
            else -> {
                val r = result!!
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    item {
                        Text(
                            r.symbol.ifBlank { symbol },
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    item {
                        Card(modifier = Modifier.fillMaxWidth()) {
                            Column(
                                modifier = Modifier.padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Text(
                                    "成交明细",
                                    style = MaterialTheme.typography.titleSmall,
                                    fontWeight = FontWeight.SemiBold,
                                )
                                KvAnalyze("交易日", r.tradeDate.ifBlank { "—" })
                                KvAnalyze(
                                    "最新一笔",
                                    r.detailLastTime?.takeIf { it.isNotBlank() } ?: "无成交明细",
                                )
                                KvAnalyze("笔数", r.detailTickCount.toString())
                                KvAnalyze(
                                    "来源",
                                    when (r.detailSource) {
                                        "eastmoney" -> "东财实时"
                                        "tencent" -> "腾讯实时"
                                        "live" -> "实时"
                                        "db" -> "数据库快照"
                                        else -> r.detailSource.ifBlank { "—" }
                                    },
                                )
                                if (!r.detailUpdatedAt.isNullOrBlank()) {
                                    KvAnalyze("库更新时间", r.detailUpdatedAt)
                                }
                            }
                        }
                    }
                    item {
                        Card(modifier = Modifier.fillMaxWidth()) {
                            Column(
                                modifier = Modifier.padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Text(
                                        "AI 分析",
                                        style = MaterialTheme.typography.titleSmall,
                                        fontWeight = FontWeight.SemiBold,
                                        modifier = Modifier.weight(1f),
                                    )
                                    if (loading) {
                                        CircularProgressIndicator(
                                            modifier = Modifier.size(18.dp),
                                            strokeWidth = 2.dp,
                                        )
                                    }
                                }
                                if (!r.aiEnabled) {
                                    Text(
                                        "AI 未启用",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error,
                                    )
                                } else if (!r.aiModel.isNullOrBlank()) {
                                    Text(
                                        "模型 ${r.aiModel}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                                HorizontalDivider()
                                Text(
                                    r.analysis.ifBlank { "暂无分析内容" },
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                            }
                        }
                    }
                    if (r.bars.isNotEmpty()) {
                        item {
                            Text(
                                "近5日日K",
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }
                        items(r.bars.asReversed(), key = { it.date }) { bar ->
                            Card(modifier = Modifier.fillMaxWidth()) {
                                Column(modifier = Modifier.padding(12.dp)) {
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        Text(
                                            bar.date,
                                            modifier = Modifier.weight(1f),
                                            style = MaterialTheme.typography.bodyMedium,
                                        )
                                        Text(
                                            formatAnalyzePct(bar.changePct),
                                            color = QuoteColors.of(bar.changePct),
                                            fontWeight = FontWeight.SemiBold,
                                        )
                                    }
                                    Text(
                                        "O ${formatAnalyzePrice(bar.open)}  H ${formatAnalyzePrice(bar.high)}  " +
                                            "L ${formatAnalyzePrice(bar.low)}  C ${formatAnalyzePrice(bar.close)}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                    Text(
                                        "额 ${formatAnalyzeAmount(bar.amount)}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun KvAnalyze(label: String, value: String) {
    Row(modifier = Modifier.fillMaxWidth()) {
        Text(
            label,
            modifier = Modifier.weight(1f),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(value, fontWeight = FontWeight.Medium)
    }
}

private fun formatAnalyzePrice(v: Double): String =
    String.format(Locale.US, "%.2f", v)

private fun formatAnalyzePct(v: Double): String =
    String.format(Locale.US, "%+.2f%%", v)

private fun formatAnalyzeAmount(v: Double): String = when {
    v >= 1e8 -> String.format(Locale.US, "%.2f亿", v / 1e8)
    v >= 1e4 -> String.format(Locale.US, "%.2f万", v / 1e4)
    else -> String.format(Locale.US, "%.0f", v)
}
