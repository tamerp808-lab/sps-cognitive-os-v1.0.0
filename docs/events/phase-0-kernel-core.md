# SPS Phase 0 — Kernel Core Event Schema

**Phase:** 0 (Kernel Core)
**Status:** Stable
**Schema Version:** 1

## Event Types

Phase 0 defines exactly three system event types. No domain event types
are defined yet — those appear in later phases.

### `system.booted`

Emitted once at kernel boot, when a fresh store is created. Not emitted
on subsequent boots of an existing store.

```json
{
  "schema_version": 1
}
```

### `system.snapshot_taken`

Emitted when a snapshot is persisted. The snapshot itself is stored
separately (in the `snapshots` table); this event records that a
snapshot was taken, for audit purposes.

```json
{
  "snapshot_tick": 1234,
  "state_hash_hex": "abc123..."
}
```

### `system.replay_verified`

Emitted after a successful `ReplayVerifier::verify_chain` run, if the
kernel is configured to persist verification results.

```json
{
  "events_verified": 1234,
  "last_tick": 1234,
  "last_hash_hex": "abc123...",
  "elapsed_us": 5678
}
```

## Event Envelope

Every event has the following fields. Fields marked **[hash]** are
included in the SHA-256 hash input; fields marked **[display]** are not.

| Field | Type | Hash? | Notes |
|---|---|---|---|
| `tick` | `u64` | ✅ | Monotonic per stream; primary identifier |
| `prev_hash` | `[u8; 32]` | ✅ | Hash of previous event; `GENESIS` (zeros) for tick 1 |
| `hash` | `[u8; 32]` | — | Computed; not an input to itself |
| `event_type` | `SmolStr` | ✅ | Dotted string, e.g. `"system.booted"` |
| `payload` | `serde_json::Value` | ✅ (canonical) | Sorted keys, recursive |
| `causation_tick` | `Option<u64>` | ❌ | Display — for tracing |
| `correlation_id` | `Uuid` (v7) | ❌ | Display — links to originating command |
| `actor` | `Actor` | ❌ | Display — who emitted |
| `schema_version` | `u16` | ❌ | For future migrations |
| `wall_time` | `u64` (ms) | ❌ | Display only |

## Hash Input

```
hash = SHA-256(
    prev_hash         // 32 bytes raw
  || tick             // 8 bytes big-endian
  || event_type       // UTF-8 bytes, no length prefix
  || payload_canonical_json  // sorted keys, recursive
)
```

**Canonical JSON** means:
- Object keys sorted lexicographically at every level.
- No whitespace between tokens.
- UTF-8 encoded.

This guarantees that two events with the same logical payload produce
the same hash regardless of insertion order in a `Map`.

## Verification

The `ReplayVerifier::verify_chain` function checks, for every event in
the store:

1. `event.tick > previous.tick` (monotonic).
2. `event.recompute_hash() == event.hash` (no tampering).
3. `event.prev_hash == previous.hash` (chain continuity).

Any failure produces a `ReplayFailure` and aborts verification. The
kernel refuses to boot on verification failure.
