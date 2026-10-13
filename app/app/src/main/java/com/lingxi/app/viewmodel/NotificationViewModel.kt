package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.prefs.ServerPrefs
import com.lingxi.app.data.repo.NotificationStore
import com.lingxi.app.notification.NotificationHelper
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class NotificationViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

    private val _refreshing = MutableStateFlow(false)
    val refreshing: StateFlow<Boolean> = _refreshing.asStateFlow()

    private val foregroundObserver = object : DefaultLifecycleObserver {
        override fun onStart(owner: LifecycleOwner) {
            // 从后台回到前台时补拉积压通知（WS 可能已漏推）
            refresh()
        }
    }

    val notifications: StateFlow<List<NotificationItem>> = NotificationStore.notifications
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val unreadCount: StateFlow<Int> = NotificationStore.notifications
        .map { list -> list.count { !it.read } }
        .onEach { unread ->
            NotificationHelper.updateBadge(getApplication(), unread)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), 0)

    val totalCount: StateFlow<Int> = NotificationStore.notifications
        .map { it.size }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), 0)

    init {
        ProcessLifecycleOwner.get().lifecycle.addObserver(foregroundObserver)
    }

    override fun onCleared() {
        ProcessLifecycleOwner.get().lifecycle.removeObserver(foregroundObserver)
        super.onCleared()
    }

    fun refresh() {
        if (_refreshing.value) return
        viewModelScope.launch {
            _refreshing.value = true
            runCatching {
                val cfg = prefs.configFlow.first()
                NotificationStore.refresh(ApiClient.create(cfg))
            }
            _refreshing.value = false
        }
    }

    fun markRead(id: Long) {
        viewModelScope.launch {
            runCatching {
                val cfg = prefs.configFlow.first()
                NotificationStore.markRead(ApiClient.create(cfg), id)
            }
        }
    }

    fun markAllRead() {
        viewModelScope.launch {
            runCatching {
                val cfg = prefs.configFlow.first()
                NotificationStore.markAllRead(ApiClient.create(cfg))
            }
        }
    }

    fun delete(id: Long) {
        viewModelScope.launch {
            runCatching {
                val cfg = prefs.configFlow.first()
                NotificationStore.delete(ApiClient.create(cfg), id)
                NotificationHelper.cancel(getApplication(), id)
            }
        }
    }

    fun deleteAll() {
        viewModelScope.launch {
            runCatching {
                val cfg = prefs.configFlow.first()
                NotificationStore.deleteAll(ApiClient.create(cfg))
                NotificationHelper.cancelAll(getApplication())
            }
        }
    }
}
