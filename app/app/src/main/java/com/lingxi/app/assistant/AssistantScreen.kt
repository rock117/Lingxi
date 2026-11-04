package com.lingxi.app.assistant

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.AddComment
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.ChatUiMessage
import com.lingxi.app.viewmodel.AssistantViewModel
import kotlinx.coroutines.launch

private val ExampleQuestions = listOf(
    "今天市场情绪怎么样？",
    "帮我看看贵州茅台近期表现",
    "全市场赚钱面和去年比如何？",
    "有哪些超跌放量的股票？",
)

private data class SceneChip(val label: String, val prompt: String)

private val SceneShortcuts = listOf(
    SceneChip("情绪", "今天市场情绪怎么样？"),
    SceneChip("个股", "帮我看看贵州茅台近期表现"),
    SceneChip("财报", "全市场赚钱面和去年比如何？"),
    SceneChip("超跌", "有哪些超跌放量的股票？"),
    SceneChip("选股", "帮我筛选近5日放量且均线多头的股票"),
)

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun AssistantScreen(
    viewModel: AssistantViewModel,
    prefill: String? = null,
    onBack: () -> Unit,
) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val listState = rememberLazyListState()

    LaunchedEffect(prefill) {
        viewModel.bootstrap(prefill)
    }

    LaunchedEffect(ui.messages.size, ui.messages.lastOrNull()?.content) {
        if (ui.messages.isNotEmpty()) {
            listState.animateScrollToItem(ui.messages.lastIndex)
        }
    }

    LaunchedEffect(ui.showHistory) {
        if (ui.showHistory) {
            drawerState.open()
        } else if (drawerState.isOpen) {
            drawerState.close()
        }
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet(modifier = Modifier.widthIn(max = 320.dp)) {
                Text(
                    "历史会话",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(16.dp),
                )
                if (ui.sessions.isEmpty()) {
                    Text(
                        "暂无会话",
                        modifier = Modifier.padding(horizontal = 16.dp),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                ui.sessions.forEach { s ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable {
                                viewModel.openSession(s.id)
                                scope.launch { drawerState.close() }
                                viewModel.toggleHistory(false)
                            }
                            .padding(horizontal = 16.dp, vertical = 10.dp),
                    ) {
                        Text(s.title.ifBlank { "未命名会话" }, fontWeight = FontWeight.Medium)
                        Text(
                            "${s.messageCount} 条 · ${s.updatedAt.take(16)}",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        TextButton(onClick = { viewModel.deleteSession(s.id) }) {
                            Text("删除")
                        }
                    }
                    HorizontalDivider()
                }
            }
        },
    ) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text("助手") },
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                        }
                    },
                    actions = {
                        IconButton(onClick = { viewModel.newChat() }) {
                            Icon(Icons.Default.AddComment, contentDescription = "新建对话")
                        }
                        IconButton(onClick = {
                            viewModel.toggleHistory(true)
                            scope.launch { drawerState.open() }
                        }) {
                            Icon(Icons.Default.History, contentDescription = "历史")
                        }
                    },
                )
            },
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
            ) {
                if (ui.status?.aiEnabled == false) {
                    Surface(
                        color = MaterialTheme.colorScheme.errorContainer,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            "助手未启用（服务端未配置 AI）。其它行情功能仍可用。",
                            modifier = Modifier.padding(12.dp),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
                ui.error?.let { err ->
                    Text(
                        err,
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                    )
                }

                Box(modifier = Modifier.weight(1f)) {
                    if (ui.loading) {
                        CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
                    } else if (ui.messages.isEmpty()) {
                        EmptyState(
                            symbolPrefill = prefill,
                            onPick = { viewModel.send(it) },
                        )
                    } else {
                        LazyColumn(
                            state = listState,
                            modifier = Modifier.fillMaxSize(),
                            contentPadding = PaddingValues(16.dp),
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            items(ui.messages, key = { it.localId }) { msg ->
                                MessageBubble(
                                    msg = msg,
                                    onToggleSteps = { viewModel.toggleSteps(msg.localId) },
                                    onSuggestion = { viewModel.setDraft(it) },
                                )
                            }
                        }
                    }
                }

                SceneShortcutRow(
                    enabled = !ui.sending && ui.status?.aiEnabled != false,
                    onPick = { viewModel.setDraft(it) },
                )

                InputBar(
                    value = ui.draft,
                    enabled = ui.status?.aiEnabled != false,
                    sending = ui.sending,
                    onChange = viewModel::setDraft,
                    onSend = { viewModel.send() },
                    onStop = { viewModel.stop() },
                )
                Text(
                    "AI 生成，仅供参考，非投资建议",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 6.dp),
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EmptyState(symbolPrefill: String?, onPick: (String) -> Unit) {
    val chips = if (!symbolPrefill.isNullOrBlank()) {
        val sym = symbolPrefill.trim()
        listOf(
            "帮我看看 $sym 近期表现",
            "$sym 基本面和财报有什么要点？",
            "今天市场情绪怎么样？",
        )
    } else {
        ExampleQuestions
    }
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("Lingxi 金融助手", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        Text(
            "可问大盘情绪、个股、财报面、超跌与选股",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(20.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            chips.forEach { q ->
                FilterChip(
                    selected = false,
                    onClick = { onPick(q) },
                    label = { Text(q) },
                )
            }
        }
    }
}

@Composable
private fun SceneShortcutRow(enabled: Boolean, onPick: (String) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        SceneShortcuts.forEach { s ->
            FilterChip(
                selected = false,
                enabled = enabled,
                onClick = { onPick(s.prompt) },
                label = { Text(s.label) },
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MessageBubble(
    msg: ChatUiMessage,
    onToggleSteps: () -> Unit,
    onSuggestion: (String) -> Unit,
) {
    val isUser = msg.role == "user"
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = if (isUser) Arrangement.End else Arrangement.Start,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 320.dp)
                .background(
                    color = if (isUser) {
                        MaterialTheme.colorScheme.primaryContainer
                    } else {
                        MaterialTheme.colorScheme.surfaceVariant
                    },
                    shape = RoundedCornerShape(12.dp),
                )
                .padding(12.dp),
        ) {
            if (!isUser && msg.steps.isNotEmpty()) {
                Text(
                    if (msg.streaming) {
                        msg.steps.lastOrNull()?.label ?: "处理中…"
                    } else {
                        "已完成 · 用了 ${msg.steps.size} 步"
                    },
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable(onClick = onToggleSteps),
                )
                if (msg.stepsExpanded || msg.streaming) {
                    Spacer(Modifier.height(6.dp))
                    msg.steps.forEach { step ->
                        val suffix = step.summary?.let { " · $it" }.orEmpty()
                        Text(
                            "· ${step.label}$suffix",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Spacer(Modifier.height(8.dp))
                }
            }
            if (msg.content.isNotBlank()) {
                Text(msg.content, style = MaterialTheme.typography.bodyMedium)
            } else if (msg.streaming) {
                Text("…", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (!isUser && !msg.streaming && msg.suggestions.isNotEmpty()) {
                Spacer(Modifier.height(8.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    msg.suggestions.take(3).forEach { s ->
                        FilterChip(
                            selected = false,
                            onClick = { onSuggestion(s) },
                            label = { Text(s, style = MaterialTheme.typography.labelSmall) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun InputBar(
    value: String,
    enabled: Boolean,
    sending: Boolean,
    onChange: (String) -> Unit,
    onSend: () -> Unit,
    onStop: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        BasicTextField(
            value = value,
            onValueChange = onChange,
            enabled = enabled && !sending,
            modifier = Modifier
                .weight(1f)
                .heightIn(min = 44.dp, max = 120.dp)
                .background(
                    MaterialTheme.colorScheme.surfaceVariant,
                    RoundedCornerShape(12.dp),
                )
                .padding(horizontal = 12.dp, vertical = 10.dp),
            textStyle = MaterialTheme.typography.bodyMedium.copy(
                color = MaterialTheme.colorScheme.onSurface,
            ),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
            keyboardActions = KeyboardActions(onSend = { if (!sending) onSend() }),
            decorationBox = { inner ->
                if (value.isEmpty()) {
                    Text(
                        "问问情绪、个股、财报或选股…",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                inner()
            },
        )
        IconButton(
            onClick = { if (sending) onStop() else onSend() },
            enabled = enabled && (sending || value.isNotBlank()),
        ) {
            Icon(
                if (sending) Icons.Default.Stop else Icons.AutoMirrored.Filled.Send,
                contentDescription = if (sending) "停止" else "发送",
                modifier = Modifier.size(22.dp),
            )
        }
    }
}
