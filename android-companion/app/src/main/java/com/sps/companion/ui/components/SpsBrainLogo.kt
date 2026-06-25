package com.sps.companion.ui.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.sps.companion.ui.theme.SpsBrainGlow
import com.sps.companion.ui.theme.SpsBrainActive
import com.sps.companion.ui.theme.SpsPrimary

/**
 * Animated SPS Brain Logo — three orbiting electrons around a glowing core.
 *
 * When [active] is true, the electrons speed up and the glow brightens
 * (used in the Voice screen while listening).
 */
@Composable
fun SpsBrainLogo(
    modifier: Modifier = Modifier,
    active: Boolean = false,
    size: Int = 48,
) {
    val transition = rememberInfiniteTransition(label = "brain")
    val rotation1 by transition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(
            animation = tween(if (active) 800 else 2400, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "r1",
    )
    val rotation2 by transition.animateFloat(
        initialValue = 120f,
        targetValue = 480f,
        animationSpec = infiniteRepeatable(
            animation = tween(if (active) 1200 else 3200, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "r2",
    )
    val rotation3 by transition.animateFloat(
        initialValue = 240f,
        targetValue = 600f,
        animationSpec = infiniteRepeatable(
            animation = tween(if (active) 1000 else 2800, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "r3",
    )
    val pulse by transition.animateFloat(
        initialValue = 0.6f,
        targetValue = if (active) 1.0f else 0.8f,
        animationSpec = infiniteRepeatable(
            animation = tween(1200, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "pulse",
    )

    Canvas(modifier = modifier.size(size.dp)) {
        val center = Offset(this.size.width / 2, this.size.height / 2)
        val radius = this.size.minDimension / 2 * 0.85f
        val coreRadius = this.size.minDimension / 2 * 0.18f * pulse

        // Outer glow.
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(SpsBrainGlow.copy(alpha = 0.4f * pulse), Color.Transparent),
                center = center,
                radius = radius,
            ),
            center = center,
            radius = radius,
        )

        // Orbit rings.
        drawCircle(
            color = SpsPrimary.copy(alpha = 0.3f),
            center = center,
            radius = radius * 0.8f,
            style = Stroke(width = 1.5f),
        )
        drawCircle(
            color = SpsPrimary.copy(alpha = 0.2f),
            center = center,
            radius = radius * 0.95f,
            style = Stroke(width = 1f),
        )

        // Core.
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(SpsBrainActive, SpsPrimary),
                center = center,
                radius = coreRadius * 2,
            ),
            center = center,
            radius = coreRadius,
        )

        // Three electrons.
        val r = radius * 0.85f
        listOf(rotation1, rotation2, rotation3).forEach { angle ->
            val rad = Math.toRadians(angle.toDouble())
            val x = center.x + (r * kotlin.math.cos(rad)).toFloat()
            val y = center.y + (r * kotlin.math.sin(rad)).toFloat()
            drawCircle(
                color = SpsBrainActive,
                center = Offset(x, y),
                radius = coreRadius * 0.35f,
            )
        }
    }
}
