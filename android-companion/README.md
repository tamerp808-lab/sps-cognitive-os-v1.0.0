# SPS Companion — Android Native Companion (Phase 10)

> The capstone component that turns SPS from "a platform in the browser" into **a true personal cognitive AI companion that lives on your phone**.

## What this is

A native Android app (Kotlin + Jetpack Compose + Material 3) that provides:

| Feature | Implementation |
|---------|----------------|
| **Always-on Wake Word** | `WakeWordDetector` using TFLite AudioClassifier on 16kHz PCM, energy-based fallback |
| **Persistent Background Service** | `SpsForegroundService` (START_STICKY, foreground notification, wake lock) |
| **Floating Assistant** | `OverlayBubbleService` (drag-to-move bubble, tap-to-talk, edge snapping) |
| **Screen Reading & Automation** | `SpsAccessibilityService` (read screen, tap elements, swipe, back/home) |
| **Notification Reader** | `SpsNotificationListener` (read, summarize, dismiss) |
| **Quick Settings Tile** | `SpsTileService` (one-tap voice from the shade) |
| **Home Screen Widget** | `SpsWidgetProvider` (tap-to-talk widget) |
| **Boot Receiver** | `BootReceiver` (auto-start SPS on device boot) |
| **Custom URL Scheme** | `sps://...` (deep-link from any app) |
| **Share Target** | "Share to SPS" from any app's share sheet |
| **App Shortcuts** | Long-press icon → Talk / Goals / Memory |
| **Native Voice** | Android `SpeechRecognizer` (STT) + `TextToSpeech` (TTS) — no cloud |
| **Material 3 UI** | Home / Chat / Voice / Goals / Memory / Settings / Permissions |
| **Local-First** | Talks to `127.0.0.1:7780` (SPS Rust kernel via Termux) — no internet needed |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     SPS Companion (APK)                      │
│                                                              │
│  ┌──────────┐  ┌─────────────┐  ┌────────────────────────┐ │
│  │ MainActivity (Compose UI) │  │ SpsForegroundService   │ │
│  │  - Home                  │  │  - WakeWordDetector    │ │
│  │  - Chat                  │  │  - Connection monitor  │ │
│  │  - Voice                 │  │  - Persistent notif    │ │
│  │  - Goals                 │  └────────────────────────┘ │
│  │  - Memory                │                              │
│  │  - Settings              │  ┌────────────────────────┐ │
│  │  - Permissions           │  │ OverlayBubbleService   │ │
│  └──────────┘                │  │  (floating bubble)    │ │
│         │                     │ ─────────────────────── │ │
│         │                     │  SpsAccessibilitySvc    │ │
│         ▼                     │  (screen reading)      │ │
│  ┌─────────────────────────┐ │ ─────────────────────── │ │
│  │  SpsClient (OkHttp)     │ │  SpsNotificationListener│ │
│  │  HTTP + SSE + WebSocket │ │  (notif reading)       │ │
│  └─────────────────────────┘ │ ─────────────────────── │ │
│         │                     │  SpsTileService        │ │
│         ▼                     │  SpsWidgetProvider     │ │
│  ┌─────────────────────────┐ │  BootReceiver          │ │
│  │  127.0.0.1:7780         │ └────────────────────────┘ │
│  │  SPS Rust Kernel        │                              │
│  │  (Termux or bundled)    │                              │
│  └─────────────────────────┘                              │
└─────────────────────────────────────────────────────────────┘
```

## The "Hey SPS" pipeline

```
1. WakeWordDetector (TFLite on 16kHz PCM)
   ↓ detects "Hey SPS"
2. SpsForegroundService.handleWakeWord()
   ↓ vibrates + updates notification
3. VoiceCommandHandler.handleWakeWord()
   ↓ voice.speak("Yes?")
4. VoiceManager.listenOnce()  (Android SpeechRecognizer)
   ↓ returns "open WhatsApp"
5. handleLocalCommand() or SpsClient.complete()
   ↓ dispatches to SPS kernel
6. SPS responds with text
   ↓
7. VoiceManager.speak(response)  (Android TextToSpeech)
   ↓
8. SpsClient.storeMemory(...)   (episodic memory)
```

## Build & Install

### Prerequisites

- Android Studio Ladybug (2024.2) or newer
- JDK 17
- Android SDK 35 (compileSdk), minSdk 26 (Android 8.0)
- (Optional) Physical Android device for full testing (emulators don't support wake word + mic well)

### Build from Android Studio

1. Open the `android-companion/` folder in Android Studio.
2. Wait for Gradle sync to complete.
3. Click **Run** (or **Build → Build APK**).

### Build from command line

```bash
cd android-companion

# Set up local.properties (point to your Android SDK)
echo "sdk.dir=/path/to/Android/Sdk" > local.properties

# Build the debug APK
./gradlew assembleDebug

# Install on a connected device
./gradlew installDebug

# Or install manually
adb install app/build/outputs/apk/debug/app-debug.apk
```

### First-run setup

1. Open SPS on your phone.
2. Go to **Settings → SPS Server** and set the URL to your SPS kernel.
   - On-device: install Termux, run `sps-server --listen 127.0.0.1:7780`
   - On a PC on the same Wi-Fi: `http://192.168.x.x:7780`
3. Tap **Reconnect** — should show "● Connected".
4. Go to **Permissions** and grant all required permissions.
5. Enable **Wake Word Detection** in Settings → Voice.
6. Say **"Hey SPS"** — the app vibrates and listens for your command.

### Optional: enable always-on capabilities

- **Floating Bubble**: Settings → Companion → Floating Bubble
- **Screen Reading**: Settings → Companion → Accessibility Service (system settings)
- **Notification Reader**: Settings → Companion → Notification Access (system settings)
- **Boot Persistence**: Settings → Companion → Start on Boot
- **Battery**: Settings → Companion → Battery Optimization → disable for SPS

## File structure

```
android-companion/
├── settings.gradle.kts          # Single-module Gradle settings
├── build.gradle.kts             # Project-level build
├── gradle.properties            # JVM + Compose flags
├── gradle/
│   ├── libs.versions.toml       # Version catalog (single source of truth)
│   └── wrapper/
│       └── gradle-wrapper.properties
├── app/
│   ├── build.gradle.kts         # App-level build (deps, SDK, signing)
│   ├── proguard-rules.pro       # Keep rules for serialization + TFLite
│   └── src/main/
│       ├── AndroidManifest.xml  # All permissions + service declarations
│       ├── java/com/sps/companion/
│       │   ├── SpsApplication.kt          # Composition root
│       │   ├── MainActivity.kt            # Compose NavHost
│       │   ├── data/
│       │   │   ├── SpsConfig.kt           # Persisted config
│       │   │   ├── SpsConfigManager.kt    # DataStore-backed
│       │   │   └── Models.kt              # DTOs (goals, memory, events)
│       │   ├── network/
│       │   │   ├── SpsClient.kt           # HTTP + SSE client
│       │   │   └── SpsConnectionState.kt
│       │   ├── voice/
│       │   │   ├── WakeWordDetector.kt    # TFLite + AudioRecord
│       │   │   └── VoiceManager.kt        # STT + TTS wrapper
│       │   ├── service/
│       │   │   ├── SpsForegroundService.kt   # Persistent background
│       │   │   ├── VoiceCommandHandler.kt    # Wake word → action pipeline
│       │   │   ├── SpsAccessibilityService.kt
│       │   │   ├── SpsNotificationListener.kt
│       │   │   ├── SpsTileService.kt         # Quick Settings tile
│       │   │   └── WakeWordService.kt        # (reserved)
│       │   ├── overlay/
│       │   │   └── OverlayBubbleService.kt   # Floating bubble
│       │   ├── receiver/
│       │   │   ├── BootReceiver.kt           # Boot auto-start
│       │   │   └── SpsWidgetProvider.kt      # Home screen widget
│       │   ├── ui/
│       │   │   ├── theme/                    # Material 3 colors + type
│       │   │   ├── components/
│       │   │   │   └── SpsBrainLogo.kt       # Animated brain logo
│       │   │   ├── screens/
│       │   │   │   ├── HomeScreen.kt
│       │   │   │   ├── ChatScreen.kt
│       │   │   │   ├── VoiceScreen.kt
│       │   │   │   ├── GoalsScreen.kt
│       │   │   │   ├── MemoryScreen.kt
│       │   │   │   ├── SettingsScreen.kt
│       │   │   │   └── PermissionsScreen.kt
│       │   │   └── viewmodel/                # AAC ViewModels
│       │   └── util/                         # (reserved)
│       └── res/
│           ├── values/                       # Strings, colors, themes
│           ├── values-night/
│           ├── drawable/                     # Vector icons (brain, mic, etc.)
│           ├── layout/widget_sps.xml         # Widget layout
│           └── xml/                          # Accessibility config, shortcuts, etc.
```

## Privacy

- **No internet permission is required for core functionality.** SPS talks to the local kernel via `127.0.0.1`.
- Audio is processed on-device by TFLite — never uploaded.
- Notifications are kept in an in-memory ring buffer (50 most recent), never persisted.
- Accessibility service does NOT log screen contents continuously — only acts on explicit user commands.
- All configuration is stored in DataStore (encrypted by Android's storage encryption).

## What's next

After installing this APK on a phone running the SPS Rust kernel (via Termux or bundled), the user can:

1. Say **"Hey SPS"** from any app → ask anything → SPS responds aloud.
2. Say **"Open WhatsApp"** → SPS launches the app.
3. Say **"Read my notifications"** → SPS reads the 5 most recent.
4. Tap the floating bubble → instant voice command.
5. Long-press the home icon → quick access to Talk / Goals / Memory.
6. Add the home screen widget → one-tap voice activation.
7. Pull down quick settings → tap "SPS Voice" → opens ready to listen.
8. Reboot the phone → SPS auto-starts.

This is what makes SPS a true **"Hey Siri / Hey Google / Hey Alexa"** competitor — but with an IDE, Memory, Agents, Goals, World Model, and Local AI all in the same system.
