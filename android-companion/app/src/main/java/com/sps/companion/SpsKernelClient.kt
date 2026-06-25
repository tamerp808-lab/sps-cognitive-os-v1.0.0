// SPS Kernel Client — HTTP communication with the SPS server
//
// Phase 13: Connects the Android companion app to the SPS kernel
// via the HTTP API (Phase 11D + companion routes).

package com.sps.companion

import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.util.concurrent.TimeUnit

class SpsKernelClient(private val context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences("sps", Context.MODE_PRIVATE)

    private val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()

    private val baseUrl: String
        get() = prefs.getString("sps_server_url", "http://10.0.2.2:7780") ?: "http://10.0.2.2:7780"

    // ─── Goal Lifecycle ───────────────────────────────────────────────────

    suspend fun activateGoal(goalId: String, milestones: String = "{}"): Result<String> =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("goal_id", goalId)
                    put("milestones", JSONObject(milestones))
                }.toString()

                val request = Request.Builder()
                    .url("$baseUrl/api/companion/goal/activate")
                    .post(body.toRequestBody("application/json".toMediaType()))
                    .build()

                val response = client.newCall(request).execute()
                if (response.isSuccessful) {
                    Result.success(response.body?.string() ?: "{}")
                } else {
                    Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
            } catch (e: Exception) {
                Result.failure(e)
            }
        }

    suspend fun deactivateGoal(goalId: String): Result<String> =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("goal_id", goalId)
                }.toString()

                val request = Request.Builder()
                    .url("$baseUrl/api/companion/goal/deactivate")
                    .post(body.toRequestBody("application/json".toMediaType()))
                    .build()

                val response = client.newCall(request).execute()
                if (response.isSuccessful) {
                    Result.success(response.body?.string() ?: "{}")
                } else {
                    Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
            } catch (e: Exception) {
                Result.failure(e)
            }
        }

    suspend fun getActiveGoals(): Result<String> =
        withContext(Dispatchers.IO) {
            try {
                val request = Request.Builder()
                    .url("$baseUrl/api/companion/active")
                    .get()
                    .build()

                val response = client.newCall(request).execute()
                if (response.isSuccessful) {
                    Result.success(response.body?.string() ?: "{}")
                } else {
                    Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
            } catch (e: Exception) {
                Result.failure(e)
            }
        }

    // ─── Heartbeat ────────────────────────────────────────────────────────

    suspend fun heartbeat(goalId: String, review: String): Result<String> =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("goal_id", goalId)
                    put("review", review)
                }.toString()

                val request = Request.Builder()
                    .url("$baseUrl/api/companion/heartbeat")
                    .post(body.toRequestBody("application/json".toMediaType()))
                    .build()

                val response = client.newCall(request).execute()
                if (response.isSuccessful) {
                    Result.success(response.body?.string() ?: "{}")
                } else {
                    Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
            } catch (e: Exception) {
                Result.failure(e)
            }
        }

    // ─── Device State Reporting ───────────────────────────────────────────

    suspend fun reportDeviceState(stateJson: String) {
        withContext(Dispatchers.IO) {
            try {
                // Dispatch a device state event to the SPS kernel.
                val body = JSONObject().apply {
                    put("event_type", "device.state_reported")
                    put("payload", JSONObject(stateJson))
                }.toString()

                val request = Request.Builder()
                    .url("$baseUrl/api/events")
                    .post(body.toRequestBody("application/json".toMediaType()))
                    .build()

                client.newCall(request).execute().close()
            } catch (e: Exception) {
                android.util.Log.w("SPS", "Failed to report device state: ${e.message}")
            }
        }
    }

    // ─── Factory Control ──────────────────────────────────────────────────

    suspend fun runFactory(description: String, projectName: String?): Result<String> =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("description", description)
                    if (projectName != null) {
                        put("preferred_name", projectName)
                    }
                    put("output_dir", "/tmp/sps-android-factory")
                }.toString()

                val request = Request.Builder()
                    .url("$baseUrl/api/factory/run")
                    .post(body.toRequestBody("application/json".toMediaType()))
                    .build()

                val response = client.newCall(request).execute()
                if (response.isSuccessful) {
                    Result.success(response.body?.string() ?: "{}")
                } else {
                    Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
            } catch (e: Exception) {
                Result.failure(e)
            }
        }

    // ─── Provider Management ─────────────────────────────────────────────

    suspend fun listProviderTemplates(): Result<String> =
        withContext(Dispatchers.IO) {
            try {
                val request = Request.Builder()
                    .url("$baseUrl/api/providers/templates")
                    .get()
                    .build()

                val response = client.newCall(request).execute()
                if (response.isSuccessful) {
                    Result.success(response.body?.string() ?: "{}")
                } else {
                    Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
            } catch (e: Exception) {
                Result.failure(e)
            }
        }

    suspend fun registerProvider(
        kind: String,
        apiUrl: String,
        apiKey: String?,
        modelName: String
    ): Result<String> = withContext(Dispatchers.IO) {
        try {
            val body = JSONObject().apply {
                put("kind", kind)
                put("api_url", apiUrl)
                if (apiKey != null) put("api_key", apiKey)
                put("model_name", modelName)
            }.toString()

            val request = Request.Builder()
                .url("$baseUrl/api/providers")
                .post(body.toRequestBody("application/json".toMediaType()))
                .build()

            val response = client.newCall(request).execute()
            if (response.isSuccessful) {
                Result.success(response.body?.string() ?: "{}")
            } else {
                Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
            }
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    // ─── Configuration ────────────────────────────────────────────────────

    fun setServerUrl(url: String) {
        prefs.edit().putString("sps_server_url", url).apply()
    }

    fun getServerUrl(): String = baseUrl
}
