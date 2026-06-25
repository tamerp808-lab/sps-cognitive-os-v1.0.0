//! Storage abstraction.
//!
//! The kernel never talks to SQLite (or any concrete backend) directly.
//! All persistence goes through [`port::StoragePort`]. This allows:
//!
//! - Swapping backends without touching kernel code.
//! - Using an in-memory backend for fast tests.
//! - Future backends (Postgres, raw file log, S3-compatible) without
//!   API churn.

pub mod port;
