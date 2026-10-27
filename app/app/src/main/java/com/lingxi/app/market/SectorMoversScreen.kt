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
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
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
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.SectorConstituentRow
import com.lingxi.app.data.model.SectorDetail
import com.lingxi.app.data.model.SectorMoverRow
import com.lingxi.app.ui.QuoteColors
import com.lingxi.app.viewmodel.MarketViewModel
import java.util.Locale

private enum class MoverSide { Gainers, Losers }

@Composable
fun SectorTab(
    viewModel: MarketViewModel,
    onOpenStock: (String) -> Unit = {},
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
            onOpenStock = onOpenStock,
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
    var side by rememberSaveable { mutableStateOf(MoverSide.Gainers) }

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
                        append("按当日涨跌 · 行业/概念 Top10")
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

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState())
                .padding(vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            FilterChip(
                selected = side == MoverSide.Gainers,
                onClick = { side = MoverSide.Gainers },
                label = { Text("领涨") },
            )
            FilterChip(
                selected = side == MoverSide.Losers,
                onClick = { side = MoverSide.Losers },
                label = { Text("领跌") },
            )
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 4.dp, bottom = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "#",
                modifier = Modifier.padding(end = 12.dp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "板块",
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "当日",
                modifier = Modifier.padding(start = 4.dp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "5日",
                modifier = Modifier.padding(start = 10.dp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "10日",
                modifier = Modifier.padding(start = 10.dp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        if (ui.loading && ui.data == null) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        } else if (ui.error != null && ui.data == null) {
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
        } else {
            val data = ui.data
            val rows = when (side) {
                MoverSide.Gainers -> data?.gainers.orEmpty()
                MoverSide.Losers -> data?.losers.orEmpty()
            }
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(0.dp),
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
                itemsIndexed(rows) { idx, row ->
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
                "${typeLabel(row.sectorType)} · ${formatAmount(row.amount)}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        PctCell(row.changePct, bold = true)
        PctCell(row.change5dPct)
        PctCell(row.change10dPct)
    }
}

@Composable
private fun PctCell(value: Double?, bold: Boolean = false) {
    Text(
        value?.let { formatPct(it) } ?: "—",
        modifier = Modifier.padding(start = 10.dp),
        style = MaterialTheme.typography.bodySmall.copy(
            fontSize = 12.sp,
            fontWeight = if (bold) FontWeight.SemiBold else FontWeight.Normal,
        ),
        color = value?.let { QuoteColors.of(it) } ?: MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
fun SectorDetailScreen(
    viewModel: MarketViewModel,
    symbol: String,
    onBack: () -> Unit,
    onOpenStock: (String) -> Unit = {},
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

        if (ui.loading && ui.data == null) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        } else if (ui.error != null && ui.data == null) {
            Text(
                ui.error ?: "加载失败",
                modifier = Modifier.padding(16.dp),
                color = MaterialTheme.colorScheme.error,
            )
        } else if (ui.data != null) {
            SectorDetailBody(
                d = ui.data!!,
                onOpenStock = onOpenStock,
            )
        }
    }
}

@Composable
private fun SectorDetailBody(
    d: SectorDetail,
    onOpenStock: (String) -> Unit,
) {
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
        item {
            Text(
                if (d.constituents.isEmpty()) "成分股（暂无数据）"
                else "成分股 ${d.constituents.size}",
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
            )
        }
        if (d.constituents.isNotEmpty()) {
            item {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        "名称",
                        modifier = Modifier.weight(1f),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        "现价",
                        modifier = Modifier.padding(start = 6.dp),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        "当日",
                        modifier = Modifier.padding(start = 8.dp),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        "5日",
                        modifier = Modifier.padding(start = 8.dp),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        "10日",
                        modifier = Modifier.padding(start = 8.dp),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            itemsIndexed(d.constituents) { idx, row ->
                if (idx > 0) {
                    HorizontalDivider(
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f),
                    )
                }
                ConstituentRowItem(row = row, onClick = { onOpenStock(row.symbol) })
            }
        }
    }
}

@Composable
private fun ConstituentRowItem(
    row: SectorConstituentRow,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                row.name.ifBlank { row.symbol },
                fontWeight = FontWeight.Medium,
                maxLines = 1,
            )
            Text(
                row.symbol,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            row.close?.let { formatPrice(it) } ?: "—",
            modifier = Modifier.padding(start = 6.dp),
            style = MaterialTheme.typography.bodySmall,
        )
        PctCell(row.changePct, bold = true)
        PctCell(row.change5dPct)
        PctCell(row.change10dPct)
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
