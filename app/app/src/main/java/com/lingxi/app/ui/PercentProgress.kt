package com.lingxi.app.ui

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * 统一进度条样式（与情绪页一致）：圆角轨道、无末端 stop 圆点。
 * [percent] 取值 0–100。
 */
@Composable
fun PercentProgress(
    percent: Double,
    modifier: Modifier = Modifier,
    height: Dp = 8.dp,
    color: Color = MaterialTheme.colorScheme.primary,
    trackColor: Color = MaterialTheme.colorScheme.surfaceVariant,
) {
    LinearProgressIndicator(
        progress = { (percent / 100.0).toFloat().coerceIn(0f, 1f) },
        modifier = modifier
            .fillMaxWidth()
            .height(height)
            .clip(RoundedCornerShape(4.dp)),
        color = color,
        trackColor = trackColor,
        strokeCap = StrokeCap.Butt,
        gapSize = 0.dp,
        drawStopIndicator = {},
    )
}
