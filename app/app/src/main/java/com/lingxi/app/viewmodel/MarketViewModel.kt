package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
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
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

data class MarketUiState(
    val loading: Boolean = false,
    val result: MsiResult? = null,
    val mtt: MttResult? = null,
    val error: String? = null,
    val serverHint: String = "",
    /** 最近一次 MSI/MTT 均成功拉取的本地时间文案；从未成功则为 null。 */
    val lastSuccessAt: String? = null,
    val refreshIntervalMinutes: Int = 5,
)

class MarketViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    val config: StateFlow<ServerConfig> = prefs.configFlow
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ServerConfig())

    private val _ui = MutableStateFlow(MarketUiState())
    val ui: StateFlow<MarketUiState> = _ui.asStateFlow()

    private var autoRefreshJob: Job? = null
    private val timeFmt = SimpleDateFormat("HH:mm:ss", Locale.getDefault())

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
                val successAt = if (bothOk) timeFmt.format(Date()) else _ui.value.lastSuccessAt

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
