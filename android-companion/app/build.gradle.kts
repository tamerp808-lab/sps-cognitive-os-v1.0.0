// SPS Android Companion — Device Agent & Control
// Phase 13: Android Accessibility Service for device control
//
// This app connects to the SPS kernel (running on the same device or a
// server) and provides:
// 1. Goal activation/deactivation via HTTP
// 2. Device control via Accessibility Service
// 3. Heartbeat reporting to SPS kernel

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.compose.compiler)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.sps.companion"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.sps.companion"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "1.0.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }
}

dependencies {
    // ─── AndroidX Core ───────────────────────────────────────────
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.service)

    // ─── Jetpack Compose ─────────────────────────────────────────
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material.icons.extended)
    debugImplementation(libs.androidx.compose.ui.tooling)

    // ─── Navigation ──────────────────────────────────────────────
    implementation(libs.androidx.navigation.compose)

    // ─── Lifecycle + ViewModel (Compose) ─────────────────────────
    implementation(libs.androidx.lifecycle.viewmodel.compose)

    // ─── DataStore (preferences) ─────────────────────────────────
    implementation(libs.androidx.datastore.preferences)

    // ─── WorkManager ─────────────────────────────────────────────
    implementation(libs.androidx.work.runtime.ktx)

    // ─── Kotlinx Serialization ───────────────────────────────────
    implementation(libs.kotlinx.serialization.json)

    // ─── Kotlinx Coroutines ──────────────────────────────────────
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.coroutines.core)

    // ─── Networking — OkHttp (HTTP + WebSocket for SPS kernel) ──
    implementation(libs.okhttp)
    implementation(libs.okhttp.logging)
    implementation(libs.okhttp.sse)

    // ─── TensorFlow Lite (on-device wake word detection) ────────
    implementation(libs.tflite)
    implementation(libs.tflite.support)
    implementation(libs.tflite.task.audio)

    // ─── Media3 (audio playback for TTS) ────────────────────────
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.ui)

    // ─── Image loading ───────────────────────────────────────────
    implementation(libs.coil.compose)

    // ─── Accompanist (permissions) ───────────────────────────────
    implementation(libs.accompanist.permissions)
}
