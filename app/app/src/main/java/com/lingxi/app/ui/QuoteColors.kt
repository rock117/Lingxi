package com.lingxi.app.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** A 股习惯：涨红、跌绿、平为当前正文色。 */
object QuoteColors {
    val Up = Color(0xFFC62828)
    val Down = Color(0xFF2E7D32)

    @Composable
    fun of(changePct: Double, eps: Double = 1e-6): Color = when {
        changePct > eps -> Up
        changePct < -eps -> Down
        else -> MaterialTheme.colorScheme.onSurface
    }
}
