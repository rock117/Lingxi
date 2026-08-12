package com.lingxi.app.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.mock.MockRepository
import com.lingxi.app.data.model.Condition
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn

class ConditionViewModel : ViewModel() {
    // TODO: 接入后端后改为 LingxiApi.listConditions() / create / update / delete
    val conditions: StateFlow<List<Condition>> = MockRepository.conditions
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun setEnabled(id: Long, enabled: Boolean) = MockRepository.setEnabled(id, enabled)

    fun delete(id: Long) = MockRepository.deleteCondition(id)

    fun saveNew(
        name: String,
        kind: String,
        symbol: String?,
        expression: String,
        enabled: Boolean,
    ) {
        MockRepository.addCondition(name, kind, symbol, expression, enabled)
    }

    fun update(item: Condition) = MockRepository.updateCondition(item)

    fun findById(id: Long): Condition? = conditions.value.find { it.id == id }
}
