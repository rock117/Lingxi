package com.lingxi.app.condition

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.StrategyDef
import com.lingxi.app.viewmodel.ScreenViewModel

/** 原子策略视觉分类（Chip 底色）。 */
private enum class AtomChipCategory {
    Trend,
    Momentum,
    Volume,
    Range,
}

private fun atomCategory(typeId: String): AtomChipCategory = when (typeId) {
    "ma_cross", "macd_cross", "ma_alignment", "price_breakout", "bollinger" ->
        AtomChipCategory.Trend
    "rsi" -> AtomChipCategory.Momentum
    "volume_spike" -> AtomChipCategory.Volume
    "price_change", "period_return" -> AtomChipCategory.Range
    else -> AtomChipCategory.Trend
}

@Composable
private fun atomChipColors(category: AtomChipCategory) = when (category) {
    AtomChipCategory.Trend -> AssistChipDefaults.assistChipColors(
        containerColor = MaterialTheme.colorScheme.primaryContainer,
        labelColor = MaterialTheme.colorScheme.onPrimaryContainer,
    )
    AtomChipCategory.Momentum -> AssistChipDefaults.assistChipColors(
        containerColor = MaterialTheme.colorScheme.tertiaryContainer,
        labelColor = MaterialTheme.colorScheme.onTertiaryContainer,
    )
    AtomChipCategory.Volume -> AssistChipDefaults.assistChipColors(
        containerColor = MaterialTheme.colorScheme.secondaryContainer,
        labelColor = MaterialTheme.colorScheme.onSecondaryContainer,
    )
    AtomChipCategory.Range -> AssistChipDefaults.assistChipColors(
        containerColor = MaterialTheme.colorScheme.surfaceVariant,
        labelColor = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StrategyListScreen(
    viewModel: ScreenViewModel,
    onComposeFrom: (String?) -> Unit,
    onEditSaved: (Long) -> Unit,
    onRunSaved: (Long) -> Unit,
) {
    val strategies by viewModel.strategies.collectAsStateWithLifecycle()
    val saved by viewModel.savedScreens.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()

    var renameTarget by remember { mutableStateOf<Condition?>(null) }
    var deleteTarget by remember { mutableStateOf<Condition?>(null) }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "我的策略",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = { onComposeFrom(null) }) { Text("新建") }
            }
        }

        if (saved.isEmpty()) {
            item {
                Text(
                    "暂无策略",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        } else {
            items(saved, key = { it.id }) { item ->
                SavedStrategyCard(
                    item = item,
                    catalog = strategies,
                    onOpen = { onEditSaved(item.id) },
                    onRun = { onRunSaved(item.id) },
                    onRename = { renameTarget = item },
                    onDelete = { deleteTarget = item },
                )
            }
        }

        item {
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                "原子策略",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(modifier = Modifier.height(8.dp))
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(0.dp),
            ) {
                strategies.forEach { def ->
                    AssistChip(
                        onClick = { onComposeFrom(def.typeId) },
                        label = { Text(def.name) },
                        colors = atomChipColors(atomCategory(def.typeId)),
                    )
                }
            }
        }

        error?.let {
            item { Text(it, color = MaterialTheme.colorScheme.error) }
        }
    }

    renameTarget?.let { target ->
        var draft by remember(target.id) { mutableStateOf(target.name) }
        AlertDialog(
            onDismissRequest = { renameTarget = null },
            title = { Text("重命名") },
            text = {
                OutlinedTextField(
                    value = draft,
                    onValueChange = { draft = it },
                    label = { Text("名称") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val n = draft.trim()
                        if (n.isNotEmpty()) {
                            viewModel.renameSaved(target.id, n)
                        }
                        renameTarget = null
                    },
                ) { Text("确定") }
            },
            dismissButton = {
                TextButton(onClick = { renameTarget = null }) { Text("取消") }
            },
        )
    }

    deleteTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("删除策略？") },
            text = { Text("将删除「${target.name}」，不可恢复。") },
            confirmButton = {
                TextButton(
                    onClick = {
                        viewModel.deleteSaved(target.id)
                        deleteTarget = null
                    },
                ) { Text("删除") }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) { Text("取消") }
            },
        )
    }
}

@Composable
private fun SavedStrategyCard(
    item: Condition,
    catalog: List<StrategyDef>,
    onOpen: () -> Unit,
    onRun: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    val summary = remember(item.expression, catalog) {
        ScreenViewModel.ruleSummary(item.expression, catalog)
    }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Column(modifier = Modifier.padding(start = 12.dp, end = 4.dp, top = 8.dp, bottom = 8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        item.name,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        summary,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Box {
                    IconButton(onClick = { menuOpen = true }) {
                        Icon(Icons.Default.MoreVert, contentDescription = "更多")
                    }
                    DropdownMenu(
                        expanded = menuOpen,
                        onDismissRequest = { menuOpen = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text("选股") },
                            onClick = {
                                menuOpen = false
                                onRun()
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("重命名") },
                            onClick = {
                                menuOpen = false
                                onRename()
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("编辑规则") },
                            onClick = {
                                menuOpen = false
                                onOpen()
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("删除") },
                            onClick = {
                                menuOpen = false
                                onDelete()
                            },
                        )
                    }
                }
            }
        }
    }
}
