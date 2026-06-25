package com.sps.companion.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val SpsDarkColorScheme = darkColorScheme(
    primary = SpsPrimary,
    onPrimary = SpsOnPrimary,
    primaryContainer = SpsPrimaryDark,
    onPrimaryContainer = SpsOnPrimary,
    secondary = SpsSecondary,
    onSecondary = SpsOnPrimary,
    secondaryContainer = SpsSecondaryDark,
    onSecondaryContainer = SpsOnPrimary,
    tertiary = SpsTertiary,
    onTertiary = SpsOnPrimary,
    background = SpsBackground,
    onBackground = SpsOnBackground,
    surface = SpsSurface,
    onSurface = SpsOnSurface,
    surfaceVariant = SpsSurfaceVariant,
    onSurfaceVariant = SpsOnSurfaceVariant,
    error = SpsError,
)

private val SpsLightColorScheme = lightColorScheme(
    primary = SpsPrimaryDark,
    onPrimary = SpsOnPrimary,
    secondary = SpsSecondaryDark,
    onSecondary = SpsOnPrimary,
    tertiary = SpsTertiary,
    background = Color(0xFFF5F7FF),
    onBackground = Color(0xFF0A0E1F),
    surface = Color(0xFFFFFFFF),
    onSurface = Color(0xFF0A0E1F),
    surfaceVariant = Color(0xFFE8ECFB),
    onSurfaceVariant = Color(0xFF555771),
    error = SpsError,
)

@Composable
fun SpsTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val colors = if (darkTheme) SpsDarkColorScheme else SpsLightColorScheme
    MaterialTheme(
        colorScheme = colors,
        typography = SpsTypography,
        content = content,
    )
}
