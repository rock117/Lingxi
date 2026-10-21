package com.lingxi.app.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Assessment
import androidx.compose.material.icons.filled.ListAlt
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.ShowChart
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
import com.lingxi.app.condition.StrategyListScreen
import com.lingxi.app.market.FundStatsScreen
import com.lingxi.app.market.SentimentScreen
import com.lingxi.app.notification.NotificationListScreen
import com.lingxi.app.viewmodel.MarketViewModel
import com.lingxi.app.viewmodel.NotificationViewModel
import com.lingxi.app.viewmodel.ScreenViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    marketViewModel: MarketViewModel,
    screenViewModel: ScreenViewModel,
    notificationViewModel: NotificationViewModel,
    onOpenSettings: () -> Unit,
    onComposeScreen: (String?) -> Unit,
    onEditScreen: (Long) -> Unit,
    onRunScreen: (Long) -> Unit,
) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val unread by notificationViewModel.unreadCount.collectAsStateWithLifecycle()

    val title = when (tab) {
        0 -> "短线情绪 · 中期结构"
        1 -> "选股策略"
        2 -> "A股财报统计"
        else -> "通知"
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
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
                    icon = { Icon(Icons.Default.ShowChart, contentDescription = null) },
                    label = { Text("情绪") },
                )
                NavigationBarItem(
                    selected = tab == 1,
                    onClick = { tab = 1 },
                    icon = { Icon(Icons.Default.ListAlt, contentDescription = null) },
                    label = { Text("策略") },
                )
                NavigationBarItem(
                    selected = tab == 2,
                    onClick = { tab = 2 },
                    icon = { Icon(Icons.Default.Assessment, contentDescription = null) },
                    label = { Text("财报") },
                )
                NavigationBarItem(
                    selected = tab == 3,
                    onClick = { tab = 3 },
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
            if (tab == 1) {
                FloatingActionButton(onClick = { onComposeScreen(null) }) {
                    Icon(Icons.Default.Add, contentDescription = "新建选股策略")
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
                0 -> SentimentScreen(viewModel = marketViewModel)
                1 -> StrategyListScreen(
                    viewModel = screenViewModel,
                    onComposeFrom = onComposeScreen,
                    onEditSaved = onEditScreen,
                    onRunSaved = onRunScreen,
                )
                2 -> FundStatsScreen(viewModel = marketViewModel)
                else -> NotificationListScreen(viewModel = notificationViewModel)
            }
        }
    }
}
