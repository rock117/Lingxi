package com.lingxi.app.market

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Assessment
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.lingxi.app.notification.NotificationListScreen
import com.lingxi.app.viewmodel.MarketViewModel
import com.lingxi.app.viewmodel.NotificationViewModel

private enum class MoreDest {
    Menu,
    FundStats,
    Notifications,
}

@Composable
fun MoreTab(
    marketViewModel: MarketViewModel,
    notificationViewModel: NotificationViewModel,
) {
    var dest by rememberSaveable { mutableStateOf(MoreDest.Menu) }
    val unread by notificationViewModel.unreadCount.collectAsStateWithLifecycle()

    when (dest) {
        MoreDest.Menu -> MoreMenuScreen(
            unread = unread,
            onFundStats = { dest = MoreDest.FundStats },
            onNotifications = { dest = MoreDest.Notifications },
        )
        MoreDest.FundStats -> Column(modifier = Modifier.fillMaxSize()) {
            MoreSubHeader(title = "A股财报统计", onBack = { dest = MoreDest.Menu })
            FundStatsScreen(viewModel = marketViewModel)
        }
        MoreDest.Notifications -> Column(modifier = Modifier.fillMaxSize()) {
            MoreSubHeader(title = "通知", onBack = { dest = MoreDest.Menu })
            NotificationListScreen(viewModel = notificationViewModel)
        }
    }
}

@Composable
private fun MoreSubHeader(title: String, onBack: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
        }
        Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
private fun MoreMenuScreen(
    unread: Int,
    onFundStats: () -> Unit,
    onNotifications: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp, vertical = 8.dp),
    ) {
        Text(
            "更多",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Text(
            "低频入口与后续扩展",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(16.dp))
        MoreMenuRow(
            icon = Icons.Default.Assessment,
            title = "财报统计",
            subtitle = "全市场赚钱占比 · 增长 · 现金流",
            onClick = onFundStats,
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
        MoreMenuRow(
            icon = Icons.Default.Notifications,
            title = "通知",
            subtitle = if (unread > 0) "未读 $unread 条" else "日内强势等推送",
            badge = unread,
            onClick = onNotifications,
        )
    }
}

@Composable
private fun MoreMenuRow(
    icon: ImageVector,
    title: String,
    subtitle: String,
    badge: Int = 0,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        BadgedBox(
            badge = {
                if (badge > 0) {
                    Badge { Text(if (badge > 99) "99+" else badge.toString()) }
                }
            },
        ) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(28.dp))
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(title, fontWeight = FontWeight.Medium)
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Icon(
            Icons.AutoMirrored.Filled.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
