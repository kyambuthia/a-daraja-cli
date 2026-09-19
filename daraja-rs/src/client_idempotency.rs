// Private idempotency state-transition helpers for [`crate::client::DarajaClient`].
//
// This module owns the duplicated locking, [`AcquireResult`] mapping, and
// outcome-persistence error mapping so `client.rs` keeps a single call site
// per transition. Payment safety semantics are unchanged:
//   - reserving a key maps terminal states to the existing `DarajaError`
//     variants without retrying,
//   - persisting an outcome maps lock/backend failures to
//     `LocalStateUncertain` so ambiguous local state never looks definitive,
//   - a disabled store (`None`) is a no-op that performs no network or disk I/O.

use crate::error::DarajaError;
use crate::idempotency::{AcquireResult, IdempotencyBackend};
use std::sync::{Mutex, MutexGuard};

/// Check the local cache and atomically reserve `scoped_key` for `fingerprint`.
///
/// Returns `Ok(None)` when no store is configured or the key was freshly
/// acquired, and `Ok(Some(response))` for a replayable success.
pub(crate) fn check_cache(
    store: &Option<Mutex<Box<dyn IdempotencyBackend>>>,
    scoped_key: &str,
    fingerprint: &str,
) -> Result<Option<Vec<u8>>, DarajaError> {
    let Some(store) = store else {
        return Ok(None);
    };
    let mut guard = lock(store, || {
        DarajaError::Config("idempotency store lock poisoned".into())
    })?;
    let result = guard.acquire(scoped_key, fingerprint).map_err(|error| {
        DarajaError::Config(format!("failed to reserve idempotency key: {error}"))
    })?;
    map_acquire(result)
}

/// Record a successful provider response for a previously reserved key.
pub(crate) fn record_success(
    store: &Option<Mutex<Box<dyn IdempotencyBackend>>>,
    key: &str,
    response: &[u8],
) -> Result<(), DarajaError> {
    persist(store, "save successful response", |backend| {
        backend.save_success(key, response)
    })
}

/// Record a definitive provider rejection for a previously reserved key.
pub(crate) fn record_failed(
    store: &Option<Mutex<Box<dyn IdempotencyBackend>>>,
    key: &str,
) -> Result<(), DarajaError> {
    persist(store, "save definitive failure", |backend| {
        backend.save_failed(key)
    })
}

/// Record an ambiguous outcome so the key is never silently retried.
pub(crate) fn record_unknown(
    store: &Option<Mutex<Box<dyn IdempotencyBackend>>>,
    key: &str,
) -> Result<(), DarajaError> {
    persist(store, "save ambiguous outcome", |backend| {
        backend.save_unknown(key)
    })
}

/// Map a reservation result to the cached-response-or-error contract.
fn map_acquire(result: AcquireResult) -> Result<Option<Vec<u8>>, DarajaError> {
    match result {
        AcquireResult::Acquired => Ok(None),
        AcquireResult::Success(response) => Ok(Some(response)),
        AcquireResult::Pending => Err(DarajaError::IdempotencyConflict),
        AcquireResult::FingerprintMismatch => Err(DarajaError::IdempotencyKeyReused),
        AcquireResult::Failed => Err(DarajaError::IdempotencyPreviousFailed),
        AcquireResult::Unknown => Err(DarajaError::IdempotencyOutcomeUnknown),
    }
}

/// Shared persistence path: lock once, run one transition, map failures.
fn persist(
    store: &Option<Mutex<Box<dyn IdempotencyBackend>>>,
    context: &str,
    transition: impl FnOnce(&mut Box<dyn IdempotencyBackend>) -> Result<(), String>,
) -> Result<(), DarajaError> {
    let Some(store) = store else {
        return Ok(());
    };
    let mut guard = lock(store, || {
        DarajaError::LocalStateUncertain("idempotency store lock poisoned".into())
    })?;
    transition(&mut guard)
        .map_err(|error| DarajaError::LocalStateUncertain(format!("{context}: {error}")))
}

/// Lock the backend, mapping poisoning to the caller's error domain.
fn lock(
    store: &Mutex<Box<dyn IdempotencyBackend>>,
    on_poison: impl FnOnce() -> DarajaError,
) -> Result<MutexGuard<'_, Box<dyn IdempotencyBackend>>, DarajaError> {
    store.lock().map_err(|_| on_poison())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// In-memory backend so state-transition mapping is tested without SQLite.
    #[derive(Default)]
    struct MemoryBackend {
        states: HashMap<String, AcquireResult>,
        fail_next_save: Option<String>,
    }

    impl MemoryBackend {
        fn acquired_with(state: AcquireResult) -> Option<Mutex<Box<dyn IdempotencyBackend>>> {
            let mut backend = MemoryBackend::default();
            backend.states.insert("key".to_string(), state);
            Some(Mutex::new(Box::new(backend)))
        }
    }

    impl IdempotencyBackend for MemoryBackend {
        fn acquire(&mut self, key: &str, _fingerprint: &str) -> Result<AcquireResult, String> {
            Ok(self.states.remove(key).unwrap_or(AcquireResult::Acquired))
        }

        fn save_success(&mut self, _key: &str, _response: &[u8]) -> Result<(), String> {
            if let Some(error) = self.fail_next_save.take() {
                return Err(error);
            }
            Ok(())
        }

        fn save_failed(&mut self, _key: &str) -> Result<(), String> {
            if let Some(error) = self.fail_next_save.take() {
                return Err(error);
            }
            Ok(())
        }

        fn save_unknown(&mut self, _key: &str) -> Result<(), String> {
            if let Some(error) = self.fail_next_save.take() {
                return Err(error);
            }
            Ok(())
        }
    }

    #[test]
    fn disabled_store_is_a_noop() {
        let none: Option<Mutex<Box<dyn IdempotencyBackend>>> = None;
        assert!(check_cache(&none, "key", "fp").unwrap().is_none());
        assert!(record_success(&none, "key", b"{}").is_ok());
        assert!(record_failed(&none, "key").is_ok());
        assert!(record_unknown(&none, "key").is_ok());
    }

    #[test]
    fn terminal_states_map_to_existing_errors() {
        for (state, expect) in [
            (AcquireResult::Pending, "already in flight"),
            (
                AcquireResult::FingerprintMismatch,
                "different request parameters",
            ),
            (AcquireResult::Failed, "failed; use a new key"),
            (AcquireResult::Unknown, "reconcile the transaction"),
        ] {
            let store = MemoryBackend::acquired_with(state);
            let error = check_cache(&store, "key", "fp").unwrap_err().to_string();
            assert!(error.contains(expect), "unexpected message: {error}");
        }
    }

    #[test]
    fn replayable_success_returns_cached_bytes() {
        let store = MemoryBackend::acquired_with(AcquireResult::Success(b"{}".to_vec()));
        assert_eq!(
            check_cache(&store, "key", "fp").unwrap(),
            Some(b"{}".to_vec())
        );
    }

    #[test]
    fn persistence_failures_stay_ambiguous() {
        let backend = MemoryBackend {
            fail_next_save: Some("disk full".to_string()),
            ..Default::default()
        };
        let store: Option<Mutex<Box<dyn IdempotencyBackend>>> = Some(Mutex::new(Box::new(backend)));
        let error = record_success(&store, "key", b"{}").unwrap_err();
        assert!(matches!(error, DarajaError::LocalStateUncertain(_)));
        assert!(error.to_string().contains("save successful response"));
        assert!(error.to_string().contains("disk full"));
    }
}
