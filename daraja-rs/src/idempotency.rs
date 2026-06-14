use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

const TTL_SECS: i64 = 86400;

/// Outcome of a prior request for a given idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Success(Vec<u8>),
    Failed,
}

/// Local SQLite cache for idempotency keys.
pub struct IdempotencyStore {
    conn: Connection,
}

impl IdempotencyStore {
    /// Open (or create) the idempotency database at `db_path`.
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS idempotency (
                key TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                response BLOB,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    /// Record that a request with this key is in flight.
    pub fn save_pending(&self, key: &str) {
        let now = unix_now();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO idempotency (key, status, response, created_at)
                 VALUES (?1, 'pending', NULL, ?2)",
                params![key, now],
            )
            .expect("save_pending");
    }

    /// Record a successful response for this key.
    pub fn save_success(&self, key: &str, response: &[u8]) {
        let now = unix_now();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO idempotency (key, status, response, created_at)
                 VALUES (?1, 'success', ?2, ?3)",
                params![key, response, now],
            )
            .expect("save_success");
    }

    /// Record that the request failed.
    pub fn save_failed(&self, key: &str) {
        let now = unix_now();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO idempotency (key, status, response, created_at)
                 VALUES (?1, 'failed', NULL, ?2)",
                params![key, now],
            )
            .expect("save_failed");
    }

    /// Look up the cached status for a key.
    pub fn get_status(&self, key: &str) -> Option<Status> {
        let mut stmt = self
            .conn
            .prepare("SELECT status, response FROM idempotency WHERE key = ?1")
            .ok()?;
        let (status, response): (String, Option<Vec<u8>>) = stmt
            .query_row(params![key], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?;
        match status.as_str() {
            "pending" => Some(Status::Pending),
            "success" => Some(Status::Success(response.unwrap_or_default())),
            "failed" => Some(Status::Failed),
            _ => None,
        }
    }

    /// Delete entries older than 24 hours.
    pub fn cleanup_expired(&self) {
        let cutoff = unix_now() - TTL_SECS;
        self.conn
            .execute(
                "DELETE FROM idempotency WHERE created_at < ?1",
                params![cutoff],
            )
            .expect("cleanup_expired");
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX epoch")
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, IdempotencyStore) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("idempotency.db");
        let store = IdempotencyStore::new(path.to_str().unwrap()).expect("open store");
        (dir, store)
    }

    fn insert_with_timestamp(store: &IdempotencyStore, key: &str, created_at: i64) {
        store
            .conn
            .execute(
                "INSERT INTO idempotency (key, status, response, created_at)
                 VALUES (?1, 'success', ?2, ?3)",
                params![key, b"old".as_slice(), created_at],
            )
            .expect("insert row");
    }

    #[test]
    fn save_and_retrieve_pending() {
        let (_dir, store) = temp_store();
        let key = "pending-key";

        store.save_pending(key);
        assert_eq!(store.get_status(key), Some(Status::Pending));
    }

    #[test]
    fn save_and_retrieve_success() {
        let (_dir, store) = temp_store();
        let key = "success-key";
        let response = br#"{"ResponseCode":"0"}"#;

        store.save_success(key, response);
        assert_eq!(
            store.get_status(key),
            Some(Status::Success(response.to_vec()))
        );
    }

    #[test]
    fn save_and_retrieve_failed() {
        let (_dir, store) = temp_store();
        let key = "failed-key";

        store.save_failed(key);
        assert_eq!(store.get_status(key), Some(Status::Failed));
    }

    #[test]
    fn missing_key_returns_none() {
        let (_dir, store) = temp_store();
        assert_eq!(store.get_status("missing-key"), None);
    }

    #[test]
    fn cleanup_expired_removes_old_entries() {
        let (_dir, store) = temp_store();
        let expired_key = "expired-key";
        let fresh_key = "fresh-key";
        let old_ts = unix_now() - TTL_SECS - 3600;

        insert_with_timestamp(&store, expired_key, old_ts);
        store.save_pending(fresh_key);

        store.cleanup_expired();
        assert_eq!(store.get_status(expired_key), None);
        assert_eq!(store.get_status(fresh_key), Some(Status::Pending));
    }
}