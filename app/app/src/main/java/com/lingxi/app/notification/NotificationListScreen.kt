package com.lingxi.app.notification

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.SelectAll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.IntradayAlertItem
import com.lingxi.app.data.model.IntradayAlertPayload
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.model.displayReason
import com.lingxi.app.data.model.parseIntradayAlert
import com.lingxi.app.viewmodel.NotificationViewModel
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

private val Shanghai = ZoneId.of("Asia/Shanghai")
private val TimeFmt = DateTimeFormatter.ofPattern("MM-dd HH:mm")
private const val COLLAPSED_COUNT = 3

private enum class AlertSide {
    Trend,
    Oversold,
}

private const val KIND_TREND = "intraday_alert"
private const val KIND_OVERSOLD = "intraday_oversold"

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun NotificationListScreen(viewModel: NotificationViewModel) {
    val items by viewModel.notifications.collectAsStateWithLifecycle()
    val total by viewModel.totalCount.collectAsStateWithLifecycle()
    val unread by viewModel.unreadCount.collectAsStateWithLifecycle()
    val refreshing by viewModel.refreshing.collectAsStateWithLifecycle()

    var side by rememberSaveable { mutableStateOf(AlertSide.Trend) }
    var selecting by remember { mutableStateOf(false) }
    var selectedIds by remember { mutableStateOf(setOf<Long>()) }
    var confirmDelete by remember { mutableStateOf(false) }

    fun exitSelection() {
        selecting = false
        selectedIds = emptySet()
    }

    val filtered = remember(items, side) {
        val want = when (side) {
            AlertSide.Trend -> KIND_TREND
            AlertSide.Oversold -> KIND_OVERSOLD
        }
        items.filter { it.parseIntradayAlert()?.kind == want }
    }
    val sideUnread = remember(filtered) { filtered.count { !it.read } }

    Column(modifier = Modifier.fillMaxSize()) {
        if (selecting) {
            SelectionBar(
                selectedCount = selectedIds.size,
                total = filtered.size,
                onCancel = { exitSelection() },
                onSelectAll = {
                    selectedIds = if (selectedIds.size == filtered.size) {
                        emptySet()
                    } else {
                        filtered.map { it.id }.toSet()
                    }
                },
                onDelete = { if (selectedIds.isNotEmpty()) confirmDelete = true },
            )
        } else {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = buildString {
                            append("本类 ${filtered.size} 条")
                            if (sideUnread > 0) append(" · 未读 $sideUnread")
                            if (total > filtered.size) append(" · 全部 $total")
                        },
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        text = "下拉刷新 · 长按可选 · 点标题展开列表 · 点个股看理由",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(
                    onClick = { viewModel.markAllRead() },
                    enabled = unread > 0,
                ) {
                    Icon(Icons.Default.DoneAll, contentDescription = null)
                    Text(" 已读")
                }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 12.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            FilterChip(
                selected = side == AlertSide.Trend,
                onClick = {
                    side = AlertSide.Trend
                    exitSelection()
                },
                label = { Text("趋势") },
            )
            FilterChip(
                selected = side == AlertSide.Oversold,
                onClick = {
                    side = AlertSide.Oversold
                    exitSelection()
                },
                label = { Text("超跌") },
            )
        }

        PullToRefreshBox(
            isRefreshing = refreshing,
            onRefresh = {
                if (!selecting) viewModel.refresh()
            },
            modifier = Modifier.weight(1f),
        ) {
            if (filtered.isEmpty()) {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(24.dp),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        when (side) {
                            AlertSide.Trend -> "暂无趋势通知"
                            AlertSide.Oversold -> "暂无超跌通知"
                        },
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(
                        "下拉可刷新",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    items(filtered, key = { it.id }) { item ->
                        val selected = item.id in selectedIds
                        NotificationCard(
                            item = item,
                            selecting = selecting,
                            selected = selected,
                            onClick = {
                                if (selecting) {
                                    selectedIds = if (selected) {
                                        selectedIds - item.id
                                    } else {
                                        selectedIds + item.id
                                    }
                                    if (selectedIds.isEmpty()) {
                                        selecting = false
                                    }
                                }
                            },
                            onLongClick = {
                                if (!selecting) {
                                    selecting = true
                                    selectedIds = setOf(item.id)
                                }
                            },
                            onToggleExpandUnread = {
                                if (!item.read) viewModel.markRead(item.id)
                            },
                        )
                    }
                }
            }
        }
    }

    if (confirmDelete) {
        val n = selectedIds.size
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("删除选中的消息？") },
            text = { Text("将删除 $n 条消息，不可恢复。") },
            confirmButton = {
                TextButton(
                    onClick = {
                        val ids = selectedIds.toList()
                        confirmDelete = false
                        exitSelection()
                        viewModel.deleteSelected(ids)
                    },
                ) {
                    Text("删除")
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = false }) {
                    Text("取消")
                }
            },
        )
    }
}

@Composable
private fun SelectionBar(
    selectedCount: Int,
    total: Int,
    onCancel: () -> Unit,
    onSelectAll: () -> Unit,
    onDelete: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onCancel) {
            Icon(Icons.Default.Close, contentDescription = "取消")
        }
        Text(
            text = if (selectedCount > 0) "已选 $selectedCount 项" else "选择消息",
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.weight(1f),
        )
        IconButton(onClick = onSelectAll) {
            Icon(
                Icons.Default.SelectAll,
                contentDescription = if (selectedCount == total) "取消全选" else "全选",
            )
        }
        IconButton(
            onClick = onDelete,
            enabled = selectedCount > 0,
        ) {
            Icon(
                Icons.Default.Delete,
                contentDescription = "删除",
                tint = if (selectedCount > 0) {
                    MaterialTheme.colorScheme.error
                } else {
                    MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                },
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun NotificationCard(
    item: NotificationItem,
    selecting: Boolean,
    selected: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    onToggleExpandUnread: () -> Unit,
) {
    val alert = item.parseIntradayAlert()
    var listExpanded by rememberSaveable(item.id) { mutableStateOf(false) }

    val container = when {
        selecting && selected -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f)
        item.read -> MaterialTheme.colorScheme.surface
        else -> MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.35f)
    }

    fun toggleListExpand() {
        if (selecting) {
            onClick()
            return
        }
        val willExpand = !listExpanded
        listExpanded = willExpand
        if (willExpand) onToggleExpandUnread()
    }

    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = container),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            // 仅头部控制整卡展开/收起，避免与个股行抢点击
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .combinedClickable(
                        onClick = { toggleListExpand() },
                        onLongClick = onLongClick,
                    ),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (selecting) {
                        Checkbox(
                            checked = selected,
                            onCheckedChange = { onClick() },
                        )
                    }
                    Text(
                        text = item.title,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = if (item.read) FontWeight.Normal else FontWeight.Bold,
                        modifier = Modifier.weight(1f),
                    )
                    if (!item.read) {
                        Text(
                            "未读",
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.labelMedium,
                            modifier = Modifier.padding(end = 4.dp),
                        )
                    }
                    if (!selecting) {
                        Icon(
                            imageVector = if (listExpanded) {
                                Icons.Default.ExpandLess
                            } else {
                                Icons.Default.ExpandMore
                            },
                            contentDescription = if (listExpanded) "收起列表" else "展开列表",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    formatCreatedAt(item.createdAt),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            if (!selecting) {
                if (alert != null) {
                    Spacer(modifier = Modifier.height(10.dp))
                    IntradayAlertContent(
                        alert = alert,
                        listExpanded = listExpanded,
                        onToggleListExpand = { toggleListExpand() },
                    )
                } else {
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        item.body,
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = if (listExpanded) Int.MAX_VALUE else 4,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.clickable { toggleListExpand() },
                    )
                }
            }
        }
    }
}

@Composable
private fun IntradayAlertContent(
    alert: IntradayAlertPayload,
    listExpanded: Boolean,
    onToggleListExpand: () -> Unit,
) {
    Text(
        text = "大盘等权 ${formatSignedPct(alert.rMkt)}",
        style = MaterialTheme.typography.bodyMedium,
        fontWeight = FontWeight.Medium,
    )
    Spacer(modifier = Modifier.height(8.dp))

    val visible = if (listExpanded) alert.items else alert.items.take(COLLAPSED_COUNT)
    visible.forEachIndexed { index, row ->
        if (index > 0) {
            HorizontalDivider(
                modifier = Modifier.padding(vertical = 6.dp),
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
            )
        }
        AlertStockRow(rank = index + 1, kind = alert.kind, item = row)
    }

    if (alert.items.size > COLLAPSED_COUNT) {
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = if (listExpanded) {
                "收起列表"
            } else {
                "展开全部 ${alert.items.size} 只"
            },
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.clickable(onClick = onToggleListExpand),
        )
    }

    AnimatedVisibility(visible = listExpanded && alert.date.isNotBlank()) {
        Column {
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "截面 ${alert.date} · 候选 ${alert.candidateSize}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun AlertStockRow(
    rank: Int,
    kind: String,
    item: IntradayAlertItem,
) {
    val code = item.symbol.substringBefore('.')
    val title = if (item.name.isNotBlank()) item.name else code
    val subtitle = if (item.name.isNotBlank()) code else item.symbol
    var reasonExpanded by rememberSaveable(kind, item.symbol, rank) { mutableStateOf(false) }
    val reasonText = remember(item, kind) { item.displayReason(kind) }

    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { reasonExpanded = !reasonExpanded },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "$rank",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(end = 10.dp),
            )
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    text = "%.0f".format(item.score),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                )
                Text(
                    text = if (item.drawdownPct != 0.0) {
                        "回撤 ${formatSignedPct(item.drawdownPct)}"
                    } else {
                        "相对 ${formatSignedPct(item.rs)}"
                    },
                    style = MaterialTheme.typography.labelSmall,
                    color = if (
                        (item.drawdownPct != 0.0 && item.drawdownPct >= 0) ||
                            (item.drawdownPct == 0.0 && item.rs >= 0)
                    ) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.error
                    },
                )
            }
            Icon(
                imageVector = if (reasonExpanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                contentDescription = if (reasonExpanded) "收起理由" else "上榜理由",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp),
            )
        }
        AnimatedVisibility(visible = reasonExpanded && reasonText.isNotBlank()) {
            Text(
                text = "上榜理由：$reasonText",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 28.dp, top = 6.dp, end = 4.dp),
            )
        }
        AnimatedVisibility(visible = reasonExpanded && reasonText.isBlank()) {
            Text(
                text = "上榜理由：暂无明细（旧通知可能未写入）",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 28.dp, top = 6.dp, end = 4.dp),
            )
        }
    }
}

private fun formatSignedPct(v: Double): String = "%+.1f%%".format(v)

/** 统一按上海时区展示；兼容历史 UTC（+00:00）与新写入的 +08:00。 */
private fun formatCreatedAt(raw: String): String {
    return runCatching {
        OffsetDateTime.parse(raw).atZoneSameInstant(Shanghai).format(TimeFmt)
    }.getOrElse {
        runCatching {
            val trimmed = raw.replace(Regex("\\.\\d+"), "")
            OffsetDateTime.parse(trimmed).atZoneSameInstant(Shanghai).format(TimeFmt)
        }.getOrDefault(raw)
    }
}
