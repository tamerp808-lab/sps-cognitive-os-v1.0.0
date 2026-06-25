# ADR-0001: SPS Kernel Foundation Decisions

**Status:** Accepted
**Date:** 2026-06-21
**Phase:** Phase 0 — Kernel Core

## Context

SPS is being re-architected as a Cognitive Operating System. Before any
code was written, eight foundational decisions were made by the owner
during architecture review. This ADR records them so future contributors
understand why the kernel looks the way it does.

## Decisions

### D1. Kernel language: Rust

The kernel core (Event Store, Reducer Pipeline, Canonical State, Replay
Engine, Hash Chain, Effect Manager) is written in **Rust**. TypeScript
is used only for surfaces (CLI, Desktop, Web) and Adapters.

**Rationale.** The kernel's defining property is deterministic replay.
Rust's ownership model, lack of GC pauses, and `const`-correctness make
it easier to enforce purity on reducers. The `napi-rs` boundary lets
TypeScript call the kernel without sacrificing performance.

**Consequence.** Two build systems (Cargo + pnpm). Mitigated by the
monorepo layout (`kernel/` + `packages/`).

### D2. Event identifier: Logical Tick (`u64`), not ULID

Every event is identified by a monotonically increasing `tick: u64`
assigned by the `LogicalClock`. ULID/UUID are not used as primary
identifiers.

**Rationale.** Deterministic replay requires a total order. A `u64` tick
is the simplest possible monotonic identifier. ULIDs embed wall time,
which would couple replay order to wall time — exactly what we want to
avoid.

**Consequence.** `correlation_id` (UUID v7) is kept for tracing but
is **display metadata only** — it does not enter the hash or reducers.

### D3. `wall_time` is display-only

`Event::wall_time` is recorded (caller-supplied) but is **excluded**
from:
- The hash input.
- Reducer logic.
- Replay ordering.

**Rationale.** Wall time is non-deterministic across runs. Including it
in any of the above would break replay.

**Implementation.** The hash input is exactly:
`SHA-256(prev_hash || tick_be || event_type_utf8 || payload_canonical_json)`.
This is enforced by `Event::canonical_hash_input` and verified by
`ReplayVerifier::verify_chain`. Tests
`hash_excludes_wall_time_and_correlation` and
`wall_time_does_not_affect_hash` lock this invariant.

### D4. Storage abstraction: `StoragePort`

The kernel core depends only on the `StoragePort` trait. No concrete
backend (SQLite, Postgres, etc.) is imported in `sps-core`.

**Rationale.** Backend swap without kernel rewrites. Tests use
`InMemoryStorage`; production uses `SqliteStorage`. Future backends
(Postgres, file log, S3) can be added without touching kernel code.

**Current backends.**
- `sps-storage-memory` — `BTreeMap`-backed, for tests.
- `sps-storage-sqlite` — `rusqlite` with `bundled` feature, default
  for production.

**Transactions.** The `StorageTx` trait was intentionally **removed**
from Phase 0. Lifetime constraints on `&self`-borrowing transactions
made the trait awkward, and no Phase 0 caller needed multi-write
atomicity. Transactions will be re-introduced in a later phase when
the snapshot+truncate-replaced workflow needs them.

### D5. Phased execution order

The owner mandates strict phase ordering. No phase may begin until the
previous phase passes all tests and ships.

```
Phase 0:  Kernel Core              ← THIS PHASE (complete)
Phase 1:  Effect System
Phase 2:  Canonical State (full)
Phase 3:  Memory
Phase 4:  World Model
Phase 5:  Reasoning
Phase 6:  Goal System
Phase 7:  Planner
Phase 8:  Execution
Phase 9:  Reflection
Phase 10: Self-Improvement
Phase 11: Software Factory
Phase 12: Autonomy
```

Agent Runtime is **deferred** to after Phase 8. The kernel must be
stable before agents are layered on.

### D6. UI stack: Next.js + React + shadcn/ui + Tauri + ink

- Web Host: Next.js 16 + React 19 + shadcn/ui.
- Desktop Host: Tauri.
- CLI Host: `clap` (Rust) for parsing/routing; `ink` (TypeScript) for
  interactive rendering. CLI core logic lives in Rust so it can read
  canonical state directly without IPC.

### D7. Monorepo layout

```
sps/
├── kernel/                # Rust workspace
│   └── crates/
│       ├── sps-core/      # Event Store, Reducers, Replay, StoragePort
│       ├── sps-storage-memory/
│       └── sps-storage-sqlite/
└── packages/              # pnpm workspace (Phase 1+)
    ├── @sps/kernel/       # napi-rs bindings
    ├── @sps/adapters/
    ├── @sps/ui/
    ├── @sps/cli/
    ├── @sps/web/
    └── @sps/desktop/
```

### D8. No encryption in Phase 0

At-rest encryption is deferred. Phase 0 ships without `SQLCipher` or
app-level payload encryption. Encryption will be added in a later phase
once the kernel mechanics are stable.

### D9. In-process napi-rs binding (no sidecar)

The TypeScript surface loads `@sps/kernel` (napi-rs) **in-process**. No
sidecar process, no JSON-RPC over stdio. This trades crash isolation
for simplicity and speed — appropriate for early phases. A sidecar mode
can be added later as a config option without redesign.

### D10. Kernel-First Migration (strangler fig)

SPS is **not** a greenfield project. A reference architecture exists
elsewhere. Migration strategy is strangler fig: existing modules are
wrapped by Adapters that translate their APIs into Kernel commands.
The kernel boots clean; legacy modules are ported one at a time, each
adapter running in parallel with its legacy counterpart until the
adapter is proven stable, then the legacy module is removed.

In Phase 0 the adapter stubs are not yet created — they will appear in
Phase 1 when the Effect Manager makes the kernel actually useful to
consumers.

## Phase 0 Definition of Done — Status

| Item | Status |
|---|---|
| `sps-core` crate with Event, EventHash, Tick, RawEvent | ✅ |
| Hash chain (SHA-256, canonical JSON, key sorting) | ✅ |
| LogicalClock (monotonic u64, resumable) | ✅ |
| StoragePort trait (backend-agnostic) | ✅ |
| InMemoryStorage backend | ✅ |
| SqliteStorage backend (default, WAL, bundled) | ✅ |
| ReducerRegistry + ReducerPipeline | ✅ |
| KernelMetaReducer (built-in) | ✅ |
| CanonicalState skeleton + extension slots | ✅ |
| EventStore facade (append with tick + hash + race handling) | ✅ |
| SnapshotManager + Snapshot (content-addressed, verifiable) | ✅ |
| ReplayEngine (from-genesis, from-snapshot, from-tick) | ✅ |
| ReplayVerifier (hash chain + monotonic tick + tamper detection) | ✅ |
| SpsKernel facade (boot, dispatch, query, snapshot, verify, replay) | ✅ |
| 17 unit tests passing | ✅ |
| 19 integration tests passing (InMemory + SQLite + cross-backend) | ✅ |
| Deterministic replay verified across runs and backends | ✅ |
| wall_time exclusion verified | ✅ |
| Payload key-order invariance verified | ✅ |
| Hash chain tamper detection verified | ✅ |
| ADR-0001 documented | ✅ |

## Test Summary

```
running 17 tests (sps-core unit)
test result: ok. 17 passed; 0 failed

running 19 tests (deterministic_replay integration)
test result: ok. 19 passed; 0 failed

Total: 36 tests, 0 failures
```

## Open Items Carried to Phase 1

- `sps-ffi` crate (napi-rs bindings) — needed when TypeScript surfaces
  come online.
- `sps-cli-core` crate (clap-based CLI) — needed for `sps verify`,
  `sps replay`, `sps snapshot` commands.
- Effect Manager — Phase 1's primary deliverable.
- Provider Runtime — Phase 1's secondary deliverable.
- Adapter stubs — appear when the kernel can do real work.
