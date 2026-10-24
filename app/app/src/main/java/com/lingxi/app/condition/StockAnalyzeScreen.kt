package com.lingxi.app.condition

import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.StockAnalyzeQuote
import com.lingxi.app.data.model.StockSearchHit
import com.lingxi.app.ui.QuoteColors
import com.lingxi.app.viewmodel.ScreenViewModel
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StockAnalyzeScreen(
    viewModel: ScreenViewModel,
    initialSymbol: String? = null,
    showBack: Boolean = true,
    /** 嵌在首页 Tab 时不画顶栏，由 Home 提供。 */
    embedded: Boolean = false,
    onBack: () -> Unit = {},
) {
    val result by viewModel.stockAnalyze.collectAsStateWithLifecycle()
    val loading by viewModel.stockAnalyzeLoading.collectAsStateWithLifecycle()
    val error by viewModel.stockAnalyzeError.collectAsStateWithLifecycle()
    val query by viewModel.stockSearchQuery.collectAsStateWithLifecycle()
    val hits by viewModel.stockSearchHits.collectAsStateWithLifecycle()
    val searchLoading by viewModel.stockSearchLoading.collectAsStateWithLifecycle()

    var selectedSymbol by remember { mutableStateOf(initialSymbol.orEmpty()) }
    var selectedName by remember { mutableStateOf("") }

    LaunchedEffect(initialSymbol) {
        val sym = initialSymbol?.trim().orEmpty()
        if (sym.isNotEmpty()) {
            selectedSymbol = sym
            viewModel.loadStockAnalyze(sym)
        }
    }

    LaunchedEffect(result?.symbol, result?.name) {
        val r = result ?: return@LaunchedEffect
        if (r.symbol.isNotBlank()) selectedSymbol = r.symbol
        if (r.name.isNotBlank()) selectedName = r.name
    }

    val displayName = selectedName
        .ifBlank { result?.name.orEmpty() }
        .ifBlank { selectedSymbol.ifBlank { "AI 分析" } }
    val displaySymbol = result?.symbol?.takeIf { it.isNotBlank() } ?: selectedSymbol

    fun selectHit(hit: StockSearchHit) {
        selectedSymbol = hit.symbol
        selectedName = hit.name
        viewModel.onStockSearchQueryChange("")
        viewModel.loadStockAnalyze(hit.symbol)
    }

    val body: @Composable (PaddingValues) -> Unit = { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            StockSearchBar(
                query = query,
                loading = searchLoading,
                onQueryChange = viewModel::onStockSearchQueryChange,
                onClear = { viewModel.onStockSearchQueryChange("") },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 10.dp),
            )

            if (hits.isNotEmpty()) {
                SearchHitList(
                    hits = hits,
                    onSelect = ::selectHit,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f, fill = false),
                )
            }

            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                when {
                    loading && result == null && selectedSymbol.isNotBlank() -> {
                        item {
                            // 首次加载只保留中间 spinner，避免标题旁再叠一个
                            SelectedStockHeader(
                                name = displayName,
                                symbol = displaySymbol,
                                loading = false,
                                showRefresh = false,
                                onRefresh = { viewModel.loadStockAnalyze(selectedSymbol) },
                            )
                        }
                        item {
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(vertical = 40.dp),
                                horizontalAlignment = Alignment.CenterHorizontally,
                            ) {
                                CircularProgressIndicator(modifier = Modifier.size(36.dp))
                                Spacer(modifier = Modifier.height(14.dp))
                                Text(
                                    "正在拉取成交明细并分析…",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                    error != null && result == null && selectedSymbol.isNotBlank() -> {
                        item {
                            SelectedStockHeader(
                                name = displayName,
                                symbol = displaySymbol,
                                loading = false,
                                showRefresh = true,
                                onRefresh = { viewModel.loadStockAnalyze(selectedSymbol) },
                            )
                        }
                        item {
                            Surface(
                                modifier = Modifier.fillMaxWidth(),
                                shape = RoundedCornerShape(12.dp),
                                color = MaterialTheme.colorScheme.errorContainer,
                            ) {
                                Text(
                                    error.orEmpty(),
                                    color = MaterialTheme.colorScheme.onErrorContainer,
                                    modifier = Modifier.padding(14.dp),
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                            }
                        }
                    }
                    result != null -> {
                        val r = result!!
                        item {
                            SelectedStockHeader(
                                name = r.name.ifBlank { displayName },
                                symbol = r.symbol.ifBlank { displaySymbol },
                                loading = loading,
                                showRefresh = true,
                                onRefresh = { viewModel.loadStockAnalyze(selectedSymbol) },
                            )
                        }
                        r.quote?.let { q ->
                            item { DayQuotePanel(q) }
                        }
                        item {
                            MetaPanel(
                                tradeDate = r.tradeDate,
                                lastTime = r.detailLastTime,
                                tickCount = r.detailTickCount,
                                source = r.detailSource,
                                updatedAt = r.detailUpdatedAt,
                            )
                        }
                        item {
                            AnalysisPanel(
                                analysis = r.analysis,
                                aiEnabled = r.aiEnabled,
                                aiModel = r.aiModel,
                                loading = loading,
                            )
                        }
                        if (r.bars.isNotEmpty()) {
                            item {
                                Text(
                                    "近5日行情",
                                    style = MaterialTheme.typography.titleSmall,
                                    fontWeight = FontWeight.SemiBold,
                                    modifier = Modifier.padding(top = 4.dp),
                                )
                            }
                            item {
                                Surface(
                                    modifier = Modifier.fillMaxWidth(),
                                    shape = RoundedCornerShape(12.dp),
                                    color = MaterialTheme.colorScheme.surface,
                                    tonalElevation = 1.dp,
                                    shadowElevation = 0.dp,
                                ) {
                                    Column {
                                        r.bars.asReversed().forEachIndexed { idx, bar ->
                                            if (idx > 0) {
                                                HorizontalDivider(
                                                    color = MaterialTheme.colorScheme.outlineVariant.copy(
                                                        alpha = 0.5f,
                                                    ),
                                                )
                                            }
                                            BarRow(barDate = bar.date, changePct = bar.changePct,
                                                open = bar.open, high = bar.high,
                                                low = bar.low, close = bar.close,
                                                amount = bar.amount)
                                        }
                                    }
                                }
                            }
                        }
                    }
                    hits.isEmpty() && query.isBlank() -> {
                        item {
                            EmptyHint()
                        }
                    }
                    hits.isEmpty() && query.isNotBlank() && !searchLoading -> {
                        item {
                            Text(
                                "未找到匹配股票",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(vertical = 24.dp),
                            )
                        }
                    }
                }
            }
        }
    }

    if (embedded) {
        body(PaddingValues(0.dp))
    } else {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = {
                        Column {
                            Text(
                                displayName,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                fontWeight = FontWeight.SemiBold,
                            )
                            if (displaySymbol.isNotBlank() && displaySymbol != displayName) {
                                Text(
                                    displaySymbol,
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    },
                    navigationIcon = {
                        if (showBack) {
                            IconButton(onClick = onBack) {
                                Icon(
                                    Icons.AutoMirrored.Filled.ArrowBack,
                                    contentDescription = "返回",
                                )
                            }
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                )
            },
        ) { padding ->
            body(padding)
        }
    }
}

@Composable
private fun StockSearchBar(
    query: String,
    loading: Boolean,
    onQueryChange: (String) -> Unit,
    onClear: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val focusManager = LocalFocusManager.current
    val focusRequester = remember { FocusRequester() }
    val shape = RoundedCornerShape(22.dp)
    val fieldBg = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.65f)

    Row(
        modifier = modifier
            .height(44.dp)
            .clip(shape)
            .background(fieldBg)
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            Icons.Default.Search,
            contentDescription = null,
            modifier = Modifier.size(20.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.width(10.dp))
        Box(modifier = Modifier.weight(1f)) {
            if (query.isEmpty()) {
                Text(
                    "代码 / 名称 / 拼音，如 600519、茅台、gzmt",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.75f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            BasicTextField(
                value = query,
                onValueChange = onQueryChange,
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
                singleLine = true,
                textStyle = TextStyle(
                    color = MaterialTheme.colorScheme.onSurface,
                    fontSize = 15.sp,
                ),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { focusManager.clearFocus() }),
            )
        }
        when {
            loading -> {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                )
            }
            query.isNotEmpty() -> {
                IconButton(
                    onClick = onClear,
                    modifier = Modifier.size(28.dp),
                ) {
                    Icon(
                        Icons.Default.Clear,
                        contentDescription = "清除",
                        modifier = Modifier.size(18.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun SearchHitList(
    hits: List<StockSearchHit>,
    onSelect: (StockSearchHit) -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier
            .padding(horizontal = 16.dp)
            .padding(bottom = 4.dp),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 2.dp,
        shadowElevation = 2.dp,
    ) {
        Column {
            hits.take(12).forEachIndexed { index, hit ->
                if (index > 0) {
                    HorizontalDivider(
                        modifier = Modifier.padding(horizontal = 14.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f),
                    )
                }
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onSelect(hit) }
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            hit.name.ifBlank { "—" },
                            style = MaterialTheme.typography.bodyLarge,
                            fontWeight = FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        val py = hit.namePinyin.takeIf { it.isNotBlank() }
                        if (py != null) {
                            Text(
                                py.uppercase(Locale.US),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    Text(
                        hit.symbol.ifBlank { hit.code },
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = FontWeight.Medium,
                    )
                }
            }
        }
    }
}

@Composable
private fun SelectedStockHeader(
    name: String,
    symbol: String,
    loading: Boolean,
    showRefresh: Boolean = true,
    onRefresh: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                name.ifBlank { "—" },
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (symbol.isNotBlank()) {
                Text(
                    symbol,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (showRefresh) {
            IconButton(onClick = onRefresh, enabled = !loading && symbol.isNotBlank()) {
                if (loading) {
                    CircularProgressIndicator(modifier = Modifier.size(22.dp), strokeWidth = 2.dp)
                } else {
                    Icon(Icons.Default.Refresh, contentDescription = "重新分析")
                }
            }
        }
    }
}

@Composable
private fun DayQuotePanel(q: StockAnalyzeQuote) {
    val chgColor = QuoteColors.of(q.changePct)
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text(
                    formatAnalyzePrice(q.current),
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                    color = chgColor,
                )
                Spacer(modifier = Modifier.width(12.dp))
                Column {
                    Text(
                        formatAnalyzePct(q.changePct),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = chgColor,
                    )
                    Text(
                        formatSignedAmount(q.changeAmount),
                        style = MaterialTheme.typography.bodySmall,
                        color = chgColor,
                    )
                }
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    q.date.ifBlank { "—" },
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.height(14.dp))
            QuoteGrid(
                listOf(
                    "开盘" to formatAnalyzePrice(q.open),
                    "最高" to formatAnalyzePrice(q.high),
                    "最低" to formatAnalyzePrice(q.low),
                    "收盘" to formatAnalyzePrice(q.close),
                    "昨收" to formatAnalyzePrice(q.preClose),
                    "成交额" to formatAnalyzeAmount(q.amount),
                    "成交量" to formatVolumeShares(q.volume),
                    "振幅" to String.format(Locale.US, "%.2f%%", q.amplitude),
                    "换手率" to (q.turnoverRate?.let { String.format(Locale.US, "%.2f%%", it) }
                        ?: "—"),
                ),
            )
        }
    }
}

@Composable
private fun QuoteGrid(cells: List<Pair<String, String>>) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        cells.chunked(3).forEach { row ->
            Row(modifier = Modifier.fillMaxWidth()) {
                row.forEach { (label, value) ->
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            label,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(modifier = Modifier.height(2.dp))
                        Text(
                            value,
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                repeat(3 - row.size) {
                    Spacer(modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun MetaPanel(
    tradeDate: String,
    lastTime: String?,
    tickCount: Long,
    source: String,
    updatedAt: String?,
) {
    val sourceLabel = when (source) {
        "eastmoney" -> "东财"
        "tencent" -> "腾讯"
        "live" -> "实时"
        "db" -> "数据库"
        else -> source.ifBlank { "—" }
    }
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "成交明细",
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                )
                SourceChip(sourceLabel)
            }
            Spacer(modifier = Modifier.height(12.dp))
            MetaGrid(
                listOf(
                    "交易日" to tradeDate.ifBlank { "—" },
                    "最新一笔" to (lastTime?.takeIf { it.isNotBlank() } ?: "无"),
                    "笔数" to tickCount.toString(),
                    "快照" to (updatedAt?.takeIf { it.isNotBlank() }?.take(19)?.replace('T', ' ')
                        ?: "—"),
                ),
            )
        }
    }
}

@Composable
private fun SourceChip(label: String) {
    Text(
        label,
        modifier = Modifier
            .clip(RoundedCornerShape(6.dp))
            .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.1f))
            .padding(horizontal = 8.dp, vertical = 3.dp),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.primary,
        fontWeight = FontWeight.Medium,
    )
}

@Composable
private fun MetaGrid(pairs: List<Pair<String, String>>) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        pairs.chunked(2).forEach { row ->
            Row(modifier = Modifier.fillMaxWidth()) {
                row.forEach { (k, v) ->
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            k,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(modifier = Modifier.height(2.dp))
                        Text(
                            v,
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                if (row.size == 1) Spacer(modifier = Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun AnalysisPanel(
    analysis: String,
    aiEnabled: Boolean,
    aiModel: String?,
    loading: Boolean,
) {
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
                    "AI 分析",
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
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
            )
            Text(
                analysis.ifBlank { "暂无分析内容" },
                style = MaterialTheme.typography.bodyMedium.copy(lineHeight = 22.sp),
            )
        }
    }
}

@Composable
private fun BarRow(
    barDate: String,
    changePct: Double,
    open: Double,
    high: Double,
    low: Double,
    close: Double,
    amount: Double,
) {
    Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                barDate,
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
            )
            Text(
                formatAnalyzePct(changePct),
                color = QuoteColors.of(changePct),
                fontWeight = FontWeight.SemiBold,
            )
        }
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            "开 ${formatAnalyzePrice(open)}  高 ${formatAnalyzePrice(high)}  " +
                "低 ${formatAnalyzePrice(low)}  收 ${formatAnalyzePrice(close)}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            "额 ${formatAnalyzeAmount(amount)}",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun EmptyHint() {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Default.Search,
            contentDescription = null,
            modifier = Modifier.size(40.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
        )
        Spacer(modifier = Modifier.height(12.dp))
        Text(
            "搜索股票开始分析",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(6.dp))
        Text(
            "支持代码、名称或拼音首字母",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.8f),
        )
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

private fun formatVolumeShares(shares: Double): String {
    val hands = shares / 100.0
    return when {
        hands >= 1e8 -> String.format(Locale.US, "%.2f亿手", hands / 1e8)
        hands >= 1e4 -> String.format(Locale.US, "%.2f万手", hands / 1e4)
        else -> String.format(Locale.US, "%.0f手", hands)
    }
}

private fun formatSignedAmount(v: Double): String =
    String.format(Locale.US, "%+.2f", v)
