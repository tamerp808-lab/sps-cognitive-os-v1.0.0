// Phase 15: Advanced Intelligence Architecture
//
// This file documents the 5 pillars of SPS Advanced Intelligence,
// building on the completed Phase 1-14 foundation.
//
// Each pillar is a future implementation track that extends SPS beyond
// its current capabilities.

## Phase 15: Advanced Intelligence — 5 Pillars

### Pillar 1: Multi-Modal Perception
**Status:** Architecture defined, implementation pending
**Depends on:** Phase 13 (Device Agent) + Phase 14 (Companion UI)

SPS can perceive the world through multiple modalities:
- **Text** (keyboard input, OCR from screen)
- **Voice** (STT via Coqui/Whisper — Phase 13)
- **Image** (camera capture + VLM analysis)
- **Screen** (Accessibility Service content reading — Phase 13)
- **Context** (location, time, battery, network state)

Implementation: `sps-perception` crate that unifies all input modalities
into a single `PerceptionEvent` dispatched to the kernel.

### Pillar 2: Multi-Modal Expression
**Status:** Architecture defined, implementation pending
**Depends on:** Phase 14 (Companion UI) + Phase 13 (Device Agent)

SPS can express itself through:
- **Text** (chat responses)
- **Voice** (TTS via Coqui/ElevenLabs — Phase 13/14)
- **3D Animation** (Unity blend shapes — Phase 14)
- **Device Actions** (Accessibility Service clicks/swipes — Phase 13)
- **Code Generation** (Factory — Phase 11)

Implementation: `sps-expression` crate that routes output decisions
to the appropriate expression channel.

### Pillar 3: Long-Term Memory Consolidation
**Status:** Architecture defined, implementation pending
**Depends on:** Phase 3 (Memory) + Phase 9 (Reflection)

SPS consolidates episodic memories into semantic knowledge:
- Episodic → Semantic: "I built 5 REST APIs → REST API pattern"
- Semantic → Procedural: "REST API pattern → code generation template"
- Procedural → Automatic: "Template → autonomous factory run"

Implementation: `MemoryConsolidator` that runs as a background task,
promoting memories through the hierarchy based on access count +
success correlation.

### Pillar 4: Predictive Planning
**Status:** Architecture defined, implementation pending
**Depends on:** Phase 5 (Reasoning) + Phase 7 (Planner) + Phase 10 (Improvement)

SPS predicts outcomes before executing:
- Given a goal + context, predict success probability
- Given multiple plans, rank by predicted outcome
- Given past failures, avoid similar approaches

Implementation: `PredictivePlanner` that uses the ImprovementState
(historical performance data) to score plans before execution.

### Pillar 5: Autonomous Self-Modification
**Status:** Architecture defined, implementation pending
**Depends on:** Phase 10 (Improvement) + Phase 11 (Factory) + Phase 12C (Self-Improvement Loop)

SPS can modify its own behavior:
- Adjust retry policies (Phase 12C — implemented)
- Optimize LLM prompts based on success/failure patterns
- Generate new factory templates from successful runs
- Propose architectural changes to itself

Implementation: `SelfModifier` that uses the Factory to generate
proposed code changes, tests them in a sandbox, and applies them
via the Improvement governance lifecycle (Proposed → Approved → Applied).

---

## Integration Points

All 5 pillars connect through the existing SPS event-sourced architecture:

```
Perception (Pillar 1)
    ↓
Reasoning (Phase 5) + Predictive Planning (Pillar 4)
    ↓
Goal → Plan → Execute (Phases 6-8)
    ↓
Factory (Phase 11) + Expression (Pillar 2)
    ↓
Reflection (Phase 9) → Memory Consolidation (Pillar 3)
    ↓
Self-Improvement Loop (Phase 12C) → Self-Modification (Pillar 5)
    ↓
   ┌──────────────────────────────────────────┐
   │  Continuous Feedback Loop (hash chain)   │
   └──────────────────────────────────────────┘
```

## Technology Stack (from roadmap)

| Layer | Technology | Phase |
|-------|-----------|-------|
| Core Kernel | Rust | 1-12 (done) |
| Android Agent | Kotlin | 13 (scaffold) |
| 3D Companion | Unity (C#) | 14 (scaffold) |
| STT | Coqui / Whisper | 13/15 |
| TTS | Coqui / ElevenLabs | 14/15 |
| Real-time | WebRTC | 14/15 |
| Local DB | Room (Android) | 13 |
| Encrypted Storage | Rust + SQLCipher | 1 (done) |
