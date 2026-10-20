package com.lingxi.app.condition

import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.StrategyDef
import com.lingxi.app.viewmodel.ScreenViewModel

@Composable
fun StrategyListScreen(
    viewModel: ScreenViewModel,
    onComposeFrom: (String?) -> Unit,
    onRunSaved: (Long) -> Unit,
) {
    val strategies by viewModel.strategies.collectAsStateWithLifecycle()
    val saved by viewModel.savedScreens.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Text("内置策略", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Text(
                "点选后可调参并组合保存为选股策略",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        items(strategies, key = { it.typeId }) { def ->
            CatalogCard(def = def, onClick = { onComposeFrom(def.typeId) })
        }
        item {
            Spacer(modifier = Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "我的选股策略",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = { onComposeFrom(null) }) { Text("新建组合") }
            }
        }
        if (saved.isEmpty()) {
            item {
                Text(
                    "暂无保存的选股策略，可从上方内置策略开始",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            items(saved, key = { it.id }) { item ->
                SavedScreenCard(
                    item = item,
                    onRun = { onRunSaved(item.id) },
                    onDelete = { viewModel.deleteSaved(item.id) },
                )
            }
        }
        error?.let {
            item {
                Text(it, color = MaterialTheme.colorScheme.error)
            }
        }
    }
}

@Composable
private fun CatalogCard(def: StrategyDef, onClick: () -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(def.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Spacer(modifier = Modifier.height(4.dp))
            Text(def.summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(modifier = Modifier.height(8.dp))
            AssistChip(onClick = {}, enabled = false, label = { Text(def.typeId) })
        }
    }
}

@Composable
private fun SavedScreenCard(
    item: Condition,
    onRun: () -> Unit,
    onDelete: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    item.name,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = onRun) {
                    Icon(Icons.Default.PlayArrow, contentDescription = "选股")
                }
                IconButton(onClick = onDelete) {
                    Icon(Icons.Default.Delete, contentDescription = "删除")
                }
            }
            Text(
                item.expression,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 3,
            )
        }
    }
}
