package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.CreateConditionRequest
import com.lingxi.app.data.model.ScreenRequest
import com.lingxi.app.data.model.ScreenResult
import com.lingxi.app.data.model.StrategyDef
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put

class ScreenViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    private val _strategies = MutableStateFlow<List<StrategyDef>>(emptyList())
    val strategies: StateFlow<List<StrategyDef>> = _strategies.asStateFlow()

    private val _savedScreens = MutableStateFlow<List<Condition>>(emptyList())
    val savedScreens: StateFlow<List<Condition>> = _savedScreens.asStateFlow()

    private val _result = MutableStateFlow<ScreenResult?>(null)
    val result: StateFlow<ScreenResult?> = _result.asStateFlow()

    private val _loading = MutableStateFlow(false)
    val loading: StateFlow<Boolean> = _loading.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    private val _lastSavedId = MutableStateFlow<Long?>(null)
    val lastSavedId: StateFlow<Long?> = _lastSavedId.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                _strategies.value = api.listStrategies()
                _savedScreens.value = api.listConditions().filter { it.kind == "screen" }
            }.onFailure {
                _error.value = it.message ?: "加载失败"
            }
        }
    }

    fun clearError() {
        _error.value = null
    }

    fun clearResult() {
        _result.value = null
    }

    fun consumeLastSavedId(): Long? {
        val id = _lastSavedId.value
        _lastSavedId.value = null
        return id
    }

    fun preview(rules: List<JsonObject>, topN: Int = 50) {
        viewModelScope.launch {
            _loading.value = true
            _error.value = null
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val expression = buildExpression(rules)
                _result.value = api.screenPreview(ScreenRequest(expression = expression, topN = topN))
            }.onFailure {
                _error.value = it.message ?: "选股失败"
            }
            _loading.value = false
        }
    }

    fun runSaved(conditionId: Long, topN: Int = 50) {
        viewModelScope.launch {
            _loading.value = true
            _error.value = null
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                _result.value = api.screenByCondition(conditionId, topN)
            }.onFailure {
                _error.value = it.message ?: "选股失败"
            }
            _loading.value = false
        }
    }

    fun saveScreen(name: String, rules: List<JsonObject>, enabled: Boolean = true) {
        viewModelScope.launch {
            _loading.value = true
            _error.value = null
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val expression = buildExpression(rules)
                val saved = api.createCondition(
                    CreateConditionRequest(
                        name = name,
                        kind = "screen",
                        symbol = null,
                        expression = expression.toString(),
                        enabled = enabled,
                    ),
                )
                _lastSavedId.value = saved.id
                refresh()
            }.onFailure {
                _error.value = it.message ?: "保存失败"
            }
            _loading.value = false
        }
    }

    fun deleteSaved(id: Long) {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                api.deleteCondition(id)
                _savedScreens.value = _savedScreens.value.filter { it.id != id }
            }.onFailure {
                _error.value = it.message ?: "删除失败"
            }
        }
    }

    fun defaultRuleOf(def: StrategyDef): JsonObject =
        def.defaultRule.jsonObject

    fun applyParam(rule: JsonObject, key: String, value: String, isNumber: Boolean): JsonObject {
        val map = rule.toMutableMap()
        map[key] = if (isNumber) {
            JsonPrimitive(value.toDoubleOrNull() ?: 0.0)
        } else {
            JsonPrimitive(value)
        }
        return JsonObject(map)
    }

    private fun buildExpression(rules: List<JsonObject>) = buildJsonObject {
        put("op", "and")
        put("rules", buildJsonArray {
            rules.forEach { add(it) }
        })
    }
}
