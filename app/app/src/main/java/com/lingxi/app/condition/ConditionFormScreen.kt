package com.lingxi.app.condition

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.lingxi.app.data.model.Condition
import com.lingxi.app.viewmodel.ConditionViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConditionFormScreen(
    viewModel: ConditionViewModel,
    editId: Long?,
    onDone: () -> Unit,
    onBack: () -> Unit,
) {
    val existing: Condition? = remember(editId) {
        editId?.let { viewModel.findById(it) }
    }

    var name by remember(existing) { mutableStateOf(existing?.name.orEmpty()) }
    var kind by remember(existing) { mutableStateOf(existing?.kind ?: "trade") }
    var symbol by remember(existing) { mutableStateOf(existing?.symbol.orEmpty()) }
    var threshold by remember(existing) {
        mutableStateOf(
            existing?.expression
                ?.substringAfter("\"threshold\":")
                ?.substringBefore("}")
                ?.trim()
                ?: "0.05",
        )
    }
    var keyword by remember(existing) {
        mutableStateOf(
            if (existing?.kind == "news") {
                existing.expression
                    .substringAfter("\"value\":\"")
                    .substringBefore("\"")
            } else {
                ""
            },
        )
    }
    var enabled by remember(existing) { mutableStateOf(existing?.enabled ?: true) }
    var error by remember { mutableStateOf<String?>(null) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (editId == null) "新建条件" else "编辑条件") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("名称") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            Text("类型")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = kind == "trade",
                    onClick = { kind = "trade" },
                    label = { Text("交易") },
                )
                FilterChip(
                    selected = kind == "news",
                    onClick = { kind = "news" },
                    label = { Text("新闻") },
                )
            }

            if (kind == "trade") {
                OutlinedTextField(
                    value = symbol,
                    onValueChange = { symbol = it.uppercase() },
                    label = { Text("股票代码") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    placeholder = { Text("如 AAPL") },
                )
                OutlinedTextField(
                    value = threshold,
                    onValueChange = { threshold = it },
                    label = { Text("跌幅阈值 (0~1)") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    placeholder = { Text("0.05") },
                )
            } else {
                OutlinedTextField(
                    value = keyword,
                    onValueChange = { keyword = it },
                    label = { Text("关键词") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    placeholder = { Text("如 rate cut") },
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("启用", modifier = Modifier.weight(1f))
                Switch(checked = enabled, onCheckedChange = { enabled = it })
            }

            error?.let {
                Text(it, color = androidx.compose.material3.MaterialTheme.colorScheme.error)
            }

            Spacer(modifier = Modifier.height(8.dp))
            Button(
                onClick = {
                    if (name.isBlank()) {
                        error = "请填写名称"
                        return@Button
                    }
                    val expression = if (kind == "trade") {
                        if (symbol.isBlank()) {
                            error = "交易条件需填写股票代码"
                            return@Button
                        }
                        val t = threshold.toDoubleOrNull()
                        if (t == null || t <= 0 || t >= 1) {
                            error = "阈值需在 0~1 之间"
                            return@Button
                        }
                        """{"type":"price_drop","threshold":$t}"""
                    } else {
                        if (keyword.isBlank()) {
                            error = "新闻条件需填写关键词"
                            return@Button
                        }
                        """{"type":"keyword","value":"$keyword"}"""
                    }
                    error = null
                    if (existing == null) {
                        viewModel.saveNew(
                            name = name.trim(),
                            kind = kind,
                            symbol = if (kind == "trade") symbol.trim() else null,
                            expression = expression,
                            enabled = enabled,
                        )
                    } else {
                        viewModel.update(
                            existing.copy(
                                name = name.trim(),
                                kind = kind,
                                symbol = if (kind == "trade") symbol.trim() else null,
                                expression = expression,
                                enabled = enabled,
                            ),
                        )
                    }
                    onDone()
                },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("保存")
            }
        }
    }
}
