package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.FundStatsMeta
import com.lingxi.app.data.model.FundStatsResult
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.PeriodOption
import com.lingxi.app.data.model.SectorDetail
import com.lingxi.app.data.model.SectorMovers
import com.lingxi.app.data.model.ServerConfig
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter

data class MarketUiState(
    val loading: Boolean = false,
    val result: MsiResult? = null,
    val mtt: MttResult? = null,
    val error: String? = null,
    val serverHint: String = "",
    /** 最近一次 MSI/MTT 均成功拉取的上海时间文案；从未成功则为 null。 */
    val lastSuccessAt: String? = null,
    val refreshIntervalMinutes: Int = 5,
)

data class FundStatsUiState(
    val loading: Boolean = false,
    val meta: FundStatsMeta? = null,
    val result: FundStatsResult? = null,
    val selectedYear: Int? = null,
    val selectedPeriod: String = "FY",
    val error: String? = null,
)

data class SectorMoversUiState(
    val loading: Boolean = false,
    val data: SectorMovers? = null,
    val error: String? = null,
)

data class SectorDetailUiState(
    val loading: Boolean = false,
    val data: SectorDetail? = null,
    val error: String? = null,
    val symbol: String? = null,
)

class MarketViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    val config: StateFlow<ServerConfig> = prefs.configFlow
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ServerConfig())

    private val _ui = MutableStateFlow(MarketUiState())
    val ui: StateFlow<MarketUiState> = _ui.asStateFlow()

    private val _fundStats = MutableStateFlow(FundStatsUiState())
    val fundStats: StateFlow<FundStatsUiState> = _fundStats.asStateFlow()

    private val _sectorMovers = MutableStateFlow(SectorMoversUiState())
    val sectorMovers: StateFlow<SectorMoversUiState> = _sectorMovers.asStateFlow()

    private val _sectorDetail = MutableStateFlow(SectorDetailUiState())
    val sectorDetail: StateFlow<SectorDetailUiState> = _sectorDetail.asStateFlow()

    private var autoRefreshJob: Job? = null
    private val shanghaiZone = ZoneId.of("Asia/Shanghai")
    private val timeFmt = DateTimeFormatter.ofPattern("HH:mm:ss")

    init {
        viewModelScope.launch {
            prefs.configFlow
                .map { it.refreshIntervalMinutes.coerceIn(1, 60) }
                .distinctUntilChanged()
                .collect { minutes ->
                    _ui.value = _ui.value.copy(refreshIntervalMinutes = minutes)
                    restartAutoRefresh(minutes)
                }
        }
        refresh()
    }

    fun loadFundStatsIfNeeded() {
        if (_fundStats.value.meta != null || _fundStats.value.loading) return
        refreshFundStats()
    }

    fun selectFundPeriod(period: String) {
        if (period == _fundStats.value.selectedPeriod) return
        // 换期次后年份列表变化，交给后端选该期次最新年
        _fundStats.value = _fundStats.value.copy(
            selectedPeriod = period,
            selectedYear = null,
        )
        refreshFundStats(keepSelection = true)
    }

    fun selectFundYear(year: Int) {
        if (year == _fundStats.value.selectedYear) return
        _fundStats.value = _fundStats.value.copy(selectedYear = year)
        refreshFundStats(keepSelection = true)
    }

    fun refreshFundStats(keepSelection: Boolean = false) {
        viewModelScope.launch {
            val cfg = prefs.configFlow.first()
            val prev = _fundStats.value
            _fundStats.value = prev.copy(loading = true, error = null)
            try {
                val api = ApiClient.create(cfg)
                val meta = if (prev.meta == null || !keepSelection) {
                    runCatching { api.fundStatsMeta() }.getOrNull() ?: prev.meta
                } else {
                    prev.meta
                }
                val period = when {
                    keepSelection -> prev.selectedPeriod
                    meta != null -> meta.defaultPeriod
                    else -> prev.selectedPeriod
                }
                val year = when {
                    keepSelection -> prev.selectedYear // 可为 null，后端按该期次最新年
                    meta?.defaultYear != null -> meta.defaultYear
                    else -> prev.selectedYear
                }
                val result = api.fundStats(year = year, period = period)
                val years = result.availableYears.ifEmpty {
                    meta?.availableYears.orEmpty()
                }
                _fundStats.value = FundStatsUiState(
                    loading = false,
                    meta = meta?.copy(availableYears = years)
                        ?: FundStatsMeta(
                            periods = listOf(
                                PeriodOption("Q1", "一季报"),
                                PeriodOption("H1", "中报"),
                                PeriodOption("Q3", "三季报"),
                                PeriodOption("FY", "年报"),
                            ),
                            defaultYear = result.year,
                            defaultPeriod = result.period,
                            availableYears = years,
                        ),
                    result = result,
                    selectedYear = result.year,
                    selectedPeriod = result.period,
                    error = null,
                )
            } catch (e: Exception) {
                _fundStats.value = prev.copy(
                    loading = false,
                    error = e.message ?: e.javaClass.simpleName,
                )
            }
        }
    }

    fun refresh(days: Int = 5) {
        viewModelScope.launch {
            val cfg = prefs.configFlow.first()
            _ui.value = _ui.value.copy(
                loading = true,
                error = null,
                serverHint = cfg.httpBaseUrl(),
                refreshIntervalMinutes = cfg.refreshIntervalMinutes.coerceIn(1, 60),
            )
            try {
                val api = ApiClient.create(cfg)
                val msiDeferred = async {
                    runCatching { api.sentiment(days = days) }
                }
                val mttDeferred = async {
                    runCatching { api.midTermTrend() }
                }
                val msiResult = msiDeferred.await()
                val mttResult = mttDeferred.await()

                val errors = buildList {
                    msiResult.exceptionOrNull()?.let {
                        add("短线：${it.message ?: it.javaClass.simpleName}")
                    }
                    mttResult.exceptionOrNull()?.let {
                        add("中期：${it.message ?: it.javaClass.simpleName}")
                    }
                }

                val bothOk = msiResult.isSuccess && mttResult.isSuccess
                val successAt = if (bothOk) {
                    ZonedDateTime.now(shanghaiZone).format(timeFmt)
                } else {
                    _ui.value.lastSuccessAt
                }

                _ui.value = MarketUiState(
                    loading = false,
                    result = msiResult.getOrElse { _ui.value.result },
                    mtt = mttResult.getOrElse { _ui.value.mtt },
                    error = errors.takeIf { it.isNotEmpty() }?.joinToString("；"),
                    serverHint = cfg.httpBaseUrl(),
                    lastSuccessAt = successAt,
                    refreshIntervalMinutes = cfg.refreshIntervalMinutes.coerceIn(1, 60),
                )
            } catch (e: Exception) {
                _ui.value = MarketUiState(
                    loading = false,
                    result = _ui.value.result,
                    mtt = _ui.value.mtt,
                    error = e.message ?: e.javaClass.simpleName,
                    serverHint = cfg.httpBaseUrl(),
                    lastSuccessAt = _ui.value.lastSuccessAt,
                    refreshIntervalMinutes = cfg.refreshIntervalMinutes.coerceIn(1, 60),
                )
            }
        }
    }

    fun loadSectorMoversIfNeeded() {
        if (_sectorMovers.value.data != null || _sectorMovers.value.loading) return
        refreshSectorMovers()
    }

    fun refreshSectorMovers() {
        viewModelScope.launch {
            val cfg = prefs.configFlow.first()
            val prev = _sectorMovers.value
            _sectorMovers.value = prev.copy(loading = true, error = null)
            try {
                val api = ApiClient.create(cfg)
                val data = api.sectorMovers(topN = 10, types = "INDUSTRY,CONCEPT")
                _sectorMovers.value = SectorMoversUiState(loading = false, data = data)
            } catch (e: Exception) {
                _sectorMovers.value = prev.copy(
                    loading = false,
                    error = e.message ?: e.javaClass.simpleName,
                )
            }
        }
    }

    fun clearSectorDetail() {
        _sectorDetail.value = SectorDetailUiState()
    }

    fun loadSectorDetail(symbol: String) {
        val sym = symbol.trim()
        if (sym.isEmpty()) return
        viewModelScope.launch {
            val cfg = prefs.configFlow.first()
            _sectorDetail.value = SectorDetailUiState(loading = true, symbol = sym)
            try {
                val api = ApiClient.create(cfg)
                val data = api.sectorDetail(sym)
                _sectorDetail.value = SectorDetailUiState(loading = false, data = data, symbol = sym)
            } catch (e: Exception) {
                _sectorDetail.value = SectorDetailUiState(
                    loading = false,
                    error = e.message ?: e.javaClass.simpleName,
                    symbol = sym,
                )
            }
        }
    }

    private fun restartAutoRefresh(minutes: Int) {
        autoRefreshJob?.cancel()
        autoRefreshJob = viewModelScope.launch {
            val periodMs = minutes.coerceIn(1, 60) * 60_000L
            while (isActive) {
                delay(periodMs)
                if (!_ui.value.loading) {
                    refresh()
                }
            }
        }
    }
}
