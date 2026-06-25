//! SQLite-backed `StoragePort` implementation.
//!
//! Default backend for SPS. Stores events, snapshots, and KV metadata
//! in a single SQLite database file.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE events (
//!     tick          INTEGER PRIMARY KEY,
//!     prev_hash     BLOB NOT NULL,   -- 32 bytes
//!     hash          BLOB NOT NULL,   -- 32 bytes
//!     event_type    TEXT NOT NULL,
//!     payload       TEXT NOT NULL,   -- canonical JSON
//!     causation_tick INTEGER,
//!     correlation_id BLOB NOT NULL,  -- 16 bytes UUID
//!     actor_kind    TEXT NOT NULL,
//!     actor_id      TEXT NOT NULL,
//!     schema_version INTEGER NOT NULL,
//!     wall_time     INTEGER NOT NULL,
//!     event_json    TEXT NOT NULL    -- full serialized event (for
//!                                     -- backward compat / future migrations)
//! );
//!
//! CREATE TABLE snapshots (
//!     tick          INTEGER PRIMARY KEY,
//!     snapshot_json TEXT NOT NULL,
//!     state_hash    BLOB NOT NULL,
//!     wall_time     INTEGER NOT NULL
//! );
//!
//! CREATE TABLE kv (
//!     key   TEXT PRIMARY KEY,
//!     value BLOB NOT NULL
//! );
//! ```

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use sps_core::event::{Event, EventHash, Tick};
use sps_core::snapshot::Snapshot;
use sps_core::storage::port::StoragePort;
use sps_core::{CoreError, CoreResult};

/// SQLite-backed storage.
pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    /// Open or create a SQLite database at the given path. Runs schema
    /// migrations if needed.
    pub fn open(path: &Path) -> CoreResult<Self> {
        let conn = Connection::open(path).map_err(map_sqlite_err)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;").map_err(map_sqlite_err)?;
        conn.execute_batch("PRAGMA synchronous = FULL;").map_err(map_sqlite_err)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;").map_err(map_sqlite_err)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    /// Open an in-memory SQLite database (for tests).
    pub fn open_in_memory() -> CoreResult<Self> {
        let conn = Connection::open_in_memory().map_err(map_sqlite_err)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    /// Wrap in `Arc<dyn StoragePort>`.
    pub fn into_arc(self) -> Arc<dyn StoragePort> {
        Arc::new(self)
    }

    fn run_migrations(&self) -> CoreResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                tick            INTEGER PRIMARY KEY,
                prev_hash       BLOB NOT NULL,
                hash            BLOB NOT NULL,
                event_type      TEXT NOT NULL,
                payload         TEXT NOT NULL,
                causation_tick  INTEGER,
                correlation_id  BLOB NOT NULL,
                actor_kind      TEXT NOT NULL,
                actor_id        TEXT NOT NULL,
                schema_version  INTEGER NOT NULL,
                wall_time       INTEGER NOT NULL,
                event_json      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                tick          INTEGER PRIMARY KEY,
                snapshot_json TEXT NOT NULL,
                state_hash    BLOB NOT NULL,
                wall_time     INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kv (
                key   TEXT PRIMARY KEY,
                value BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .map_err(map_sqlite_err)?;
        Ok(())
    }
}

fn map_sqlite_err(e: rusqlite::Error) -> CoreError {
    CoreError::Storage(anyhow::anyhow!("sqlite: {}", e))
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let event_json: String = row.get("event_json")?;
    let event: Event = serde_json::from_str(&event_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;
    Ok(event)
}

impl StoragePort for SqliteStorage {
    fn append_event(&self, event: &Event) -> CoreResult<()> {
        let conn = self.conn.lock();
        let event_json = serde_json::to_string(event)?;
        let prev_hash = event.prev_hash.as_bytes();
        let hash = event.hash.as_bytes();
        let correlation_id = event.correlation_id.0.as_bytes();
        conn.execute(
            "INSERT INTO events
                (tick, prev_hash, hash, event_type, payload, causation_tick,
                 correlation_id, actor_kind, actor_id, schema_version,
                 wall_time, event_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                event.tick as i64,
                prev_hash,
                hash,
                event.event_type.as_str(),
                event.payload.to_string(),
                event.causation_tick.map(|t| t as i64),
                correlation_id,
                serde_json::to_string(&event.actor.kind)
                    .map_err(|e| CoreError::Storage(anyhow::anyhow!("actor_kind ser: {}", e)))?,
                event.actor.id.as_str(),
                event.schema_version,
                event.wall_time as i64,
                event_json,
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(ref fail, _)
                if fail.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                CoreError::NonMonotonicTick {
                    prev: 0, // we don't know the exact previous tick here
                    curr: event.tick,
                }
            }
            other => map_sqlite_err(other),
        })?;
        Ok(())
    }

    fn read_events_from(&self, from_tick: Tick, limit: usize) -> CoreResult<Vec<Event>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT event_json FROM events WHERE tick >= ?1 ORDER BY tick ASC LIMIT ?2",
            )
            .map_err(map_sqlite_err)?;
        let rows = stmt
            .query_map(params![from_tick as i64, limit as i64], row_to_event)
            .map_err(map_sqlite_err)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(map_sqlite_err)?);
        }
        Ok(out)
    }

    fn read_event_by_tick(&self, tick: Tick) -> CoreResult<Option<Event>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT event_json FROM events WHERE tick = ?1")
            .map_err(map_sqlite_err)?;
        let event = stmt
            .query_row(params![tick as i64], row_to_event)
            .optional()
            .map_err(map_sqlite_err)?;
        Ok(event)
    }

    fn last_tick(&self) -> CoreResult<Tick> {
        let conn = self.conn.lock();
        let tick: Option<i64> = conn
            .query_row("SELECT MAX(tick) FROM events", [], |row| row.get(0))
            .map_err(map_sqlite_err)?;
        Ok(tick.unwrap_or(0) as Tick)
    }

    fn last_hash(&self) -> CoreResult<EventHash> {
        let conn = self.conn.lock();
        let hash: Option<Vec<u8>> = conn
            .query_row(
                "SELECT hash FROM events ORDER BY tick DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite_err)?;
        match hash {
            Some(bytes) => {
                let mut arr = [0u8; 32];
                if bytes.len() != 32 {
                    return Err(CoreError::Internal(anyhow::anyhow!(
                        "stored hash has wrong length: {}",
                        bytes.len()
                    )));
                }
                arr.copy_from_slice(&bytes);
                Ok(EventHash::from_bytes(arr))
            }
            None => Ok(EventHash::GENESIS),
        }
    }

    fn count_events(&self) -> CoreResult<u64> {
        let conn = self.conn.lock();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .map_err(map_sqlite_err)?;
        Ok(count as u64)
    }

    fn write_snapshot(&self, snapshot: &Snapshot) -> CoreResult<()> {
        let conn = self.conn.lock();
        let json = serde_json::to_string(snapshot)?;
        conn.execute(
            "INSERT OR REPLACE INTO snapshots (tick, snapshot_json, state_hash, wall_time)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                snapshot.tick as i64,
                json,
                snapshot.state_hash,
                snapshot.wall_time as i64,
            ],
        )
        .map_err(map_sqlite_err)?;
        Ok(())
    }

    fn read_latest_snapshot(&self) -> CoreResult<Option<Snapshot>> {
        let conn = self.conn.lock();
        let json: Option<String> = conn
            .query_row(
                "SELECT snapshot_json FROM snapshots ORDER BY tick DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite_err)?;
        match json {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => Ok(None),
        }
    }

    fn write_kv(&self, key: &str, value: &[u8]) -> CoreResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO kv (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(map_sqlite_err)?;
        Ok(())
    }

    fn read_kv(&self, key: &str) -> CoreResult<Option<Vec<u8>>> {
        let conn = self.conn.lock();
        let value: Option<Vec<u8>> = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(map_sqlite_err)?;
        Ok(value)
    }

    fn backend_name(&self) -> &'static str {
        "sqlite"
    }
}
