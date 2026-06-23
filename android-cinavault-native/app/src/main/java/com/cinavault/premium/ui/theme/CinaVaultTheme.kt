package com.cinavault.premium.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

object CinaVaultColors {
    val Ink = Color(0xFF050713)
    val InkElevated = Color(0xFF0A1024)
    val Panel = Color(0xCC111A34)
    val PanelSoft = Color(0x991A2445)
    val Border = Color(0x33FFFFFF)
    val Text = Color(0xFFEAF2FF)
    val Subtext = Color(0xFFA9B7D0)
    val Muted = Color(0xFF6E7C96)
    val Accent = Color(0xFF72F7FF)
    val Accent2 = Color(0xFFB16CFF)
    val Success = Color(0xFF61FFB1)
    val Warning = Color(0xFFFFC857)
    val Critical = Color(0xFFFF5C7A)
    val DeepBlue = Color(0xFF08152F)
}

private val CinaVaultDarkScheme: ColorScheme = darkColorScheme(
    primary = CinaVaultColors.Accent,
    secondary = CinaVaultColors.Accent2,
    tertiary = CinaVaultColors.Success,
    background = CinaVaultColors.Ink,
    surface = CinaVaultColors.InkElevated,
    surfaceVariant = CinaVaultColors.PanelSoft,
    onPrimary = CinaVaultColors.Ink,
    onSecondary = CinaVaultColors.Text,
    onTertiary = CinaVaultColors.Ink,
    onBackground = CinaVaultColors.Text,
    onSurface = CinaVaultColors.Text,
    onSurfaceVariant = CinaVaultColors.Subtext,
    error = CinaVaultColors.Critical,
    onError = CinaVaultColors.Text
)

@Composable
fun CinaVaultTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = CinaVaultDarkScheme,
        typography = CinaVaultTypography,
        content = content
    )
}
