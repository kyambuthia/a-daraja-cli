use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

const TTL_SECS: i64 = 86_400;

/// Outcome of a prior request for a scoped idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Success(Vec<u8>),
    Failed,
    Unknown,
}

/// Result of atomically reserving an idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquireResult {
    Acquired,
    Pending,
    Success(Vec<u8>),
    Failed,
    Unknown,
    FingerprintMismatch,
}

/// Local SQLite cache for idempotency keys.
pub struct IdempotencyStore {
    conn: Connection,
}

impl IdempotencyStore {
    /// Open (or create) the idempotency database at `db_path`.
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS idempotency (
                key TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
                response BLOB,
                created_at INTEGER NOT NULL
             );",
        )?;

        // Migrate databases created before request fingerprints were stored.
        let has_fingerprint = {
            let mut stmt = conn.prepare("PRAGMA table_info(idempotency)")?;
            let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for column in columns {
                if column? == "fingerprint" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_fingerprint {
            conn.execute(
                "ALTER TABLE idempotency ADD COLUMN fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }

        Ok(Self { conn })
    }

    /// Atomically reserve a scoped key for a request fingerprint.
    pub fn acquire(
        &mut self,
        key: &str,
        fingerprint: &str,
    ) -> Result<AcquireResult, rusqlite::Error> {
        let now = unix_now();
        let cutoff = now - TTL_SECS;
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM idempotency WHERE created_at < ?1",
            params![cutoff],
        )?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO idempotency (key, fingerprint, status, response, created_at)
             VALUES (?1, ?2, 'pending', NULL, ?3)",
            params![key, fingerprint, now],
        )?;

        let result = if inserted == 1 {
            AcquireResult::Acquired
        } else {
            let (stored_fingerprint, status, response): (String, String, Option<Vec<u8>>) = tx
                .query_row(
                    "SELECT fingerprint, status, response FROM idempotency WHERE key = ?1",
                    params![key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            if stored_fingerprint != fingerprint {
                AcquireResult::FingerprintMismatch
            } else {
                match status.as_str() {
                    "pending" => AcquireResult::Pending,
                    "success" => AcquireResult::Success(response.unwrap_or_default()),
                    "failed" => AcquireResult::Failed,
                    "unknown" => AcquireResult::Unknown,
                    _ => AcquireResult::Unknown,
                }
            }
        };
        tx.commit()?;
        Ok(result)
    }

    /// Record a successful, parsed response for this key.
    pub fn save_success(&self, key: &str, response: &[u8]) -> Result<(), rusqlite::Error> {
        self.update(key, "success", Some(response))
    }

    /// Record a definitive HTTP failure for this key.
    pub fn save_failed(&self, key: &str) -> Result<(), rusqlite::Error> {
        self.update(key, "failed", None)
    }

    /// Record that the request may have reached Daraja but no result was obtained.
    pub fn save_unknown(&self, key: &str) -> Result<(), rusqlite::Error> {
        self.update(key, "unknown", None)
    }

    fn update(
        &self,
        key: &str,
        status: &str,
        response: Option<&[u8]>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE idempotency
             SET status = ?2, response = ?3, created_at = ?4
             WHERE key = ?1",
            params![key, status, response, unix_now()],
        )?;
        Ok(())
    }

    /// Look up the cached status for a key.
    pub fn get_status(&self, key: &str) -> Result<Option<Status>, rusqlite::Error> {
        let row: Option<(String, Option<Vec<u8>>)> = self
            .conn
            .query_row(
                "SELECT status, response FROM idempotency WHERE key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(status, response)| match status.as_str() {
            "pending" => Status::Pending,
            "success" => Status::Success(response.unwrap_or_default()),
            "failed" => Status::Failed,
            "unknown" => Status::Unknown,
            _ => Status::Unknown,
        }))
    }

    /// Delete entries older than 24 hours.
    pub fn cleanup_expired(&self) -> Result<(), rusqlite::Error> {
        let cutoff = unix_now() - TTL_SECS;
        self.conn.execute(
            "DELETE FROM idempotency WHERE created_at < ?1",
            params![cutoff],
        )?;
        Ok(())
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
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

    #[test]
    fn acquire_is_atomic_and_detects_existing_pending_request() {
        let (_dir, mut store) = temp_store();
        assert_eq!(
            store.acquire("scope:key", "fingerprint").unwrap(),
            AcquireResult::Acquired
        );
        assert_eq!(
            store.acquire("scope:key", "fingerprint").unwrap(),
            AcquireResult::Pending
        );
    }

    #[test]
    fn acquire_rejects_same_key_for_different_payload() {
        let (_dir, mut store) = temp_store();
        store.acquire("scope:key", "first").unwrap();
        assert_eq!(
            store.acquire("scope:key", "second").unwrap(),
            AcquireResult::FingerprintMismatch
        );
    }

    #[test]
    fn success_response_is_reused() {
        let (_dir, mut store) = temp_store();
        let response = br#"{"ResponseCode":"0"}"#;
        store.acquire("scope:key", "fingerprint").unwrap();
        store.save_success("scope:key", response).unwrap();
        assert_eq!(
            store.acquire("scope:key", "fingerprint").unwrap(),
            AcquireResult::Success(response.to_vec())
        );
    }

    #[test]
    fn unknown_outcome_is_not_retried() {
        let (_dir, mut store) = temp_store();
        store.acquire("scope:key", "fingerprint").unwrap();
        store.save_unknown("scope:key").unwrap();
        assert_eq!(
            store.acquire("scope:key", "fingerprint").unwrap(),
            AcquireResult::Unknown
        );
        assert_eq!(
            store.get_status("scope:key").unwrap(),
            Some(Status::Unknown)
        );
    }

    #[test]
    fn cleanup_expired_removes_old_entries() {
        let (_dir, store) = temp_store();
        store
            .conn
            .execute(
                "INSERT INTO idempotency (key, fingerprint, status, response, created_at)
                 VALUES (?1, ?2, 'success', ?3, ?4)",
                params![
                    "expired",
                    "fp",
                    b"old".as_slice(),
                    unix_now() - TTL_SECS - 1
                ],
            )
            .unwrap();
        store.cleanup_expired().unwrap();
        assert_eq!(store.get_status("expired").unwrap(), None);
    }
}
