//! SPS integration tests — end-to-end cognitive pipeline.
//!
//! These tests exercise the full pipeline:
//! command → goal → plan → task → effect → reflection → learning → memory.
//!
//! They use ONLY in-memory storage (no disk I/O) so they're fast and
//! deterministic.

// Just a marker lib — the actual tests are in tests/.
