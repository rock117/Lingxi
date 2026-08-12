package com.lingxi.app.data.mock

import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.NotificationItem
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.time.Instant
import java.util.concurrent.atomic.AtomicLong

/**
 * 本地内存 Mock，当前 UI / ViewModel 全部走这里。
 *
 * TODO: 后续用 [com.lingxi.app.data.api.LingxiApi] + [com.lingxi.app.data.ws.WsClient]
 *  实现真实 Repository，再替换各 ViewModel 中对本对象的依赖。
 */
object MockRepository {
    private val conditionIdSeq = AtomicLong(3)
    private val notificationIdSeq = AtomicLong(3)

    private val _conditions = MutableStateFlow(
        listOf(
            Condition(
                id = 1,
                name = "AAPL 跌幅提醒",
                kind = "trade",
                symbol = "AAPL",
                expression = """{"type":"price_drop","threshold":0.05}""",
                enabled = true,
                createdAt = "2026-08-01T10:00:00Z",
                updatedAt = "2026-08-01T10:00:00Z",
            ),
            Condition(
                id = 2,
                name = "美联储利率新闻",
                kind = "news",
                symbol = null,
                expression = """{"type":"keyword","value":"rate cut"}""",
                enabled = true,
                createdAt = "2026-08-02T09:00:00Z",
                updatedAt = "2026-08-02T09:00:00Z",
            ),
            Condition(
                id = 3,
                name = "TSLA 波动监控",
                kind = "trade",
                symbol = "TSLA",
                expression = """{"type":"price_drop","threshold":0.08}""",
                enabled = false,
                createdAt = "2026-08-03T14:20:00Z",
                updatedAt = "2026-08-05T11:00:00Z",
            ),
        ),
    )
    val conditions: StateFlow<List<Condition>> = _conditions.asStateFlow()

    private val _notifications = MutableStateFlow(
        listOf(
            NotificationItem(
                id = 1,
                conditionId = 1,
                title = "AAPL 交易信号",
                body = "跌幅超过 5%，当前模拟触发",
                read = false,
                createdAt = "2026-08-10T08:15:00Z",
            ),
            NotificationItem(
                id = 2,
                conditionId = 2,
                title = "新闻事件: 美联储利率新闻",
                body = "匹配关键词 rate cut",
                read = false,
                createdAt = "2026-08-11T16:40:00Z",
            ),
            NotificationItem(
                id = 3,
                conditionId = 1,
                title = "AAPL 交易信号",
                body = "MA5 上穿 MA20（历史）",
                read = true,
                createdAt = "2026-08-08T12:00:00Z",
            ),
        ),
    )
    val notifications: StateFlow<List<NotificationItem>> = _notifications.asStateFlow()

    fun addCondition(
        name: String,
        kind: String,
        symbol: String?,
        expression: String,
        enabled: Boolean,
    ): Condition {
        val now = Instant.now().toString()
        val item = Condition(
            id = conditionIdSeq.incrementAndGet(),
            name = name,
            kind = kind,
            symbol = symbol?.ifBlank { null },
            expression = expression,
            enabled = enabled,
            createdAt = now,
            updatedAt = now,
        )
        _conditions.update { listOf(item) + it }
        return item
    }

    fun updateCondition(item: Condition) {
        val now = Instant.now().toString()
        _conditions.update { list ->
            list.map { if (it.id == item.id) item.copy(updatedAt = now) else it }
        }
    }

    fun setEnabled(id: Long, enabled: Boolean) {
        _conditions.update { list ->
            list.map {
                if (it.id == id) {
                    it.copy(enabled = enabled, updatedAt = Instant.now().toString())
                } else {
                    it
                }
            }
        }
    }

    fun deleteCondition(id: Long) {
        _conditions.update { it.filterNot { c -> c.id == id } }
        _notifications.update { it.filterNot { n -> n.conditionId == id } }
    }

    fun markRead(id: Long) {
        _notifications.update { list ->
            list.map { if (it.id == id) it.copy(read = true) else it }
        }
    }

    fun markAllRead() {
        _notifications.update { list -> list.map { it.copy(read = true) } }
    }

    /** 模拟一条推送，方便演示通知页刷新 */
    fun pushMockNotification() {
        val cond = _conditions.value.firstOrNull { it.enabled }
            ?: _conditions.value.firstOrNull()
            ?: return
        val item = NotificationItem(
            id = notificationIdSeq.incrementAndGet(),
            conditionId = cond.id,
            title = when (cond.kind) {
                "trade" -> "${cond.symbol ?: "?"} 交易信号"
                else -> "新闻事件: ${cond.name}"
            },
            body = "Mock 推送 · ${Instant.now()}",
            read = false,
            createdAt = Instant.now().toString(),
        )
        _notifications.update { listOf(item) + it }
    }

    suspend fun mockTestConnection(): Result<String> {
        delay(600)
        return Result.success("Mock 连接成功（未请求真实 API）")
    }
}
