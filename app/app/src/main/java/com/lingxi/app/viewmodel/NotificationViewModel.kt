package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.mock.MockRepository
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.notification.NotificationHelper
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn

class NotificationViewModel(application: Application) : AndroidViewModel(application) {
    // TODO: 接入后端后改为 LingxiApi.listNotifications / markRead + WsClient 实时推送
    val notifications: StateFlow<List<NotificationItem>> = MockRepository.notifications
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val unreadCount: StateFlow<Int> = MockRepository.notifications
        .map { list -> list.count { !it.read } }
        .onEach { unread ->
            // 未读数变化时同步更新桌面图标角标
            NotificationHelper.updateBadge(getApplication(), unread)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), 0)

    fun markRead(id: Long) = MockRepository.markRead(id)

    fun markAllRead() = MockRepository.markAllRead()

    fun pushMock() = MockRepository.pushMockNotification()

    override fun onCleared() {
        super.onCleared()
        // ViewModel 销毁时不清除角标，让 Service 继续维护
    }
}
