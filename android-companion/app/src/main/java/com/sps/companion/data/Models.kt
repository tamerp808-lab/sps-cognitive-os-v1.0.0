package com.sps.companion.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/**
 * SPS server DTOs — mirror the Rust kernel's JSON shapes.
 *
 * These are intentionally permissive (nullable fields, defaults) because
 * the SPS server is under active development and may add fields.
 */

@Serializable
data class SpsHealth(
    val status: String = "ok",
    val version: String = "unknown",
    val kernelBooted: Boolean = false,
    val backend: String = "unknown",
    val eventCount: Long = 0,
)

@Serializable
data class SpsEvent(
    val tick: Long = 0,
    val eventType: String = "",
    val payload: JsonElement? = null,
    val actor: SpsActor? = null,
    val wallTime: Long = 0,
    val hash: String = "",
    val prevHash: String = "",
)

@Serializable
data class SpsActor(
    val kind: String = "user",
    val id: String = "owner",
)

@Serializable
data class SpsGoal(
    val id: String = "",
    val title: String = "",
    val description: String = "",
    val targetDate: String? = null,
    val progress: Double = 0.0,
    val status: String = "active",
    val milestones: List<SpsMilestone> = emptyList(),
    val tasks: List<SpsTask> = emptyList(),
    val createdAt: Long = 0,
    val category: String = "general",
)

@Serializable
data class SpsMilestone(
    val id: String = "",
    val title: String = "",
    val description: String = "",
    val completed: Boolean = false,
    val targetDate: String? = null,
)

@Serializable
data class SpsTask(
    val id: String = "",
    val title: String = "",
    val completed: Boolean = false,
    val dueDate: String? = null,
    val priority: Int = 0,
)

@Serializable
data class SpsMemory(
    val id: String = "",
    val content: String = "",
    val memoryType: String = "episodic",
    val importance: Double = 0.5,
    val createdAt: Long = 0,
    val lastAccessedAt: Long = 0,
    val tags: List<String> = emptyList(),
    val source: String = "companion",
    val decayScore: Double = 1.0,
)

@Serializable
data class SpsDeviceApp(
    val packageName: String = "",
    val label: String = "",
    val isSystem: Boolean = false,
    val category: String = "other",
)

@Serializable
data class SpsChatMessage(
    val role: String = "user", // "user" | "assistant" | "system"
    val content: String = "",
    val timestamp: Long = 0,
    val streaming: Boolean = false,
)

@Serializable
data class SpsBriefing(
    val date: String = "",
    val greeting: String = "Good morning",
    val summary: String = "",
    val topGoals: List<SpsGoal> = emptyList(),
    val topTasks: List<SpsTask> = emptyList(),
    val recentMemories: List<SpsMemory> = emptyList(),
    val weather: String? = null,
    val schedule: List<String> = emptyList(),
)

@Serializable
data class SpsCompletionRequest(
    val prompt: String,
    val system: String? = null,
    val providerId: String? = null,
    val conversationId: String? = null,
)

@Serializable
data class SpsCompletionResponse(
    val text: String = "",
    val provider: String = "",
    val model: String = "",
    val elapsedMs: Long = 0,
)

@Serializable
data class SpsCreateGoalRequest(
    val title: String,
    val description: String = "",
    val targetDate: String? = null,
    val category: String = "general",
)

@Serializable
data class SpsDispatchRequest(
    val eventType: String,
    val payload: JsonElement,
    val actorKind: String = "user",
    val actorId: String = "owner",
)

@Serializable
data class SpsDispatchResponse(
    val tick: Long = 0,
    val hash: String = "",
    val eventType: String = "",
)

/** A single token from a streaming LLM response. */
@Serializable
data class SpsStreamToken(
    val delta: String = "",
    @SerialName("is_done") val isDone: Boolean = false,
    val model: String = "",
)
