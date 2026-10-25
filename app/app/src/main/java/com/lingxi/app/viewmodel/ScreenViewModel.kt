package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.CreateConditionRequest
import com.lingxi.app.data.model.ScreenRequest
import com.lingxi.app.data.model.ScreenResult
import com.lingxi.app.data.model.DimAi
import com.lingxi.app.data.model.StockAnalyze
import com.lingxi.app.data.model.StockSearchHit
import com.lingxi.app.data.model.StrategyDef
import com.lingxi.app.data.model.UpdateConditionRequest
import com.lingxi.app.data.prefs.AnalyzeHistoryPrefs
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

class ScreenViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)
    private val analyzeHistory = AnalyzeHistoryPrefs(application)

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

    private val _stockAnalyze = MutableStateFlow<StockAnalyze?>(null)
    val stockAnalyze: StateFlow<StockAnalyze?> = _stockAnalyze.asStateFlow()

    private val _stockAnalyzeLoading = MutableStateFlow(false)
    val stockAnalyzeLoading: StateFlow<Boolean> = _stockAnalyzeLoading.asStateFlow()

    private val _stockAnalyzeError = MutableStateFlow<String?>(null)
    val stockAnalyzeError: StateFlow<String?> = _stockAnalyzeError.asStateFlow()

    /** 当前正在请求的 AI 维：tape / short_term / fundamentals / statements */
    private val _stockAnalyzeAiLoadingDim = MutableStateFlow<String?>(null)
    val stockAnalyzeAiLoadingDim: StateFlow<String?> = _stockAnalyzeAiLoadingDim.asStateFlow()

    private val _stockSearchQuery = MutableStateFlow("")
    val stockSearchQuery: StateFlow<String> = _stockSearchQuery.asStateFlow()

    private val _stockSearchHits = MutableStateFlow<List<StockSearchHit>>(emptyList())
    val stockSearchHits: StateFlow<List<StockSearchHit>> = _stockSearchHits.asStateFlow()

    private val _stockSearchLoading = MutableStateFlow(false)
    val stockSearchLoading: StateFlow<Boolean> = _stockSearchLoading.asStateFlow()

    val analyzeSearchHistory: StateFlow<List<StockSearchHit>> =
        analyzeHistory.historyFlow.stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(5_000),
            emptyList(),
        )

    private var stockSearchJob: Job? = null
    private var stockAnalyzeAiJob: Job? = null

    init {
        refresh()
    }

    fun rememberAnalyzeStock(hit: StockSearchHit) {
        viewModelScope.launch {
            analyzeHistory.remember(
                symbol = hit.symbol,
                name = hit.name,
                code = hit.code,
                namePinyin = hit.namePinyin,
            )
        }
    }

    fun rememberAnalyzeStock(symbol: String, name: String) {
        viewModelScope.launch {
            analyzeHistory.remember(symbol = symbol, name = name)
        }
    }

    fun removeAnalyzeHistory(symbol: String) {
        viewModelScope.launch { analyzeHistory.remove(symbol) }
    }

    fun clearAnalyzeHistory() {
        viewModelScope.launch { analyzeHistory.clear() }
    }

    fun clearStockAnalyze() {
        stockAnalyzeAiJob?.cancel()
        _stockAnalyze.value = null
        _stockAnalyzeError.value = null
        _stockAnalyzeAiLoadingDim.value = null
        _stockSearchQuery.value = ""
        _stockSearchHits.value = emptyList()
    }

    fun onStockSearchQueryChange(q: String) {
        _stockSearchQuery.value = q
        stockSearchJob?.cancel()
        val trimmed = q.trim()
        if (trimmed.isEmpty()) {
            _stockSearchHits.value = emptyList()
            _stockSearchLoading.value = false
            return
        }
        stockSearchJob = viewModelScope.launch {
            delay(280)
            _stockSearchLoading.value = true
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                _stockSearchHits.value = api.stocksSearch(trimmed).items
            }.onFailure {
                _stockSearchHits.value = emptyList()
            }
            _stockSearchLoading.value = false
        }
    }

    fun loadStockAnalyze(symbol: String) {
        stockAnalyzeAiJob?.cancel()
        _stockAnalyzeAiLoadingDim.value = null
        viewModelScope.launch {
            _stockAnalyzeLoading.value = true
            _stockAnalyzeError.value = null
            _stockAnalyze.value = null
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val out = api.stockAnalyze(symbol)
                _stockAnalyze.value = out
                analyzeHistory.remember(
                    symbol = out.symbol.ifBlank { symbol },
                    name = out.name,
                )
            }.onFailure {
                _stockAnalyzeError.value = it.message ?: "分析失败"
            }
            _stockAnalyzeLoading.value = false
        }
    }

    /**
     * 按 Tab 懒加载单维 AI。已 ready 且非 force 则跳过。
     * @param dim tape | short_term | fundamentals | statements
     */
    fun loadStockAnalyzeAi(symbol: String, dim: String, force: Boolean = false) {
        val cur = _stockAnalyze.value
        if (cur == null || cur.symbol != symbol) return
        if (!force && dimAiOf(cur, dim).status == "ready") return
        if (!force && !cur.aiEnabled) return

        stockAnalyzeAiJob?.cancel()
        stockAnalyzeAiJob = viewModelScope.launch {
            _stockAnalyzeAiLoadingDim.value = dim
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val out = api.stockAnalyzeAi(symbol, dim, force)
                _stockAnalyze.update { prev ->
                    if (prev == null || prev.symbol != symbol) prev
                    else mergeDimAi(prev, out.dim, out.ai, out.aiModel)
                }
            }.onFailure { e ->
                val err = DimAi(
                    status = "error",
                    text = e.message ?: "AI 请求失败",
                    bias = "unknown",
                    confidence = "unknown",
                )
                _stockAnalyze.update { prev ->
                    if (prev == null || prev.symbol != symbol) prev
                    else mergeDimAi(prev, dim, err, prev.aiModel)
                }
            }
            if (_stockAnalyzeAiLoadingDim.value == dim) {
                _stockAnalyzeAiLoadingDim.value = null
            }
        }
    }

    private fun dimAiOf(a: StockAnalyze, dim: String): DimAi = when (dim) {
        "tape" -> a.ai
        "short_term" -> a.shortTerm.ai
        "fundamentals" -> a.fundamentals.ai
        "statements" -> a.statements.ai
        else -> DimAi()
    }

    private fun mergeDimAi(
        prev: StockAnalyze,
        dim: String,
        ai: DimAi,
        aiModel: String?,
    ): StockAnalyze {
        val model = aiModel ?: prev.aiModel
        return when (dim) {
            "tape" -> prev.copy(ai = ai, analysis = ai.text, aiModel = model)
            "short_term" -> prev.copy(
                shortTerm = prev.shortTerm.copy(ai = ai),
                aiModel = model,
            )
            "fundamentals" -> prev.copy(
                fundamentals = prev.fundamentals.copy(ai = ai),
                aiModel = model,
            )
            "statements" -> prev.copy(
                statements = prev.statements.copy(ai = ai),
                aiModel = model,
            )
            else -> prev
        }
    }

    fun refresh() {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                _strategies.value = api.listStrategies()
                _savedScreens.value = api.listConditions()
                    .filter { it.kind == "screen" }
                    .sortedByDescending { it.id }
            }.onFailure {
                _error.value = it.message ?: "加载失败"
            }
        }
    }

    fun findSaved(id: Long): Condition? = _savedScreens.value.find { it.id == id }

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

    fun updateScreen(id: Long, name: String, rules: List<JsonObject>, enabled: Boolean = true) {
        viewModelScope.launch {
            _loading.value = true
            _error.value = null
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val expression = buildExpression(rules)
                api.updateCondition(
                    id,
                    UpdateConditionRequest(
                        name = name,
                        kind = "screen",
                        symbol = null,
                        expression = expression.toString(),
                        enabled = enabled,
                    ),
                )
                _lastSavedId.value = id
                refresh()
            }.onFailure {
                _error.value = it.message ?: "保存失败"
            }
            _loading.value = false
        }
    }

    fun renameSaved(id: Long, name: String) {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val saved = api.updateCondition(id, UpdateConditionRequest(name = name))
                _savedScreens.value = _savedScreens.value
                    .map { if (it.id == id) saved else it }
                    .sortedByDescending { it.id }
            }.onFailure {
                _error.value = it.message ?: "重命名失败"
            }
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

    fun parseRules(expression: String): List<JsonObject> = Companion.parseRules(expression)

    private fun buildExpression(rules: List<JsonObject>) = buildJsonObject {
        put("op", "and")
        put("rules", buildJsonArray {
            rules.forEach { add(it) }
        })
    }

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        fun parseRules(expression: String): List<JsonObject> = runCatching {
            val root = json.parseToJsonElement(expression)
            when {
                root is JsonObject && root.containsKey("rules") -> {
                    root["rules"]?.jsonArray?.mapNotNull { el ->
                        el as? JsonObject
                    }.orEmpty()
                }
                root is JsonObject && root.containsKey("type") -> listOf(root)
                root is JsonArray -> root.mapNotNull { it as? JsonObject }
                else -> emptyList()
            }
        }.getOrDefault(emptyList())

        fun ruleSummary(expression: String, catalog: List<StrategyDef>): String {
            val rules = parseRules(expression)
            if (rules.isEmpty()) return "无规则"
            val names = rules.map { rule ->
                val tid = rule["type"]?.jsonPrimitive?.contentOrNull.orEmpty()
                catalog.find { it.typeId == tid }?.name ?: tid.ifBlank { "规则" }
            }
            return "${names.size} 条 · " + names.joinToString(" · ")
        }
    }
}
