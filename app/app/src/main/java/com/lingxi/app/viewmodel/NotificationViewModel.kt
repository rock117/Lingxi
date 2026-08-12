package com.lingxi.app.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.mock.MockRepository
import com.lingxi.app.data.model.NotificationItem
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

class NotificationViewModel : ViewModel() {
    // TODO: 接入后端后改为 LingxiApi.listNotifications / markRead + WsClient 实时推送
    val notifications: StateFlow<List<NotificationItem>> = MockRepository.notifications
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val unreadCount: StateFlow<Int> = MockRepository.notifications
        .map { list -> list.count { !it.read } }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), 0)

    fun markRead(id: Long) = MockRepository.markRead(id)

    fun markAllRead() = MockRepository.markAllRead()

    fun pushMock() = MockRepository.pushMockNotification()
}
