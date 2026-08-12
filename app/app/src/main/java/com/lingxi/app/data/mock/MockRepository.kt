package com.lingxi.app.data.mock

import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.NotificationItem
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.time.Instant
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random

/**
 * 本地内存 Mock，当前 UI / ViewModel 全部走这里。
 *
 * TODO: 后续用 [com.lingxi.app.data.api.LingxiApi] + [com.lingxi.app.data.ws.WsClient]
 *  实现真实 Repository，再替换各 ViewModel 中对本对象的依赖。
 */
object MockRepository {
    private val conditionIdSeq = AtomicLong(3)
    private val notificationIdSeq = AtomicLong(3)

    /** 通知列表最多保留条数 */
    private const val MAX_NOTIFICATIONS = 100

    /** 定时推送间隔范围（秒） */
    private const val MIN_INTERVAL_SEC = 5
    private const val MAX_INTERVAL_SEC = 15

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

    /** 新通知事件流：每次产生新通知时发射，供 Service 监听后发系统通知 */
    private val _newNotification = MutableSharedFlow<NotificationItem>(extraBufferCapacity = 16)
    val newNotification: SharedFlow<NotificationItem> = _newNotification.asSharedFlow()

    // ---- 定时随机推送 ----

    private var pushJob: Job? = null

    /** 启动定时随机推送（Mock 模式），每隔 5~15 秒随机发一条 */
    fun startAutoPush(scope: CoroutineScope) {
        if (pushJob?.isActive == true) return
        pushJob = scope.launch {
            while (isActive) {
                val waitSec = Random.nextInt(MIN_INTERVAL_SEC, MAX_INTERVAL_SEC + 1)
                delay(waitSec * 1000L)
                if (!isActive) break
                pushRandom()
            }
        }
    }

    /** 停止定时随机推送 */
    fun stopAutoPush() {
        pushJob?.cancel()
        pushJob = null
    }

    val isAutoPushing: Boolean get() = pushJob?.isActive == true

    // ---- CRUD ----

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

    /** 手动模拟一条推送，方便演示通知页刷新 */
    fun pushMockNotification() {
        pushRandom()
    }

    /** 随机选一个启用的条件，生成一条通知（列表上限 100 条） */
    private fun pushRandom() {
        val cond = _conditions.value.filter { it.enabled }.randomOrNull()
            ?: _conditions.value.firstOrNull()
            ?: return

        val bodies = when (cond.kind) {
            "trade" -> listOf(
                "MA5 上穿 MA20",
                "价格涨幅超过 ${Random.nextInt(3, 10)}%",
                "价格跌幅超过 ${Random.nextInt(3, 10)}%",
                "成交量异常放大",
                "RSI 超买区域",
            )
            else -> listOf(
                "匹配关键词: ${cond.expression}",
                "突发新闻事件触发",
                "相关市场异动",
            )
        }

        val item = NotificationItem(
            id = notificationIdSeq.incrementAndGet(),
            conditionId = cond.id,
            title = when (cond.kind) {
                "trade" -> "${cond.symbol ?: "?"} 交易信号"
                else -> "新闻事件: ${cond.name}"
            },
            body = bodies.random(),
            read = false,
            createdAt = Instant.now().toString(),
        )

        _notifications.update { list ->
            (listOf(item) + list).take(MAX_NOTIFICATIONS)
        }

        // 通过 SharedFlow 通知 Service 发系统通知
        _newNotification.tryEmit(item)
    }

    suspend fun mockTestConnection(): Result<String> {
        delay(600)
        return Result.success("Mock 连接成功（未请求真实 API）")
    }
}
