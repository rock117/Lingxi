package com.lingxi.app.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val Blue = Color(0xFF1B4F72)
private val BlueLight = Color(0xFF2E86C1)
private val SurfaceLight = Color(0xFFF5F7FA)

private val LightColors = lightColorScheme(
    primary = Blue,
    secondary = BlueLight,
    tertiary = Color(0xFF117A65),
    background = SurfaceLight,
    surface = Color.White,
)

private val DarkColors = darkColorScheme(
    primary = BlueLight,
    secondary = Color(0xFF85C1E9),
    tertiary = Color(0xFF48C9B0),
)

@Composable
fun LingxiTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content,
    )
}
