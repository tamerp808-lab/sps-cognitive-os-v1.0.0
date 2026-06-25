# SPS — Cognitive Operating System
# Release 1.0.0

## Version
1.0.0

## Release Date
2026-06-25

## Architecture

SPS is a local-first personal AI platform with:
- **Rust kernel** (28 crates): event-sourced, hash-chained, deterministic replay
- **Android companion** (36 Kotlin files): Accessibility, voice, overlay, widget
- **Unity companion** (6 C# scripts): 3D emotional avatar with STT/TTS pipeline

## Rust Kernel (28 crates)

### Core (sps-core)
- Event Store with SHA-256 hash chain
- Deterministic replay engine
- Snapshot manager (content-addressed, verified)
- P3D typed extensions (zero JSON overhead per dispatch)
- ErasedExtension trait + TypedExtensionRegistry
- EventSink trait + dispatch_trusted (P2 optimization)
- KernelMetaReducer always-on (Fix #16)

### Domain Crates
- sps-memory: 4 memory kinds + graph + decay + promotion
- sps-world: projects, files, agents, tools, relationships
- sps-reasoning: goal analyzer, task decomposer, conflict detector, risk analyzer
- sps-goals: Goal→Objective→Milestone→Task hierarchy
- sps-planner: plan templates + lifecycle
- sps-execution: code analyzer + project generator
- sps-reflection: success/failure analyzers + pattern extractor
- sps-improvement: performance analyzer + governance-gated proposals
- sps-factory: 8-stage workflow (RequirementAnalysis→DeploymentPrep)
  - Effect-based (Phase 11B): all actions via effect.intent
  - FactorySupervisor (Phase 11C): automated retry/rollback/abort
  - LLM-powered (Phase 11D): 3 stages LLM-driven, 5 deterministic
  - 13 built-in LLM providers + custom provider support
- sps-autonomy: governor + long-running goal runner
- sps-agents: 6 archetypes + AgentRuntime with EventSink
- sps-cognitive: CognitiveLoop + decision-driven behavior
  - PredictivePlanner, GoalForecaster, ScenarioSimulator (Monte Carlo)
  - DecisionScorer, OpportunityDetector, CounterfactualEngine
  - MemoryConsolidator, ForgettingPolicy, ImportanceScorer
  - EmotionalMemory, KnowledgeGraphExpander
  - SelfModificationGovernor (Propose→Simulate→Validate→Approve→Apply)
  - SelfModificationPipeline (Factory→Compile→Test→Governance→Deploy)
  - BackgroundScheduler (autonomous operation without user input)
  - PerceptionFuser (multi-modal: voice+screen+notifications+location+time)
  - PlatformAdapters (10 traits: camera, mic, accessibility, etc.)
  - CognitiveLoop (16 steps, decision-driven, all tools wired)

### Infrastructure
- sps-effects: EffectManager + 5 executors (fs, shell, git, search, factory)
- sps-providers-http: 13 LLM provider templates + custom providers
- sps-storage-sqlite: SQLite backend (WAL, bundled rusqlite)
- sps-storage-memory: In-memory backend (tests)
- sps-server: HTTP API + WebSocket + companion routes
- sps-cli: `sps` binary
- sps-ffi: napi-rs bindings for TypeScript

## Android Companion (36 Kotlin files)
- Jetpack Compose UI (7 screens: Home, Chat, Voice, Goals, Memory, Settings, Permissions)
- 6 Services: Foreground, Accessibility, WakeWord, Overlay, NotificationListener, Tile
- 2 BroadcastReceivers: Boot, Widget
- VoiceManager + WakeWordDetector (TFLite AudioClassifier)
- SpsKernelClient (HTTP + WebSocket to SPS kernel)
- DataStore configuration persistence
- Build: AGP 8.7.3, Kotlin 2.0.21, Compose BOM 2024.12.01, SDK 35

## Unity Companion (6 C# scripts)
- CompanionController: 8 emotions, blend shapes, lip sync, blinking
- EmotionManager: emotion state machine with Arabic+English detection
- TtsHandler: TTS via SPS kernel (Coqui/ElevenLabs)
- SttHandler: STT via microphone + SPS kernel (Coqui/Whisper)
- ConversationManager: STT→LLM→TTS pipeline with emotion tracking
- CompanionBootstrap: auto-configuration

## Test Results
- sps-core: 21 unit tests ✓
- sps-cognitive: 69 unit tests ✓ (including 5 decision-driven behavioral tests)
- sps-autonomy: 3 unit tests ✓
- sps-improvement: 5 unit tests ✓
- Integration tests: H0, H0.5, H1, H2, H3, H4, Checkpoint E, Phase 11A-D, Phase 12A-C ✓
- Android: BUILD SUCCESSFUL (APK 60MB, sha256 cff310ef)

## Audit Results
```
TODO:             0
FIXME:            0
stub:             0
placeholder:      0
unimplemented!(): 0
todo!():          0
unreachable!():   0
```

## Cognitive Pipeline (16 steps, decision-driven)
1. Memory Recall → query memories
2. Reasoning → intent analysis (Arabic + English)
3. Decision Scoring → DecisionScorer.rank() — best option chosen
4. Goal Assessment → GoalForecaster.forecast() — goal rejected if < 20%
5. Goal Forecasting → (merged with Goal Assessment)
6. Predictive Planning → PredictivePlanner.score() — plan created if score > 0
7. Scenario Simulation → ScenarioSimulator.monte_carlo(50) — plan regenerated if < 40%
8. Execution → aborted if simulation < 20%
9. Factory → triggered for code generation requests
10. Reflection → success analysis
11. Counterfactual → CounterfactualEngine.analyze() — lesson extracted
12. Memory Store → episodic memory created
13. Memory Consolidation → MemoryConsolidator.evaluate_batch()
14. Forgetting Check → ForgettingPolicy.evaluate() — Forget dispatches memory.removed
15. Self-Improvement → improvement proposed if failures detected
16. Output → Text/Voice/Action/Code

## Build Instructions

### Rust Kernel
```bash
cd kernel
cargo build --workspace
cargo test --workspace
```

### Android Companion
```bash
cd android-companion
./gradlew assembleDebug
# Output: app/build/outputs/apk/debug/app-debug.apk
```

### Unity Companion
Open `sps-companion-unity/` in Unity 2022.3+ and build.
