package com.lingxi.app.data.repo

import com.lingxi.app.data.api.LingxiApi
import com.lingxi.app.data.model.NotificationItem
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * 通知内存镜像：HTTP 拉取 + WebSocket 增量。
 */
object NotificationStore {
    private const val MAX = 100

    private val _notifications = MutableStateFlow<List<NotificationItem>>(emptyList())
    val notifications: StateFlow<List<NotificationItem>> = _notifications.asStateFlow()

    private val _newNotification = MutableSharedFlow<NotificationItem>(extraBufferCapacity = 16)
    val newNotification: SharedFlow<NotificationItem> = _newNotification.asSharedFlow()

    suspend fun refresh(api: LingxiApi) {
        val list = api.listNotifications()
        _notifications.value = list.take(MAX)
    }

    fun ingest(item: NotificationItem) {
        _notifications.update { cur ->
            listOf(item) + cur.filter { it.id != item.id }.take(MAX - 1)
        }
        _newNotification.tryEmit(item)
    }

    suspend fun markRead(api: LingxiApi, id: Long) {
        val saved = api.markRead(id)
        _notifications.update { list ->
            list.map { if (it.id == id) saved else it }
        }
    }

    suspend fun markAllRead(api: LingxiApi) {
        val unread = _notifications.value.filter { !it.read }
        for (item in unread) {
            runCatching { markRead(api, item.id) }
        }
    }
}
