package com.lingxi.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * 金融助手标识：四角星芒 + 小星点，贴近主流 App 的 AI 入口而非卡通机器人。
 */
val AssistantMark: ImageVector
    get() {
        if (_assistantMark != null) return _assistantMark!!
        _assistantMark = ImageVector.Builder(
            name = "AssistantMark",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            // 主星芒（柔和四角）
            path(
                fill = SolidColor(Color.Black),
                pathFillType = PathFillType.EvenOdd,
            ) {
                moveTo(11.2f, 3.2f)
                curveTo(11.45f, 2.45f, 12.55f, 2.45f, 12.8f, 3.2f)
                lineTo(14.05f, 7.05f)
                curveTo(14.15f, 7.35f, 14.4f, 7.6f, 14.7f, 7.7f)
                lineTo(18.55f, 8.95f)
                curveTo(19.3f, 9.2f, 19.3f, 10.3f, 18.55f, 10.55f)
                lineTo(14.7f, 11.8f)
                curveTo(14.4f, 11.9f, 14.15f, 12.15f, 14.05f, 12.45f)
                lineTo(12.8f, 16.3f)
                curveTo(12.55f, 17.05f, 11.45f, 17.05f, 11.2f, 16.3f)
                lineTo(9.95f, 12.45f)
                curveTo(9.85f, 12.15f, 9.6f, 11.9f, 9.3f, 11.8f)
                lineTo(5.45f, 10.55f)
                curveTo(4.7f, 10.3f, 4.7f, 9.2f, 5.45f, 8.95f)
                lineTo(9.3f, 7.7f)
                curveTo(9.6f, 7.6f, 9.85f, 7.35f, 9.95f, 7.05f)
                close()
            }
            // 右上小星点
            path(fill = SolidColor(Color.Black)) {
                moveTo(18.15f, 3.55f)
                curveTo(18.3f, 3.15f, 18.85f, 3.15f, 19.0f, 3.55f)
                lineTo(19.45f, 4.7f)
                curveTo(19.5f, 4.85f, 19.65f, 4.95f, 19.8f, 5.0f)
                lineTo(20.95f, 5.45f)
                curveTo(21.35f, 5.6f, 21.35f, 6.15f, 20.95f, 6.3f)
                lineTo(19.8f, 6.75f)
                curveTo(19.65f, 6.8f, 19.5f, 6.95f, 19.45f, 7.1f)
                lineTo(19.0f, 8.25f)
                curveTo(18.85f, 8.65f, 18.3f, 8.65f, 18.15f, 8.25f)
                lineTo(17.7f, 7.1f)
                curveTo(17.65f, 6.95f, 17.5f, 6.8f, 17.35f, 6.75f)
                lineTo(16.2f, 6.3f)
                curveTo(15.8f, 6.15f, 15.8f, 5.6f, 16.2f, 5.45f)
                lineTo(17.35f, 5.0f)
                curveTo(17.5f, 4.95f, 17.65f, 4.85f, 17.7f, 4.7f)
                close()
            }
        }.build()
        return _assistantMark!!
    }

private var _assistantMark: ImageVector? = null

/**
 * 顶栏 / 列表入口：浅色圆底 + 主色星芒，避免裸图标显得廉价。
 */
@Composable
fun AssistantEntryIcon(
    modifier: Modifier = Modifier,
    size: Dp = 32.dp,
    iconSize: Dp = 18.dp,
    contentDescription: String? = "助手",
) {
    val bg = MaterialTheme.colorScheme.primary.copy(alpha = 0.12f)
    val fg = MaterialTheme.colorScheme.primary
    Box(
        modifier = modifier
            .size(size)
            .background(bg, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = AssistantMark,
            contentDescription = contentDescription,
            modifier = Modifier.size(iconSize),
            tint = fg,
        )
    }
}
