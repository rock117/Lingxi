package com.lingxi.app.data.repo

import com.lingxi.app.data.api.LingxiApi
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.model.parseIntradayAlert
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * 通知内存镜像：HTTP 拉取 + WebSocket 增量。
 * 展示侧按内容指纹去重（相同日内强势截面 / 相同标题正文只保留最新一条）。
 */
object NotificationStore {
    private const val MAX = 100

    private val _notifications = MutableStateFlow<List<NotificationItem>>(emptyList())
    val notifications: StateFlow<List<NotificationItem>> = _notifications.asStateFlow()

    private val _newNotification = MutableSharedFlow<NotificationItem>(extraBufferCapacity = 16)
    val newNotification: SharedFlow<NotificationItem> = _newNotification.asSharedFlow()

    suspend fun refresh(api: LingxiApi) {
        val list = api.listNotifications()
        _notifications.value = dedupeNewestFirst(list).take(MAX)
    }

    fun ingest(item: NotificationItem) {
        val key = contentKey(item)
        var isNewContent = true
        _notifications.update { cur ->
            isNewContent = cur.none { it.id != item.id && contentKey(it) == key }
            dedupeNewestFirst(listOf(item) + cur.filter { it.id != item.id }).take(MAX)
        }
        // 内容与已有条重复时不弹系统通知
        if (isNewContent) {
            _newNotification.tryEmit(item)
        }
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

    suspend fun delete(api: LingxiApi, id: Long) {
        api.deleteNotification(id)
        _notifications.update { list -> list.filter { it.id != id } }
    }

    suspend fun deleteAll(api: LingxiApi) {
        api.deleteAllNotifications()
        _notifications.value = emptyList()
    }

    /** 列表已按时间新→旧时，每种内容只留第一条。 */
    internal fun dedupeNewestFirst(list: List<NotificationItem>): List<NotificationItem> {
        val seen = HashSet<String>()
        return list.filter { seen.add(contentKey(it)) }
    }

    /**
     * 日内强势：同日 + 相同 Top 标的/得分视为重复；
     * 其它：标题+正文相同视为重复。
     */
    internal fun contentKey(item: NotificationItem): String {
        val alert = item.parseIntradayAlert()
        if (alert != null && alert.date.isNotBlank()) {
            val sig = alert.items.joinToString("|") { row ->
                "${row.symbol}:${"%.2f".format(row.score)}"
            }
            return "intraday:${alert.date}:$sig"
        }
        return "plain:${item.title}\u0000${item.body}"
    }
}
