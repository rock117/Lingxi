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
import androidx.compose.material.icons.filled.ShowChart
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lingxi.app.ui.AssistantEntryIcon
import com.lingxi.app.viewmodel.MarketViewModel

private enum class MoreDest {
    Menu,
    FundStats,
    EtfShare,
}

@Composable
fun MoreTab(
    marketViewModel: MarketViewModel,
    onOpenAssistant: () -> Unit = {},
) {
    var dest by rememberSaveable { mutableStateOf(MoreDest.Menu) }

    when (dest) {
        MoreDest.Menu -> MoreMenuScreen(
            onFundStats = { dest = MoreDest.FundStats },
            onEtfShare = { dest = MoreDest.EtfShare },
            onAssistant = onOpenAssistant,
        )
        MoreDest.FundStats -> Column(modifier = Modifier.fillMaxSize()) {
            MoreSubHeader(title = "A股财报统计", onBack = { dest = MoreDest.Menu })
            FundStatsScreen(viewModel = marketViewModel)
        }
        MoreDest.EtfShare -> Column(modifier = Modifier.fillMaxSize()) {
            MoreSubHeader(title = "宽基ETF份额", onBack = { dest = MoreDest.Menu })
            EtfShareScreen(viewModel = marketViewModel)
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
    onFundStats: () -> Unit,
    onEtfShare: () -> Unit,
    onAssistant: () -> Unit,
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
            icon = {
                AssistantEntryIcon(
                    size = 36.dp,
                    iconSize = 20.dp,
                    contentDescription = null,
                )
            },
            title = "智能助手",
            subtitle = "金融问答 · 多轮对话",
            onClick = onAssistant,
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
        MoreMenuRow(
            icon = {
                Icon(
                    Icons.Default.Assessment,
                    contentDescription = null,
                    modifier = Modifier.size(28.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            title = "财报统计",
            subtitle = "全市场赚钱占比 · 增长 · 现金流",
            onClick = onFundStats,
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
        MoreMenuRow(
            icon = {
                Icon(
                    Icons.Default.ShowChart,
                    contentDescription = null,
                    modifier = Modifier.size(28.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            title = "宽基ETF份额",
            subtitle = "沪深300 / 上证50 / 科创50 / 创业板 · 份额与市值",
            onClick = onEtfShare,
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
    }
}

@Composable
private fun MoreMenuRow(
    icon: @Composable () -> Unit,
    title: String,
    subtitle: String,
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
        icon()
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
