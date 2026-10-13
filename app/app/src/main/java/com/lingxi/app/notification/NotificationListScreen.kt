package com.lingxi.app.notification

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.ExperimentalFoundationApi
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
import com.lingxi.app.data.model.parseIntradayAlert
import com.lingxi.app.viewmodel.NotificationViewModel
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

private val Shanghai = ZoneId.of("Asia/Shanghai")
private val TimeFmt = DateTimeFormatter.ofPattern("MM-dd HH:mm")
private const val COLLAPSED_COUNT = 3

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun NotificationListScreen(viewModel: NotificationViewModel) {
    val items by viewModel.notifications.collectAsStateWithLifecycle()
    val total by viewModel.totalCount.collectAsStateWithLifecycle()
    val unread by viewModel.unreadCount.collectAsStateWithLifecycle()
    val refreshing by viewModel.refreshing.collectAsStateWithLifecycle()

    var selecting by remember { mutableStateOf(false) }
    var selectedIds by remember { mutableStateOf(setOf<Long>()) }
    var confirmDelete by remember { mutableStateOf(false) }

    fun exitSelection() {
        selecting = false
        selectedIds = emptySet()
    }

    Column(modifier = Modifier.fillMaxSize()) {
        if (selecting) {
            SelectionBar(
                selectedCount = selectedIds.size,
                total = total,
                onCancel = { exitSelection() },
                onSelectAll = {
                    selectedIds = if (selectedIds.size == items.size) {
                        emptySet()
                    } else {
                        items.map { it.id }.toSet()
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
                        text = if (unread > 0) "共 $total 条 · 未读 $unread" else "共 $total 条",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        text = "下拉刷新 · 长按可选中删除 · 点按展开",
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

        PullToRefreshBox(
            isRefreshing = refreshing,
            onRefresh = {
                if (!selecting) viewModel.refresh()
            },
            modifier = Modifier.weight(1f),
        ) {
            if (items.isEmpty()) {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(24.dp),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text("暂无通知", style = MaterialTheme.typography.titleMedium)
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
                    items(items, key = { it.id }) { item ->
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
                                } else {
                                    // expand handled inside card
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
    var expanded by rememberSaveable(item.id) { mutableStateOf(false) }

    val container = when {
        selecting && selected -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f)
        item.read -> MaterialTheme.colorScheme.surface
        else -> MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.35f)
    }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .combinedClickable(
                onClick = {
                    if (selecting) {
                        onClick()
                    } else {
                        val willExpand = !expanded
                        expanded = willExpand
                        if (willExpand) onToggleExpandUnread()
                    }
                },
                onLongClick = onLongClick,
            ),
        colors = CardDefaults.cardColors(containerColor = container),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
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
                        imageVector = if (expanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                        contentDescription = if (expanded) "收起" else "展开",
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

            if (!selecting) {
                if (alert != null) {
                    Spacer(modifier = Modifier.height(10.dp))
                    IntradayAlertContent(alert = alert, expanded = expanded)
                } else {
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        item.body,
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = if (expanded) Int.MAX_VALUE else 4,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun IntradayAlertContent(
    alert: IntradayAlertPayload,
    expanded: Boolean,
) {
    Text(
        text = "大盘等权 ${formatSignedPct(alert.rMkt)}",
        style = MaterialTheme.typography.bodyMedium,
        fontWeight = FontWeight.Medium,
    )
    Spacer(modifier = Modifier.height(8.dp))

    val visible = if (expanded) alert.items else alert.items.take(COLLAPSED_COUNT)
    visible.forEachIndexed { index, row ->
        if (index > 0) {
            HorizontalDivider(
                modifier = Modifier.padding(vertical = 6.dp),
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
            )
        }
        AlertStockRow(rank = index + 1, item = row)
    }

    if (alert.items.size > COLLAPSED_COUNT) {
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = if (expanded) {
                "点击收起"
            } else {
                "展开全部 ${alert.items.size} 只"
            },
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
        )
    }

    AnimatedVisibility(visible = expanded && alert.date.isNotBlank()) {
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
private fun AlertStockRow(rank: Int, item: IntradayAlertItem) {
    val code = item.symbol.substringBefore('.')
    val title = if (item.name.isNotBlank()) item.name else code
    val subtitle = if (item.name.isNotBlank()) code else item.symbol

    Row(
        modifier = Modifier.fillMaxWidth(),
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
                text = "相对 ${formatSignedPct(item.rs)}",
                style = MaterialTheme.typography.labelSmall,
                color = if (item.rs >= 0) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.error
                },
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
