package com.sps.companion.network

import com.sps.companion.data.SpsBriefing
import com.sps.companion.data.SpsCompletionRequest
import com.sps.companion.data.SpsCompletionResponse
import com.sps.companion.data.SpsCreateGoalRequest
import com.sps.companion.data.SpsDeviceApp
import com.sps.companion.data.SpsDispatchRequest
import com.sps.companion.data.SpsDispatchResponse
import com.sps.companion.data.SpsEvent
import com.sps.companion.data.SpsGoal
import com.sps.companion.data.SpsHealth
import com.sps.companion.data.SpsMemory
import com.sps.companion.data.SpsStreamToken
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources
import okio.IOException
import java.util.concurrent.TimeUnit

/**
 * SPS Client — talks to the local SPS Rust kernel over HTTP + SSE.
 *
 * All endpoints are blocking-ok because we're on a mobile device and the
 * kernel is on 127.0.0.1 (no network latency). Calls are dispatched on
 * Dispatchers.IO by default.
 *
 * Connection state is exposed as a [StateFlow] so the UI and services
 * can react to disconnects (e.g. when Termux SPS server dies).
 */
class SpsClient(
    /** Base URL of the SPS server, e.g. "http://127.0.0.1:7780". */
    private var baseUrl: String,
) {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = false
    }

    private val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(5, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .writeTimeout(30, TimeUnit.SECONDS)
        .retryOnConnectionFailure(true)
        .build()

    private val sseFactory = EventSources.createFactory(http)

    private val _connectionState = MutableStateFlow<SpsConnectionState>(SpsConnectionState.Disconnected)
    val connectionState: StateFlow<SpsConnectionState> = _connectionState.asStateFlow()

    /** Update the base URL and reconnect. */
    fun setBaseUrl(url: String) {
        baseUrl = url.trimEnd('/')
        _connectionState.value = SpsConnectionState.Disconnected
    }

    /** Health check — pings /api/health and updates [connectionState]. */
    suspend fun healthCheck(): SpsHealth = withContext(Dispatchers.IO) {
        try {
            _connectionState.value = SpsConnectionState.Connecting
            val resp = get("$baseUrl/api/health")
            if (resp.isSuccessful) {
                val health = json.decodeFromString(SpsHealth.serializer(), resp.body!!.string())
                _connectionState.value = SpsConnectionState.Connected
                health
            } else {
                _connectionState.value = SpsConnectionState.Failed("HTTP ${resp.code}")
                SpsHealth(status = "error", version = "unknown")
            }
        } catch (e: Exception) {
            _connectionState.value = SpsConnectionState.Failed(e.message ?: "unknown")
            SpsHealth(status = "error", version = "unknown")
        }
    }

    /** List all goals (active, completed, paused). */
    suspend fun listGoals(): List<SpsGoal> = withContext(Dispatchers.IO) {
        getJson("$baseUrl/api/goals")
    }

    /** Create a new long-term goal. Triggers autonomous milestone+task generation. */
    suspend fun createGoal(req: SpsCreateGoalRequest): SpsGoal = withContext(Dispatchers.IO) {
        postJson("$baseUrl/api/goals", req)
    }

    /** Delete a goal by id. */
    suspend fun deleteGoal(id: String): Boolean = withContext(Dispatchers.IO) {
        delete("$baseUrl/api/goals/$id").isSuccessful
    }

    /** Get today's daily briefing. */
    suspend fun getBriefing(): SpsBriefing = withContext(Dispatchers.IO) {
        getJson("$baseUrl/api/companion/briefing")
    }

    /** List recent memories. */
    suspend fun listMemories(limit: Int = 20): List<SpsMemory> = withContext(Dispatchers.IO) {
        getJson("$baseUrl/api/memory?limit=$limit")
    }

    /** Search memories by query. */
    suspend fun searchMemories(query: String): List<SpsMemory> = withContext(Dispatchers.IO) {
        getJson("$baseUrl/api/memory/search?q=${java.net.URLEncoder.encode(query, "UTF-8")}")
    }

    /** Store a new memory. */
    suspend fun storeMemory(content: String, type: String = "episodic"): SpsMemory = withContext(Dispatchers.IO) {
        val payload = mapOf("content" to content, "memory_type" to type, "importance" to 0.7, "source" to "companion")
        postJson("$baseUrl/api/memory", payload)
    }

    /** List installed apps (for device control UI). */
    suspend fun listApps(): List<SpsDeviceApp> = withContext(Dispatchers.IO) {
        getJson("$baseUrl/api/device/apps")
    }

    /** Launch an app by package name. */
    suspend fun launchApp(packageName: String): Boolean = withContext(Dispatchers.IO) {
        val payload = mapOf("package" to packageName)
        val resp = post("$baseUrl/api/device/launch", payload)
        resp.isSuccessful
    }

    /** Dispatch a raw event to the kernel (advances the hash chain). */
    suspend fun dispatch(req: SpsDispatchRequest): SpsDispatchResponse = withContext(Dispatchers.IO) {
        postJson("$baseUrl/api/dispatch", req)
    }

    /** List recent events (default 50). */
    suspend fun listEvents(limit: Int = 50): List<SpsEvent> = withContext(Dispatchers.IO) {
        getJson("$baseUrl/api/events?limit=$limit")
    }

    /** Non-streaming LLM completion. */
    suspend fun complete(req: SpsCompletionRequest): SpsCompletionResponse = withContext(Dispatchers.IO) {
        postJson("$baseUrl/api/llm/complete", req)
    }

    /**
     * Streaming LLM completion via SSE.
     *
     * Emits [SpsStreamToken]s as they arrive from the server. The final
     * token has [SpsStreamToken.isDone] = true. Cancelling the collector
     * cancels the stream.
     */
    fun streamComplete(req: SpsCompletionRequest): Flow<SpsStreamToken> = callbackFlow {
        val body = json.encodeToString(req).toRequestBody("application/json".toMediaType())
        val request = Request.Builder()
            .url("$baseUrl/api/llm/stream")
            .post(body)
            .build()

        val source = sseFactory.newEventSource(request, object : EventSourceListener() {
            override fun onEvent(eventSource: EventSource, id: String?, type: String?, data: String) {
                if (data == "[DONE]" || data.isEmpty()) {
                    trySend(SpsStreamToken(isDone = true))
                    close()
                    return
                }
                runCatching { json.decodeFromString(SpsStreamToken.serializer(), data) }
                    .onSuccess { trySend(it) }
                    .onFailure {
                        // Treat unparseable data as a plain delta.
                        trySend(SpsStreamToken(delta = data))
                    }
            }

            override fun onClosed(eventSource: EventSource) {
                close()
            }

            override fun onFailure(eventSource: EventSource, t: Throwable?, response: Response?) {
                close(t ?: IOException("SSE failure: ${response?.code}"))
            }
        })

        awaitClose { source.cancel() }
    }

    /**
     * Subscribe to the live event stream (SSE). Emits every new event the
     * kernel appends. Used by services that need to react to state changes
     * (e.g. "new goal created → speak confirmation").
     */
    fun streamEvents(): Flow<SpsEvent> = callbackFlow {
        val request = Request.Builder()
            .url("$baseUrl/api/events/stream")
            .build()

        val source = sseFactory.newEventSource(request, object : EventSourceListener() {
            override fun onEvent(eventSource: EventSource, id: String?, type: String?, data: String) {
                runCatching { json.decodeFromString(SpsEvent.serializer(), data) }
                    .onSuccess { trySend(it) }
            }
            override fun onClosed(eventSource: EventSource) { close() }
            override fun onFailure(eventSource: EventSource, t: Throwable?, response: Response?) {
                close(t ?: IOException("event stream failure"))
            }
        })
        awaitClose { source.cancel() }
    }

    /** Close the client — cancel all in-flight requests. */
    fun close() {
        http.dispatcher.executorService.shutdown()
        http.connectionPool.evictAll()
        _connectionState.value = SpsConnectionState.Disconnected
    }

    // ─── HTTP helpers ────────────────────────────────────────────────────

    private fun get(url: String): Response = http.newCall(Request.Builder().url(url).get().build()).execute()

    private fun post(url: String, body: Any): Response {
        val reqBody = json.encodeToString(body).toRequestBody("application/json".toMediaType())
        return http.newCall(Request.Builder().url(url).post(reqBody).build()).execute()
    }

    private fun delete(url: String): Response = http.newCall(Request.Builder().url(url).delete().build()).execute()

    private inline fun <reified T> getJson(url: String): T {
        val resp = get(url)
        if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: ${resp.body?.string()?.take(200)}")
        return json.decodeFromString<T>(resp.body!!.string())
    }

    private inline fun <reified T> postJson(url: String, body: Any): T {
        val resp = post(url, body)
        if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: ${resp.body?.string()?.take(200)}")
        return json.decodeFromString<T>(resp.body!!.string())
    }
}
