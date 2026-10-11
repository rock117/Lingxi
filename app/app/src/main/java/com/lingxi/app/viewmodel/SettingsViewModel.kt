package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.ServerConfig
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class SettingsViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    val config: StateFlow<ServerConfig> = prefs.configFlow
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ServerConfig())

    private val _message = MutableStateFlow<String?>(null)
    val message: StateFlow<String?> = _message.asStateFlow()

    private val _testing = MutableStateFlow(false)
    val testing: StateFlow<Boolean> = _testing.asStateFlow()

    fun save(config: ServerConfig) {
        viewModelScope.launch {
            prefs.save(config)
            _message.value = "已保存"
        }
    }

    fun testConnection() {
        viewModelScope.launch {
            _testing.value = true
            _message.value = null
            try {
                val cfg = prefs.configFlow.first()
                val health = ApiClient.create(cfg).health()
                _message.value = "连接成功：${health.status}（${cfg.httpBaseUrl()}）"
            } catch (e: Exception) {
                _message.value = "连接失败：${e.message ?: e.javaClass.simpleName}"
            } finally {
                _testing.value = false
            }
        }
    }

    fun clearMessage() {
        _message.value = null
    }
}
