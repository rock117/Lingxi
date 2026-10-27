package com.lingxi.app.market

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.SectorBar
import com.lingxi.app.data.model.SectorDetail
import com.lingxi.app.data.model.SectorMoverRow
import com.lingxi.app.ui.QuoteColors
import com.lingxi.app.viewmodel.MarketViewModel
import java.util.Locale

@Composable
fun SectorTab(
    viewModel: MarketViewModel,
) {
    var selectedSymbol by rememberSaveable { mutableStateOf<String?>(null) }

    if (selectedSymbol != null) {
        SectorDetailScreen(
            viewModel = viewModel,
            symbol = selectedSymbol!!,
            onBack = {
                viewModel.clearSectorDetail()
                selectedSymbol = null
            },
        )
    } else {
        SectorMoversScreen(
            viewModel = viewModel,
            onOpenSector = { selectedSymbol = it },
        )
    }
}

@Composable
fun SectorMoversScreen(
    viewModel: MarketViewModel,
    onOpenSector: (String) -> Unit,
) {
    val ui by viewModel.sectorMovers.collectAsStateWithLifecycle()

    LaunchedEffect(Unit) {
        viewModel.loadSectorMoversIfNeeded()
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
                    "板块强弱",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    buildString {
                        append("按当日涨跌幅 · 行业/概念 Top10")
                        ui.data?.tradeDate?.takeIf { it.isNotBlank() }?.let {
                            append(" · $it")
                        }
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(
                onClick = { viewModel.refreshSectorMovers() },
                enabled = !ui.loading,
            ) {
                Icon(Icons.Default.Refresh, contentDescription = "刷新")
            }
        }

        when {
            ui.loading && ui.data == null -> {
                Box(Modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            }
            ui.error != null && ui.data == null -> {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(top = 48.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(ui.error ?: "加载失败", color = MaterialTheme.colorScheme.error)
                    TextButton(onClick = { viewModel.refreshSectorMovers() }) {
                        Text("重试")
                    }
                }
            }
            else -> {
                val data = ui.data
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(bottom = 24.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    if (ui.error != null) {
                        item {
                            Text(
                                ui.error!!,
                                color = MaterialTheme.colorScheme.error,
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }
                    item {
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            "领涨 Top${data?.topN ?: 10}",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                    itemsIndexed(data?.gainers.orEmpty()) { idx, row ->
                        SectorMoverRowItem(
                            rank = idx + 1,
                            row = row,
                            onClick = { onOpenSector(row.symbol) },
                        )
                        HorizontalDivider(
                            color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f),
                        )
                    }
                    item {
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            "领跌 Top${data?.topN ?: 10}",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                    itemsIndexed(data?.losers.orEmpty()) { idx, row ->
                        SectorMoverRowItem(
                            rank = idx + 1,
                            row = row,
                            onClick = { onOpenSector(row.symbol) },
                        )
                        HorizontalDivider(
                            color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun SectorMoverRowItem(
    rank: Int,
    row: SectorMoverRow,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            "$rank",
            modifier = Modifier.padding(end = 12.dp),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(row.name.ifBlank { row.symbol }, fontWeight = FontWeight.Medium)
            Text(
                typeLabel(row.sectorType),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Column(horizontalAlignment = Alignment.End) {
            Text(
                formatPct(row.changePct),
                color = QuoteColors.of(row.changePct),
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                formatAmount(row.amount),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
fun SectorDetailScreen(
    viewModel: MarketViewModel,
    symbol: String,
    onBack: () -> Unit,
) {
    val ui by viewModel.sectorDetail.collectAsStateWithLifecycle()

    LaunchedEffect(symbol) {
        viewModel.loadSectorDetail(symbol)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    ui.data?.name?.takeIf { it.isNotBlank() } ?: symbol,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    listOfNotNull(
                        ui.data?.symbol,
                        ui.data?.sectorType?.let { typeLabel(it) },
                        ui.data?.tradeDate,
                    ).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(
                onClick = { viewModel.loadSectorDetail(symbol) },
                enabled = !ui.loading,
            ) {
                Icon(Icons.Default.Refresh, contentDescription = "刷新")
            }
        }

        when {
            ui.loading && ui.data == null -> {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            }
            ui.error != null && ui.data == null -> {
                Text(
                    ui.error ?: "加载失败",
                    modifier = Modifier.padding(16.dp),
                    color = MaterialTheme.colorScheme.error,
                )
            }
            ui.data != null -> {
                SectorDetailBody(ui.data!!)
            }
        }
    }
}

@Composable
private fun SectorDetailBody(d: SectorDetail) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Surface(
                modifier = Modifier.fillMaxWidth(),
                tonalElevation = 1.dp,
                shape = MaterialTheme.shapes.medium,
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("当日", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
                    Spacer(modifier = Modifier.height(8.dp))
                    DetailGrid(
                        listOf(
                            "涨跌幅" to formatPct(d.changePct),
                            "收盘" to formatPrice(d.close),
                            "开盘" to formatPrice(d.open),
                            "最高" to formatPrice(d.high),
                            "最低" to formatPrice(d.low),
                            "昨收" to formatPrice(d.preClose),
                            "成交额" to formatAmount(d.amount),
                        ),
                        highlightPctKey = "涨跌幅",
                        pctValue = d.changePct,
                    )
                }
            }
        }
        item {
            Surface(
                modifier = Modifier.fillMaxWidth(),
                tonalElevation = 1.dp,
                shape = MaterialTheme.shapes.medium,
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        "多日涨跌",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    DetailGrid(
                        listOf(
                            "近5日" to (d.change5dPct?.let { formatPct(it) } ?: "—"),
                            "近10日" to (d.change10dPct?.let { formatPct(it) } ?: "—"),
                        ),
                        pctPairs = listOf(
                            "近5日" to d.change5dPct,
                            "近10日" to d.change10dPct,
                        ),
                    )
                }
            }
        }
        if (d.bars.isNotEmpty()) {
            item {
                Text(
                    "近${d.bars.size}日行情",
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                )
            }
            itemsIndexed(d.bars.asReversed()) { idx, bar ->
                if (idx > 0) {
                    HorizontalDivider(
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f),
                    )
                }
                SectorBarRow(bar)
            }
        }
    }
}

@Composable
private fun DetailGrid(
    cells: List<Pair<String, String>>,
    highlightPctKey: String? = null,
    pctValue: Double? = null,
    pctPairs: List<Pair<String, Double?>> = emptyList(),
) {
    val pctMap = pctPairs.toMap()
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        cells.chunked(2).forEach { row ->
            Row(modifier = Modifier.fillMaxWidth()) {
                row.forEach { (label, value) ->
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            label,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        val color = when {
                            highlightPctKey != null && label == highlightPctKey && pctValue != null ->
                                QuoteColors.of(pctValue)
                            pctMap.containsKey(label) ->
                                pctMap[label]?.let { QuoteColors.of(it) }
                                    ?: MaterialTheme.colorScheme.onSurface
                            else -> MaterialTheme.colorScheme.onSurface
                        }
                        Text(value, fontWeight = FontWeight.Medium, color = color)
                    }
                }
                if (row.size == 1) Spacer(modifier = Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun SectorBarRow(bar: SectorBar) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            bar.date.takeLast(5),
            modifier = Modifier.weight(1.1f),
            style = MaterialTheme.typography.bodySmall,
        )
        Text(
            formatPrice(bar.close),
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodySmall,
        )
        Text(
            formatPct(bar.changePct),
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodySmall,
            color = QuoteColors.of(bar.changePct),
            fontWeight = FontWeight.Medium,
        )
        Text(
            formatAmount(bar.amount),
            modifier = Modifier.weight(1.2f),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun typeLabel(t: String): String = when (t.uppercase(Locale.US)) {
    "INDUSTRY" -> "行业"
    "CONCEPT" -> "概念"
    "REGION" -> "地域"
    else -> t
}

private fun formatPct(v: Double): String =
    String.format(Locale.US, "%+.2f%%", v)

private fun formatPrice(v: Double): String =
    String.format(Locale.US, "%.2f", v)

private fun formatAmount(v: Double): String = when {
    v >= 1e8 -> String.format(Locale.US, "%.2f亿", v / 1e8)
    v >= 1e4 -> String.format(Locale.US, "%.0f万", v / 1e4)
    else -> String.format(Locale.US, "%.0f", v)
}
