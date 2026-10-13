package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.prefs.ServerPrefs
import com.lingxi.app.data.repo.NotificationStore
import com.lingxi.app.notification.NotificationHelper
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class NotificationViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)

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
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            runCatching {
                val cfg = prefs.configFlow.first()
                NotificationStore.refresh(ApiClient.create(cfg))
            }
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
