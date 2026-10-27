package com.lingxi.app.condition

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.BalancePeriodRow
import com.lingxi.app.data.model.CashflowPeriodRow
import com.lingxi.app.data.model.DetailSummary
import com.lingxi.app.data.model.DimAi
import com.lingxi.app.data.model.FundamentalsDim
import com.lingxi.app.data.model.IncomePeriodRow
import com.lingxi.app.data.model.ShortTermDim
import com.lingxi.app.data.model.StatementsDim
import com.lingxi.app.data.model.StockAnalyze
import com.lingxi.app.data.model.StockAnalyzeQuote
import com.lingxi.app.data.model.StockBar
import com.lingxi.app.data.model.StockSearchHit
import com.lingxi.app.ui.QuoteColors
import com.lingxi.app.viewmodel.ScreenViewModel
import java.util.Locale

private enum class AnalyzeDim(val label: String, val apiKey: String) {
    Tape("盘面", "tape"),
    ShortTerm("短线走势", "short_term"),
    Fundament("基本面", "fundamentals"),
    Statements("财务报表", "statements"),
}

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
    val aiLoadingDim by viewModel.stockAnalyzeAiLoadingDim.collectAsStateWithLifecycle()
    val ticksLoading by viewModel.stockAnalyzeTicksLoading.collectAsStateWithLifecycle()
    val query by viewModel.stockSearchQuery.collectAsStateWithLifecycle()
    val hits by viewModel.stockSearchHits.collectAsStateWithLifecycle()
    val searchLoading by viewModel.stockSearchLoading.collectAsStateWithLifecycle()
    val searchHistory by viewModel.analyzeSearchHistory.collectAsStateWithLifecycle()

    var selectedSymbol by remember { mutableStateOf(initialSymbol.orEmpty()) }
    var selectedName by remember { mutableStateOf("") }
    var dimTab by rememberSaveable { mutableIntStateOf(0) }
    // 已选股时默认收起搜索，把可视区让给分析；点「换股」再展开
    var searchOpen by rememberSaveable {
        mutableStateOf(initialSymbol.isNullOrBlank())
    }

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

    // 切换 Tab 或事实就绪后，懒加载当前维 AI（盘面等明细补全后再请求）
    LaunchedEffect(result?.symbol, result?.aiEnabled, result?.detailStatus, dimTab, ticksLoading) {
        val r = result ?: return@LaunchedEffect
        if (r.symbol.isBlank() || !r.aiEnabled) return@LaunchedEffect
        val dim = AnalyzeDim.entries[dimTab.coerceIn(0, AnalyzeDim.entries.lastIndex)]
        if (dim == AnalyzeDim.Tape && (ticksLoading || r.detailStatus == "pending")) {
            return@LaunchedEffect
        }
        viewModel.loadStockAnalyzeAi(r.symbol, dim.apiKey)
    }

    val displayName = selectedName
        .ifBlank { result?.name.orEmpty() }
        .ifBlank { selectedSymbol.ifBlank { "AI 分析" } }
    val displaySymbol = result?.symbol?.takeIf { it.isNotBlank() } ?: selectedSymbol
    val hasStock = selectedSymbol.isNotBlank()

    fun selectHit(hit: StockSearchHit) {
        selectedSymbol = hit.symbol
        selectedName = hit.name
        dimTab = 0
        searchOpen = false
        viewModel.onStockSearchQueryChange("")
        viewModel.rememberAnalyzeStock(hit)
        viewModel.loadStockAnalyze(hit.symbol)
    }

    val showSearchPanel =
        !hasStock || searchOpen || query.isNotBlank() || hits.isNotEmpty()
    val showHistory =
        showSearchPanel && query.isBlank() && hits.isEmpty() && searchHistory.isNotEmpty()

    val body: @Composable (PaddingValues) -> Unit = { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            // 吸顶仅保留：换股搜索（按需）或一行精简身份条 + Tab
            if (showSearchPanel) {
                StockSearchBar(
                    query = query,
                    loading = searchLoading,
                    onQueryChange = {
                        searchOpen = true
                        viewModel.onStockSearchQueryChange(it)
                    },
                    onClear = { viewModel.onStockSearchQueryChange("") },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
                if (hasStock && query.isBlank() && hits.isEmpty()) {
                    Text(
                        "收起搜索",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier
                            .padding(horizontal = 20.dp)
                            .padding(bottom = 4.dp)
                            .clickable { searchOpen = false },
                    )
                }
                if (hits.isNotEmpty()) {
                    SearchHitList(
                        hits = hits,
                        onSelect = ::selectHit,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp)
                            .padding(bottom = 4.dp),
                    )
                } else if (showHistory) {
                    SearchHistoryList(
                        items = searchHistory,
                        onSelect = ::selectHit,
                        onRemove = viewModel::removeAnalyzeHistory,
                        onClearAll = viewModel::clearAnalyzeHistory,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp)
                            .padding(bottom = 4.dp),
                    )
                }
            } else if (hasStock) {
                CompactStockChrome(
                    name = displayName,
                    symbol = displaySymbol,
                    quote = result?.quote,
                    loading = loading && result == null,
                    showRefresh = result != null || error != null,
                    refreshLoading = loading && result != null,
                    onSearch = { searchOpen = true },
                    onRefresh = { viewModel.loadStockAnalyze(selectedSymbol) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }

            if (hasStock && !showSearchPanel) {
                AnalyzeDimTabs(
                    selected = dimTab,
                    onSelect = { dimTab = it },
                    modifier = Modifier.fillMaxWidth(),
                )
            } else if (hasStock && showSearchPanel && hits.isEmpty()) {
                // 搜索展开但无结果列表时仍保留 Tab，避免整页被搜索占满
                AnalyzeDimTabs(
                    selected = dimTab,
                    onSelect = { dimTab = it },
                    modifier = Modifier.fillMaxWidth(),
                )
            }

            when {
                !hasStock && hits.isEmpty() && !showHistory -> {
                    EmptyHint(modifier = Modifier.weight(1f))
                }
                (!hasStock && hits.isNotEmpty()) || (!hasStock && showHistory) -> {
                    Spacer(modifier = Modifier.weight(1f))
                }
                hasStock && loading && result == null -> {
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth(),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(36.dp))
                        Spacer(modifier = Modifier.height(14.dp))
                        Text(
                            "正在拉取行情与盘面数据…",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                hasStock && error != null && result == null -> {
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth()
                            .padding(16.dp),
                    ) {
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
                hasStock && result != null -> {
                    val r = result!!
                    val dim = AnalyzeDim.entries[dimTab.coerceIn(0, AnalyzeDim.entries.lastIndex)]
                    val aiBusy = aiLoadingDim == dim.apiKey
                    when (dim) {
                        AnalyzeDim.Tape -> TapeTabContent(
                            result = r,
                            aiLoading = aiBusy,
                            ticksLoading = ticksLoading,
                            quote = r.quote,
                            onRetryAi = {
                                viewModel.loadStockAnalyzeAi(r.symbol, dim.apiKey, force = true)
                            },
                            onRetryTicks = { viewModel.loadStockAnalyzeTicks(r.symbol) },
                            modifier = Modifier.weight(1f),
                        )
                        AnalyzeDim.ShortTerm -> ShortTermTabContent(
                            dim = r.shortTerm,
                            aiEnabled = r.aiEnabled,
                            aiModel = r.aiModel,
                            aiLoading = aiBusy,
                            quote = r.quote,
                            onRetryAi = {
                                viewModel.loadStockAnalyzeAi(r.symbol, dim.apiKey, force = true)
                            },
                            modifier = Modifier.weight(1f),
                        )
                        AnalyzeDim.Fundament -> FundamentalsTabContent(
                            dim = r.fundamentals,
                            aiEnabled = r.aiEnabled,
                            aiModel = r.aiModel,
                            aiLoading = aiBusy,
                            quote = r.quote,
                            onRetryAi = {
                                viewModel.loadStockAnalyzeAi(r.symbol, dim.apiKey, force = true)
                            },
                            modifier = Modifier.weight(1f),
                        )
                        AnalyzeDim.Statements -> StatementsTabContent(
                            dim = r.statements,
                            aiEnabled = r.aiEnabled,
                            aiModel = r.aiModel,
                            aiLoading = aiBusy,
                            quote = r.quote,
                            onRetryAi = {
                                viewModel.loadStockAnalyzeAi(r.symbol, dim.apiKey, force = true)
                            },
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
                else -> Spacer(modifier = Modifier.weight(1f))
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
private fun AnalyzeDimTabs(
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    ScrollableTabRow(
        selectedTabIndex = selected,
        modifier = modifier,
        edgePadding = 12.dp,
        containerColor = MaterialTheme.colorScheme.background,
        divider = {
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f),
            )
        },
    ) {
        AnalyzeDim.entries.forEachIndexed { index, dim ->
            Tab(
                selected = selected == index,
                onClick = { onSelect(index) },
                text = {
                    Text(
                        dim.label,
                        fontWeight = if (selected == index) FontWeight.SemiBold else FontWeight.Normal,
                    )
                },
            )
        }
    }
}

/** 已选股时的精简吸顶条：名称 + 涨跌 + 换股/刷新，完整报价随列表滚动。 */
@Composable
private fun CompactStockChrome(
    name: String,
    symbol: String,
    quote: StockAnalyzeQuote?,
    loading: Boolean,
    showRefresh: Boolean,
    refreshLoading: Boolean,
    onSearch: () -> Unit,
    onRefresh: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                name.ifBlank { "—" },
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (symbol.isNotBlank()) {
                    Text(
                        symbol,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (loading && quote == null) {
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        "行情加载中…",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        if (quote != null) {
            val chg = QuoteColors.of(quote.changePct)
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    formatAnalyzePrice(quote.current),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = chg,
                )
                Text(
                    formatAnalyzePct(quote.changePct),
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = chg,
                )
            }
            Spacer(modifier = Modifier.width(4.dp))
        }
        IconButton(onClick = onSearch) {
            Icon(Icons.Default.Search, contentDescription = "换股搜索")
        }
        if (showRefresh) {
            IconButton(onClick = onRefresh, enabled = !refreshLoading && symbol.isNotBlank()) {
                if (refreshLoading) {
                    CircularProgressIndicator(modifier = Modifier.size(22.dp), strokeWidth = 2.dp)
                } else {
                    Icon(Icons.Default.Refresh, contentDescription = "刷新")
                }
            }
        }
    }
}

@Composable
private fun TapeTabContent(
    result: StockAnalyze,
    aiLoading: Boolean,
    ticksLoading: Boolean,
    quote: StockAnalyzeQuote? = null,
    onRetryAi: () -> Unit = {},
    onRetryTicks: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (quote != null) {
            item { DayQuotePanel(quote) }
        }
        item {
            MetaPanel(
                tradeDate = result.tradeDate,
                lastTime = result.detailLastTime,
                tickCount = result.detailTickCount,
                source = result.detailSource,
                updatedAt = result.detailUpdatedAt,
            )
        }
        item {
            DetailSummaryPanel(
                summary = result.detailSummary,
                status = result.detailStatus,
                loading = ticksLoading,
                onRetry = onRetryTicks,
            )
        }
        item {
            AnalysisPanel(
                title = "盘面 AI",
                ai = result.ai,
                aiEnabled = result.aiEnabled,
                aiModel = result.aiModel,
                loading = aiLoading,
                onRetry = onRetryAi,
            )
        }
        if (result.bars.isNotEmpty()) {
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
                ) {
                    Column {
                        result.bars.asReversed().forEachIndexed { idx, bar ->
                            if (idx > 0) {
                                HorizontalDivider(
                                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                                )
                            }
                            BarRow(bar)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ShortTermTabContent(
    dim: ShortTermDim,
    aiEnabled: Boolean,
    aiModel: String?,
    aiLoading: Boolean,
    quote: StockAnalyzeQuote? = null,
    onRetryAi: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (quote != null) {
            item { DayQuotePanel(quote) }
        }
        item {
            Surface(
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        "短线指标",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                    QuoteGrid(
                        listOf(
                            "5日涨跌" to formatOptPct(dim.change5dPct),
                            "10日涨跌" to formatOptPct(dim.change10dPct),
                            "20日涨跌" to formatOptPct(dim.change20dPct),
                            "MA5" to formatOptPrice(dim.ma5),
                            "MA10" to formatOptPrice(dim.ma10),
                            "MA20" to formatOptPrice(dim.ma20),
                            "额/5日均" to (dim.amountVs5dAvg?.let {
                                String.format(Locale.US, "%.2fx", it)
                            } ?: "—"),
                            "距MA5" to formatOptPct(dim.priceVsMa5Pct),
                            "量能" to dim.volumeLabel,
                            "RSI(14)" to (dim.rsi14?.let {
                                String.format(Locale.US, "%.1f", it)
                            } ?: "—"),
                            "RSI区间" to when (dim.rsiZone) {
                                "overbought" -> "超买"
                                "oversold" -> "超卖"
                                "neutral" -> "中性"
                                else -> "—"
                            },
                            "RSIΔ5" to (dim.rsiDelta5?.let {
                                String.format(Locale.US, "%+.1f", it)
                            } ?: "—"),
                            "近10超买日" to dim.rsiDaysOb.toString(),
                            "近10超卖日" to dim.rsiDaysOs.toString(),
                            "穿越" to when {
                                dim.rsiCrossedOversold && dim.rsiCrossedOverbought -> "上穿超卖/下破超买"
                                dim.rsiCrossedOversold -> "上穿超卖"
                                dim.rsiCrossedOverbought -> "下破超买"
                                else -> "—"
                            },
                        ),
                    )
                    if (dim.labels.isNotEmpty()) {
                        Spacer(modifier = Modifier.height(10.dp))
                        FlagChips(dim.labels)
                    }
                }
            }
        }
        item {
            AnalysisPanel(
                title = "短线 AI",
                ai = dim.ai,
                aiEnabled = aiEnabled,
                aiModel = aiModel,
                loading = aiLoading,
                onRetry = onRetryAi,
            )
        }
        if (dim.bars.isNotEmpty()) {
            item {
                Text(
                    "近20日行情",
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                )
            }
            item {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                ) {
                    Column {
                        dim.bars.asReversed().forEachIndexed { idx, bar ->
                            if (idx > 0) {
                                HorizontalDivider(
                                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                                )
                            }
                            BarRow(bar)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun FundamentalsTabContent(
    dim: FundamentalsDim,
    aiEnabled: Boolean,
    aiModel: String?,
    aiLoading: Boolean,
    quote: StockAnalyzeQuote? = null,
    onRetryAi: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (quote != null) {
            item { DayQuotePanel(quote) }
        }
        item {
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
                    ) {
                        Text(
                            "基本面快照",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.SemiBold,
                        )
                        val period = listOfNotNull(
                            dim.reportDate?.takeIf { it.isNotBlank() },
                            dim.fiscalPeriod?.takeIf { it.isNotBlank() },
                        ).joinToString(" · ")
                        if (period.isNotBlank()) {
                            Text(
                                period,
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    Spacer(modifier = Modifier.height(12.dp))
                    QuoteGrid(
                        listOf(
                            "PE" to formatOptNum(dim.pe),
                            "PB" to formatOptNum(dim.pb),
                            "流通值" to formatOptYi(dim.floatMv),
                            "总市值" to formatOptYi(dim.totalMv),
                            "营收" to formatOptMoney(dim.revenue),
                            "营收同比" to formatOptPct(dim.revenueYoy),
                            "归母净利" to formatOptMoney(dim.parentNp),
                            "净利同比" to formatOptPct(dim.parentNpYoy),
                            "扣非净利" to formatOptMoney(dim.deductNp),
                            "经营现金流" to formatOptMoney(dim.operatingCf),
                            "经营同比" to formatOptPct(dim.operatingCfYoy),
                            "应收账款" to formatOptMoney(dim.receivable),
                            "总资产" to formatOptMoney(dim.totalAssets),
                            "总负债" to formatOptMoney(dim.totalLiabilities),
                            "净资产" to formatOptMoney(dim.totalEquity),
                            "现金流/净利" to (dim.ocfVsNp?.let {
                                String.format(Locale.US, "%.2fx", it)
                            } ?: "—"),
                            "资产负债率" to (dim.liabilityRatio?.let {
                                String.format(Locale.US, "%.1f%%", it * 100)
                            } ?: "—"),
                        ),
                    )
                    if (dim.qualityFlags.isNotEmpty()) {
                        Spacer(modifier = Modifier.height(10.dp))
                        FlagChips(dim.qualityFlags)
                    }
                }
            }
        }
        item {
            AnalysisPanel(
                title = "基本面 AI",
                ai = dim.ai,
                aiEnabled = aiEnabled,
                aiModel = aiModel,
                loading = aiLoading,
                onRetry = onRetryAi,
            )
        }
    }
}

@Composable
private fun StatementsTabContent(
    dim: StatementsDim,
    aiEnabled: Boolean,
    aiModel: String?,
    aiLoading: Boolean,
    quote: StockAnalyzeQuote? = null,
    onRetryAi: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (quote != null) {
            item { DayQuotePanel(quote) }
        }
        if (dim.trendFlags.isNotEmpty()) {
            item {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(
                            "趋势标签",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.SemiBold,
                        )
                        Spacer(modifier = Modifier.height(10.dp))
                        FlagChips(dim.trendFlags)
                    }
                }
            }
        }
        item {
            AnalysisPanel(
                title = "财报 AI",
                ai = dim.ai,
                aiEnabled = aiEnabled,
                aiModel = aiModel,
                loading = aiLoading,
                onRetry = onRetryAi,
            )
        }
        if (dim.income.isNotEmpty()) {
            item { SectionTitle("利润表（近几期 · 亿/万元）") }
            item { IncomeTable(dim.income) }
        }
        if (dim.cashflow.isNotEmpty()) {
            item { SectionTitle("现金流量表（近几期 · 亿/万元）") }
            item { CashflowTable(dim.cashflow) }
        }
        if (dim.balance.isNotEmpty()) {
            item { SectionTitle("资产负债表（近几期 · 亿/万元）") }
            item { BalanceTable(dim.balance) }
        }
        if (dim.income.isEmpty() && dim.cashflow.isEmpty() && dim.balance.isEmpty()) {
            item {
                Text(
                    "暂无财务报表数据",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
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
private fun IncomeTable(rows: List<IncomePeriodRow>) {
    StatementSurface {
        StatementHeader(listOf("报告期", "营收", "归母", "扣非"))
        rows.forEach { r ->
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f))
            StatementMetricRow(
                period = periodLabel(r.reportDate, r.fiscalPeriod),
                metrics = listOf(
                    r.revenue to r.revenueYoy,
                    r.parentNp to r.parentNpYoy,
                    r.deductNp to r.deductNpYoy,
                ),
            )
        }
    }
}

@Composable
private fun CashflowTable(rows: List<CashflowPeriodRow>) {
    StatementSurface {
        StatementHeader(listOf("报告期", "经营", "投资", "筹资"))
        rows.forEach { r ->
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f))
            StatementMetricRow(
                period = periodLabel(r.reportDate, r.fiscalPeriod),
                metrics = listOf(
                    r.operating to r.operatingYoy,
                    r.investing to r.investingYoy,
                    r.financing to r.financingYoy,
                ),
            )
        }
    }
}

@Composable
private fun BalanceTable(rows: List<BalancePeriodRow>) {
    StatementSurface {
        StatementHeader(listOf("报告期", "总资产", "总负债", "净资产"))
        rows.forEach { r ->
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f))
            StatementMetricRow(
                period = periodLabel(r.reportDate, r.fiscalPeriod),
                metrics = listOf(
                    r.totalAssets to r.totalAssetsYoy,
                    r.totalLiabilities to r.totalLiabilitiesYoy,
                    r.totalEquity to r.totalEquityYoy,
                ),
            )
        }
    }
}

@Composable
private fun StatementSurface(content: @Composable () -> Unit) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
    ) {
        Column(content = { content() })
    }
}

@Composable
private fun StatementHeader(cols: List<String>) {
    Row(modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
        cols.forEachIndexed { i, c ->
            Text(
                c,
                modifier = Modifier.weight(if (i == 0) 1.15f else 1f),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun StatementMetricRow(
    period: String,
    metrics: List<Pair<Double?, Double?>>,
) {
    Row(
        modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Text(
            period,
            modifier = Modifier.weight(1.15f),
            style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.Medium,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        metrics.forEach { (amount, yoy) ->
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    formatStatementMoney(amount),
                    style = MaterialTheme.typography.bodySmall,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    formatStatementYoy(yoy),
                    style = MaterialTheme.typography.labelSmall,
                    color = statementYoyColor(yoy),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

private fun periodLabel(date: String, period: String?): String {
    val d = date.takeLast(5).ifBlank { date }
    val p = period?.takeIf { it.isNotBlank() }
    return if (p != null) "$d\n$p" else d
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
        modifier = modifier,
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SearchHistoryList(
    items: List<StockSearchHit>,
    onSelect: (StockSearchHit) -> Unit,
    onRemove: (String) -> Unit,
    onClearAll: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val chipBg = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f)
    val chipBorder = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.55f)
    Column(modifier = modifier.padding(top = 2.dp, bottom = 6.dp)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 2.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Default.History,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.85f),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                "最近分析",
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            Text(
                "清空",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.75f),
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .clickable(onClick = onClearAll)
                    .padding(horizontal = 8.dp, vertical = 4.dp),
            )
        }
        Spacer(modifier = Modifier.height(10.dp))
        FlowRow(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items.take(16).forEach { hit ->
                val title = hit.name.ifBlank { hit.symbol }
                val code = hit.symbol.ifBlank { hit.code }
                Row(
                    modifier = Modifier
                        .clip(RoundedCornerShape(10.dp))
                        .background(chipBg)
                        .border(1.dp, chipBorder, RoundedCornerShape(10.dp))
                        .clickable { onSelect(hit) }
                        .padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.padding(end = 2.dp)) {
                        Text(
                            title,
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.SemiBold,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (code.isNotBlank() && code != title) {
                            Text(
                                code,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.8f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                    Icon(
                        Icons.Default.Close,
                        contentDescription = "删除",
                        modifier = Modifier
                            .size(28.dp)
                            .clip(RoundedCornerShape(6.dp))
                            .clickable { onRemove(hit.symbol) }
                            .padding(6.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f),
                    )
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
        Column(modifier = Modifier.padding(14.dp)) {
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
            Spacer(modifier = Modifier.height(12.dp))
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
private fun DetailSummaryPanel(
    summary: DetailSummary,
    status: String,
    loading: Boolean,
    onRetry: () -> Unit = {},
) {
    val s = summary
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "盘面摘要",
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                if (loading) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        "拉取明细中…",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Spacer(modifier = Modifier.height(12.dp))
            if (loading && status == "pending" && s.tickCount == 0) {
                Text(
                    "报价与其它维度已就绪，正在后台拉取成交明细（东财/腾讯翻页较慢）。",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else if (!loading && status == "empty" && s.tickCount == 0) {
                Text(
                    "暂无成交明细。",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    "重试拉取",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable(onClick = onRetry),
                )
            } else {
                QuoteGrid(
                    listOf(
                        "买盘占比" to (s.buyRatio?.let { String.format(Locale.US, "%.0f%%", it * 100) } ?: "—"),
                        "卖盘占比" to (s.sellRatio?.let { String.format(Locale.US, "%.0f%%", it * 100) } ?: "—"),
                        "VWAP" to (s.vwapEst?.let { formatAnalyzePrice(it) } ?: "—"),
                        "价相对VWAP" to formatOptPct(s.priceVsVwapPct),
                        "大单净手数" to String.format(Locale.US, "%.0f", s.largeNetLots),
                        "大买后跟价" to formatOptPct(s.largeBuyFollowPct),
                        "开盘买/卖" to String.format(Locale.US, "%.0f/%.0f", s.openBuyLots, s.openSellLots),
                        "尾盘买/卖" to String.format(Locale.US, "%.0f/%.0f", s.closeBuyLots, s.closeSellLots),
                    ),
                )
                if (s.labels.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(10.dp))
                    FlagChips(s.labels)
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FlagChips(flags: List<String>) {
    FlowRow(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        flags.take(6).forEach { flag ->
            Text(
                flag,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .background(MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f))
                    .padding(horizontal = 8.dp, vertical = 4.dp),
            )
        }
    }
}

@Composable
private fun AnalysisPanel(
    ai: DimAi,
    aiEnabled: Boolean,
    aiModel: String?,
    loading: Boolean,
    title: String = "盘面 AI",
    onRetry: () -> Unit = {},
) {
    val biasLabel = when (ai.bias) {
        "bullish" -> "偏多"
        "bearish" -> "偏空"
        "neutral" -> "中性"
        else -> null
    }
    val confLabel = when (ai.confidence) {
        "high" -> "高置信"
        "medium" -> "中置信"
        "low" -> "低置信"
        else -> null
    }
    val body = when {
        loading && ai.text.isBlank() -> "正在生成解读…"
        ai.status == "pending" && aiEnabled -> "准备生成解读…"
        ai.status == "disabled" -> ai.text.ifBlank { "AI 未启用" }
        ai.status == "error" -> ai.text.ifBlank { "AI 失败" }
        else -> ai.text.ifBlank { "暂无分析内容" }
    }
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
                    title,
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
            if (biasLabel != null || confLabel != null || ai.flags.isNotEmpty()) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    biasLabel?.let {
                        val c = when (ai.bias) {
                            "bullish" -> QuoteColors.Up
                            "bearish" -> QuoteColors.Down
                            else -> MaterialTheme.colorScheme.onSurfaceVariant
                        }
                        Text(
                            it,
                            style = MaterialTheme.typography.labelLarge,
                            fontWeight = FontWeight.SemiBold,
                            color = c,
                        )
                    }
                    confLabel?.let {
                        Text(
                            it,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                if (ai.flags.isNotEmpty()) {
                    FlagChips(ai.flags)
                }
            }
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
            )
            Text(
                body,
                style = MaterialTheme.typography.bodyMedium.copy(lineHeight = 22.sp),
            )
            if (ai.status == "error" || (ai.status == "ready" && aiEnabled)) {
                Text(
                    if (ai.status == "error") "重试" else "重新生成",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable(enabled = !loading, onClick = onRetry),
                )
            }
        }
    }
}

@Composable
private fun BarRow(bar: StockBar) {
    Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                bar.date,
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
            )
            Text(
                formatAnalyzePct(bar.changePct),
                color = QuoteColors.of(bar.changePct),
                fontWeight = FontWeight.SemiBold,
            )
        }
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            "开 ${formatAnalyzePrice(bar.open)}  高 ${formatAnalyzePrice(bar.high)}  " +
                "低 ${formatAnalyzePrice(bar.low)}  收 ${formatAnalyzePrice(bar.close)}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            "额 ${formatAnalyzeAmount(bar.amount)}",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun EmptyHint(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
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

private fun formatOptPct(v: Double?): String =
    v?.let { formatAnalyzePct(it) } ?: "—"

private fun formatOptPrice(v: Double?): String =
    v?.let { formatAnalyzePrice(it) } ?: "—"

private fun formatOptNum(v: Double?): String =
    v?.let { String.format(Locale.US, "%.2f", it) } ?: "—"

/** 腾讯市值字段已是「亿」 */
private fun formatOptYi(v: Double?): String =
    v?.let { String.format(Locale.US, "%.2f亿", it) } ?: "—"

/** 财报金额（元）→ 亿/万（基本面快照等仍用自适应） */
private fun formatOptMoney(v: Double?): String =
    v?.let { formatAnalyzeAmount(it) } ?: "—"

/**
 * 财务报表金额：≥1 亿用「亿」保留 2 位；不足 1 亿用「万」保留 2 位。
 * 单位按绝对值选择，符号保留。
 */
private fun formatStatementMoney(v: Double?): String {
    if (v == null || !v.isFinite()) return "—"
    val abs = kotlin.math.abs(v)
    return if (abs >= 1e8) {
        String.format(Locale.US, "%.2f亿", v / 1e8)
    } else {
        String.format(Locale.US, "%.2f万", v / 1e4)
    }
}

private fun formatStatementYoy(yoy: Double?): String =
    yoy?.takeIf { it.isFinite() }?.let { String.format(Locale.US, "%+.2f%%", it) } ?: "—"

@Composable
private fun statementYoyColor(yoy: Double?): Color {
    val v = yoy ?: return MaterialTheme.colorScheme.onSurfaceVariant
    return when {
        v > 1e-9 -> QuoteColors.Up
        v < -1e-9 -> QuoteColors.Down
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }
}
