package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.ServerConfig
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

data class MarketUiState(
    val loading: Boolean = false,
    val result: MsiResult? = null,
    val mtt: MttResult? = null,
    val error: String? = null,
    val serverHint: String = "",
)

class MarketViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    val config: StateFlow<ServerConfig> = prefs.configFlow
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ServerConfig())

    private val _ui = MutableStateFlow(MarketUiState())
    val ui: StateFlow<MarketUiState> = _ui.asStateFlow()

    init {
        refresh()
    }

    fun refresh(days: Int = 5) {
        viewModelScope.launch {
            val cfg = prefs.configFlow.first()
            _ui.value = _ui.value.copy(
                loading = true,
                error = null,
                serverHint = cfg.httpBaseUrl(),
            )
            try {
                val api = ApiClient.create(cfg)
                val msiDeferred = async {
                    runCatching { api.sentiment(days = days, useMock = true) }
                }
                val mttDeferred = async {
                    runCatching { api.midTermTrend(useMock = true) }
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

                _ui.value = MarketUiState(
                    loading = false,
                    result = msiResult.getOrElse { _ui.value.result },
                    mtt = mttResult.getOrElse { _ui.value.mtt },
                    error = errors.takeIf { it.isNotEmpty() }?.joinToString("；"),
                    serverHint = cfg.httpBaseUrl(),
                )
            } catch (e: Exception) {
                _ui.value = MarketUiState(
                    loading = false,
                    result = _ui.value.result,
                    mtt = _ui.value.mtt,
                    error = e.message ?: e.javaClass.simpleName,
                    serverHint = cfg.httpBaseUrl(),
                )
            }
        }
    }
}
