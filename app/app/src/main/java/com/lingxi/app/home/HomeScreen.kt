package com.lingxi.app.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ListAlt
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.NotificationsActive
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.condition.ConditionListScreen
import com.lingxi.app.notification.NotificationListScreen
import com.lingxi.app.viewmodel.ConditionViewModel
import com.lingxi.app.viewmodel.NotificationViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    conditionViewModel: ConditionViewModel,
    notificationViewModel: NotificationViewModel,
    onOpenSettings: () -> Unit,
    onCreateCondition: () -> Unit,
    onEditCondition: (Long) -> Unit,
) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val unread by notificationViewModel.unreadCount.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (tab == 0) "条件" else "通知") },
                actions = {
                    IconButton(onClick = onOpenSettings) {
                        Icon(Icons.Default.Settings, contentDescription = "设置")
                    }
                },
            )
        },
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = tab == 0,
                    onClick = { tab = 0 },
                    icon = { Icon(Icons.Default.ListAlt, contentDescription = null) },
                    label = { Text("条件") },
                )
                NavigationBarItem(
                    selected = tab == 1,
                    onClick = { tab = 1 },
                    icon = {
                        BadgedBox(
                            badge = {
                                if (unread > 0) {
                                    Badge { Text(if (unread > 99) "99+" else unread.toString()) }
                                }
                            },
                        ) {
                            Icon(Icons.Default.Notifications, contentDescription = null)
                        }
                    },
                    label = { Text("通知") },
                )
            }
        },
        floatingActionButton = {
            when (tab) {
                0 -> FloatingActionButton(onClick = onCreateCondition) {
                    Icon(Icons.Default.Add, contentDescription = "新建条件")
                }
                else -> FloatingActionButton(onClick = { notificationViewModel.pushMock() }) {
                    Icon(Icons.Default.NotificationsActive, contentDescription = "模拟推送")
                }
            }
        },
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            when (tab) {
                0 -> ConditionListScreen(
                    viewModel = conditionViewModel,
                    onEdit = onEditCondition,
                )
                else -> NotificationListScreen(viewModel = notificationViewModel)
            }
        }
    }
}
