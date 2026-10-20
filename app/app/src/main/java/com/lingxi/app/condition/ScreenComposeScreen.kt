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
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.StrategyDef
import com.lingxi.app.viewmodel.ScreenViewModel
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonPrimitive

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScreenComposeScreen(
    viewModel: ScreenViewModel,
    initialTypeId: String?,
    onBack: () -> Unit,
    onSavedAndRun: (Long) -> Unit,
    onPreviewDone: () -> Unit,
) {
    val strategies by viewModel.strategies.collectAsStateWithLifecycle()
    val loading by viewModel.loading.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()
    val lastSaved by viewModel.lastSavedId.collectAsStateWithLifecycle()

    val rules = remember { mutableStateListOf<JsonObject>() }
    var name by remember { mutableStateOf("") }
    var showPicker by remember { mutableStateOf(false) }
    var seeded by remember { mutableStateOf(false) }

    LaunchedEffect(strategies, initialTypeId) {
        if (!seeded && strategies.isNotEmpty()) {
            seeded = true
            val seed = initialTypeId?.let { id -> strategies.find { it.typeId == id } }
            if (seed != null) {
                rules.add(viewModel.defaultRuleOf(seed))
                if (name.isBlank()) name = seed.name
            }
        }
    }

    LaunchedEffect(lastSaved) {
        lastSaved?.let { id ->
            viewModel.consumeLastSavedId()
            onSavedAndRun(id)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("组合选股策略") },
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
                label = { Text("策略名称") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            Text("规则（全部满足）", style = MaterialTheme.typography.titleSmall)
            rules.forEachIndexed { index, rule ->
                val typeId = rule["type"]?.jsonPrimitive?.contentOrNull.orEmpty()
                val def = strategies.find { it.typeId == typeId }
                RuleEditorCard(
                    def = def,
                    rule = rule,
                    onChange = { rules[index] = it },
                    onRemove = { rules.removeAt(index) },
                    applyParam = viewModel::applyParam,
                )
            }

            OutlinedButton(
                onClick = { showPicker = !showPicker },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Icon(Icons.Default.Add, contentDescription = null)
                Text(if (showPicker) "收起策略库" else "添加策略")
            }

            if (showPicker) {
                strategies.forEach { def ->
                    OutlinedButton(
                        onClick = {
                            rules.add(viewModel.defaultRuleOf(def))
                            if (name.isBlank()) name = def.name
                            showPicker = false
                        },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("${def.name} · ${def.summary}")
                    }
                }
            }

            error?.let {
                Text(it, color = MaterialTheme.colorScheme.error)
            }

            Spacer(modifier = Modifier.height(8.dp))
            Button(
                onClick = {
                    if (rules.isEmpty()) return@Button
                    viewModel.clearResult()
                    viewModel.preview(rules.toList())
                    onPreviewDone()
                },
                enabled = !loading && rules.isNotEmpty(),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (loading) "选股中…" else "预览选股")
            }
            Button(
                onClick = {
                    val n = name.trim().ifBlank { "组合策略" }
                    viewModel.saveScreen(n, rules.toList())
                },
                enabled = !loading && rules.isNotEmpty(),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("保存并选股")
            }
        }
    }
}

@Composable
private fun RuleEditorCard(
    def: StrategyDef?,
    rule: JsonObject,
    onChange: (JsonObject) -> Unit,
    onRemove: () -> Unit,
    applyParam: (JsonObject, String, String, Boolean) -> JsonObject,
) {
    val typeId = rule["type"]?.jsonPrimitive?.contentOrNull.orEmpty()
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    def?.name ?: typeId,
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = onRemove) {
                    Icon(Icons.Default.Delete, contentDescription = "移除")
                }
            }
            def?.params?.forEach { param ->
                val current = rule[param.key]
                when (param.kind) {
                    "enum" -> {
                        Text(param.label, style = MaterialTheme.typography.labelMedium)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            param.options.orEmpty().forEach { opt ->
                                val selected = current?.jsonPrimitive?.contentOrNull == opt.value
                                FilterChip(
                                    selected = selected,
                                    onClick = {
                                        onChange(applyParam(rule, param.key, opt.value, false))
                                    },
                                    label = { Text(opt.label) },
                                )
                            }
                        }
                    }
                    else -> {
                        val initial = current?.jsonPrimitive?.doubleOrNull?.toString()
                            ?: current?.jsonPrimitive?.contentOrNull
                            ?: runCatching { param.default.jsonPrimitive.content }.getOrDefault("")
                        var text by remember(typeId, param.key) { mutableStateOf(initial) }
                        OutlinedTextField(
                            value = text,
                            onValueChange = {
                                text = it
                                onChange(applyParam(rule, param.key, it, true))
                            },
                            label = { Text(param.label) },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                    }
                }
            }
        }
    }
}
