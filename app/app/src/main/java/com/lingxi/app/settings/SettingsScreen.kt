package com.lingxi.app.settings

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
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.data.model.ServerConfig
import com.lingxi.app.viewmodel.SettingsViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    viewModel: SettingsViewModel,
    onBack: () -> Unit,
) {
    val saved by viewModel.config.collectAsStateWithLifecycle()
    val message by viewModel.message.collectAsStateWithLifecycle()
    val testing by viewModel.testing.collectAsStateWithLifecycle()

    var host by remember { mutableStateOf(saved.host) }
    var httpPort by remember { mutableStateOf(saved.httpPort.toString()) }
    var wsPort by remember { mutableStateOf(saved.wsPort.toString()) }
    var useTls by remember { mutableStateOf(saved.useTls) }
    var wsPath by remember { mutableStateOf(saved.wsPath) }

    LaunchedEffect(saved) {
        host = saved.host
        httpPort = saved.httpPort.toString()
        wsPort = saved.wsPort.toString()
        useTls = saved.useTls
        wsPath = saved.wsPath
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("服务器设置") },
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
            Text(
                "当前为 Mock 模式：设置仅保存在本地，「测试连接」不会请求真实 API。",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            OutlinedTextField(
                value = host,
                onValueChange = { host = it },
                label = { Text("服务器地址") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                placeholder = { Text("10.0.2.2 或局域网 IP") },
            )
            OutlinedTextField(
                value = httpPort,
                onValueChange = { httpPort = it.filter { c -> c.isDigit() } },
                label = { Text("HTTP 端口") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            OutlinedTextField(
                value = wsPort,
                onValueChange = { wsPort = it.filter { c -> c.isDigit() } },
                label = { Text("WebSocket 端口") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            OutlinedTextField(
                value = wsPath,
                onValueChange = { wsPath = it },
                label = { Text("WebSocket 路径") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("启用 TLS", modifier = Modifier.weight(1f))
                Switch(checked = useTls, onCheckedChange = { useTls = it })
            }

            Spacer(modifier = Modifier.height(8.dp))
            Button(
                onClick = {
                    viewModel.save(
                        ServerConfig(
                            host = host,
                            httpPort = httpPort.toIntOrNull() ?: 8000,
                            wsPort = wsPort.toIntOrNull() ?: 8000,
                            useTls = useTls,
                            wsPath = wsPath,
                        ),
                    )
                },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("保存")
            }
            OutlinedButton(
                onClick = { viewModel.testConnection() },
                enabled = !testing,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (testing) "测试中…" else "测试连接（Mock）")
            }

            message?.let {
                Text(it, color = MaterialTheme.colorScheme.primary)
            }
        }
    }
}
