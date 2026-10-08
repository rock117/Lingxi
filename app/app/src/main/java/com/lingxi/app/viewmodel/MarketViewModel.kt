package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.ServerConfig
import com.lingxi.app.data.prefs.ServerPrefs
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
                val result = api.sentiment(days = days, useMock = true)
                _ui.value = MarketUiState(
                    loading = false,
                    result = result,
                    error = null,
                    serverHint = cfg.httpBaseUrl(),
                )
            } catch (e: Exception) {
                _ui.value = MarketUiState(
                    loading = false,
                    result = _ui.value.result,
                    error = e.message ?: e.javaClass.simpleName,
                    serverHint = cfg.httpBaseUrl(),
                )
            }
        }
    }
}
