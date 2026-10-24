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
import androidx.compose.material3.Button
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
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
fun StockDetailScreen(
    viewModel: ScreenViewModel,
    symbol: String,
    onBack: () -> Unit,
    onAnalyze: (String) -> Unit = {},
) {
    val detail by viewModel.stockDetail.collectAsStateWithLifecycle()
    val loading by viewModel.stockLoading.collectAsStateWithLifecycle()
    val error by viewModel.stockError.collectAsStateWithLifecycle()

    LaunchedEffect(symbol) {
        viewModel.loadStockDetail(symbol)
    }

    val title = detail?.let { d ->
        d.name.ifBlank { d.symbol }.ifBlank { symbol }
    } ?: symbol

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                    }
                },
            )
        },
    ) { padding ->
        when {
            loading && detail == null -> {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    CircularProgressIndicator()
                }
            }
            error != null && detail == null -> {
                Text(
                    error.orEmpty(),
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(padding).padding(24.dp),
                )
            }
            detail == null -> {
                Text("暂无数据", modifier = Modifier.padding(padding).padding(24.dp))
            }
            else -> {
                val d = detail!!
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    item {
                        Text(
                            d.symbol,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            d.date,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            formatPrice(d.close),
                            style = MaterialTheme.typography.headlineMedium,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            formatPct(d.changePct),
                            style = MaterialTheme.typography.titleMedium,
                            color = QuoteColors.of(d.changePct),
                            fontWeight = FontWeight.SemiBold,
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        Button(
                            onClick = { onAnalyze(d.symbol.ifBlank { symbol }) },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("AI 分析个股")
                        }
                    }
                    item {
                        Card(modifier = Modifier.fillMaxWidth()) {
                            Column(
                                modifier = Modifier.padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                KvRow("开盘", formatPrice(d.open))
                                KvRow("最高", formatPrice(d.high))
                                KvRow("最低", formatPrice(d.low))
                                KvRow("收盘", formatPrice(d.close))
                                KvRow("成交额", formatAmount(d.amount))
                                HorizontalDivider()
                                KvRow(
                                    "近5日涨跌",
                                    d.change5dPct?.let { formatPct(it) } ?: "—",
                                    valueColor = d.change5dPct?.let { QuoteColors.of(it) },
                                )
                                KvRow(
                                    "近10日涨跌",
                                    d.change10dPct?.let { formatPct(it) } ?: "—",
                                    valueColor = d.change10dPct?.let { QuoteColors.of(it) },
                                )
                            }
                        }
                    }
                    if (d.bars.isNotEmpty()) {
                        item {
                            Text(
                                "近期日K",
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }
                        items(d.bars.asReversed(), key = { it.date }) { bar ->
                            Card(modifier = Modifier.fillMaxWidth()) {
                                Column(modifier = Modifier.padding(12.dp)) {
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        Text(
                                            bar.date,
                                            modifier = Modifier.weight(1f),
                                            style = MaterialTheme.typography.bodyMedium,
                                        )
                                        Text(
                                            formatPct(bar.changePct),
                                            color = QuoteColors.of(bar.changePct),
                                            fontWeight = FontWeight.SemiBold,
                                        )
                                    }
                                    Text(
                                        "O ${formatPrice(bar.open)}  H ${formatPrice(bar.high)}  " +
                                            "L ${formatPrice(bar.low)}  C ${formatPrice(bar.close)}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                    Text(
                                        "额 ${formatAmount(bar.amount)}",
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
private fun KvRow(
    label: String,
    value: String,
    valueColor: androidx.compose.ui.graphics.Color? = null,
) {
    Row(modifier = Modifier.fillMaxWidth()) {
        Text(
            label,
            modifier = Modifier.weight(1f),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            value,
            fontWeight = FontWeight.Medium,
            color = valueColor ?: MaterialTheme.colorScheme.onSurface,
        )
    }
}

private fun formatPrice(v: Double): String =
    String.format(Locale.US, "%.2f", v)

private fun formatPct(v: Double): String =
    String.format(Locale.US, "%+.2f%%", v)

private fun formatAmount(v: Double): String = when {
    v >= 1e8 -> String.format(Locale.US, "%.2f亿", v / 1e8)
    v >= 1e4 -> String.format(Locale.US, "%.2f万", v / 1e4)
    else -> String.format(Locale.US, "%.0f", v)
}
