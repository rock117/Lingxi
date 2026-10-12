package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.CreateConditionRequest
import com.lingxi.app.data.model.UpdateConditionRequest
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

class ConditionViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    private val _conditions = MutableStateFlow<List<Condition>>(emptyList())
    val conditions: StateFlow<List<Condition>> = _conditions.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                _conditions.value = api.listConditions()
            }
        }
    }

    fun setEnabled(id: Long, enabled: Boolean) {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                val saved = api.updateCondition(id, UpdateConditionRequest(enabled = enabled))
                _conditions.value = _conditions.value.map { if (it.id == id) saved else it }
            }
        }
    }

    fun delete(id: Long) {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                api.deleteCondition(id)
                _conditions.value = _conditions.value.filter { it.id != id }
            }
        }
    }

    fun saveNew(
        name: String,
        kind: String,
        symbol: String?,
        expression: String,
        enabled: Boolean,
    ) {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                api.createCondition(
                    CreateConditionRequest(
                        name = name,
                        kind = kind,
                        symbol = symbol,
                        expression = expression,
                        enabled = enabled,
                    ),
                )
                refresh()
            }
        }
    }

    fun update(item: Condition) {
        viewModelScope.launch {
            runCatching {
                val api = ApiClient.create(prefs.configFlow.first())
                api.updateCondition(
                    item.id,
                    UpdateConditionRequest(
                        name = item.name,
                        kind = item.kind,
                        symbol = item.symbol,
                        expression = item.expression,
                        enabled = item.enabled,
                    ),
                )
                refresh()
            }
        }
    }

    fun findById(id: Long): Condition? = _conditions.value.find { it.id == id }
}
