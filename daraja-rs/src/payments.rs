//! Durable payment records and guarded state transitions.

use crate::error::DarajaError;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Payment lifecycle states persisted by [`PaymentStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentStatus {
    Created,
    Submitted,
    Pending,
    Succeeded,
    Failed,
    Unknown,
    Reversed,
}

impl PaymentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Submitted => "submitted",
            Self::Pending => "pending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Reversed => "reversed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, DarajaError> {
        match value {
            "created" => Ok(Self::Created),
            "submitted" => Ok(Self::Submitted),
            "pending" => Ok(Self::Pending),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            "reversed" => Ok(Self::Reversed),
            other => Err(DarajaError::Config(format!(
                "unknown persisted payment status: {other}"
            ))),
        }
    }

    fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Created,
                Self::Submitted | Self::Failed | Self::Unknown
            ) | (
                Self::Submitted,
                Self::Pending | Self::Succeeded | Self::Failed | Self::Unknown
            ) | (
                Self::Pending,
                Self::Succeeded | Self::Failed | Self::Unknown
            ) | (
                Self::Unknown,
                Self::Pending | Self::Succeeded | Self::Failed
            ) | (Self::Succeeded, Self::Reversed)
        )
    }
}

/// Data needed to create a payment record before sending a provider request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPayment {
    pub id: String,
    pub environment: String,
    pub shortcode: String,
    pub amount_kes: u64,
    pub phone: Option<String>,
    pub reference: String,
    pub idempotency_key: Option<String>,
}

/// Persisted payment record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment {
    pub id: String,
    pub environment: String,
    pub shortcode: String,
    pub amount_kes: u64,
    pub phone: Option<String>,
    pub reference: String,
    pub idempotency_key: Option<String>,
    pub merchant_request_id: Option<String>,
    pub checkout_request_id: Option<String>,
    pub transaction_id: Option<String>,
    pub receipt_number: Option<String>,
    pub status: PaymentStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

/// SQLite-backed payment ledger.
pub struct PaymentStore {
    conn: Connection,
}

impl PaymentStore {
    /// Open (or create) a payment database.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self, DarajaError> {
        let conn = Connection::open(db_path)
            .map_err(|error| DarajaError::Config(format!("open payment store: {error}")))?;
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS payments (
                id TEXT PRIMARY KEY,
                environment TEXT NOT NULL,
                shortcode TEXT NOT NULL,
                amount_kes INTEGER NOT NULL CHECK (amount_kes > 0),
                phone TEXT,
                reference TEXT NOT NULL,
                idempotency_key TEXT UNIQUE,
                merchant_request_id TEXT,
                checkout_request_id TEXT,
                transaction_id TEXT,
                receipt_number TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS payments_status_idx ON payments(status);
             CREATE INDEX IF NOT EXISTS payments_checkout_idx ON payments(checkout_request_id);
             CREATE TABLE IF NOT EXISTS payment_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payment_id TEXT NOT NULL,
                from_status TEXT,
                to_status TEXT NOT NULL,
                detail TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(payment_id) REFERENCES payments(id)
             );
             CREATE INDEX IF NOT EXISTS payment_events_payment_idx
                ON payment_events(payment_id, id);",
        )
        .map_err(|error| DarajaError::Config(format!("initialize payment store: {error}")))?;
        Ok(Self { conn })
    }

    /// Create a record before submitting a provider request.
    pub fn create(&mut self, input: &NewPayment) -> Result<Payment, DarajaError> {
        if input.id.trim().is_empty()
            || input.environment.trim().is_empty()
            || input.shortcode.trim().is_empty()
            || input.reference.trim().is_empty()
        {
            return Err(DarajaError::Config(
                "payment id, environment, shortcode, and reference are required".into(),
            ));
        }
        if input.amount_kes == 0 || input.amount_kes > i64::MAX as u64 {
            return Err(DarajaError::Config(
                "payment amount must be a positive whole KES amount".into(),
            ));
        }
        let now = unix_now();
        let tx = self
            .conn
            .transaction()
            .map_err(|error| DarajaError::Config(format!("begin payment create: {error}")))?;
        tx.execute(
            "INSERT INTO payments (
                id, environment, shortcode, amount_kes, phone, reference,
                idempotency_key, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                input.id,
                input.environment,
                input.shortcode,
                input.amount_kes as i64,
                input.phone,
                input.reference,
                input.idempotency_key,
                PaymentStatus::Created.as_str(),
                now,
            ],
        )
        .map_err(|error| DarajaError::Config(format!("create payment: {error}")))?;
        tx.execute(
            "INSERT INTO payment_events
                (payment_id, from_status, to_status, detail, created_at)
             VALUES (?1, NULL, ?2, ?3, ?4)",
            params![
                input.id,
                PaymentStatus::Created.as_str(),
                "payment created",
                now
            ],
        )
        .map_err(|error| DarajaError::Config(format!("record payment creation: {error}")))?;
        tx.commit()
            .map_err(|error| DarajaError::Config(format!("commit payment create: {error}")))?;
        self.get(&input.id)?.ok_or_else(|| {
            DarajaError::Config("payment was created but could not be reloaded".into())
        })
    }

    /// Transition a payment and append an immutable lifecycle event.
    pub fn transition(
        &mut self,
        payment_id: &str,
        next: PaymentStatus,
        detail: Option<&str>,
    ) -> Result<Payment, DarajaError> {
        let current = self
            .get(payment_id)?
            .ok_or_else(|| DarajaError::TransactionNotFound(payment_id.into()))?;
        if !current.status.can_transition_to(next) {
            return Err(DarajaError::InvalidPaymentTransition {
                from: current.status.as_str().into(),
                to: next.as_str().into(),
            });
        }
        let now = unix_now();
        let tx = self
            .conn
            .transaction()
            .map_err(|error| DarajaError::Config(format!("begin payment transition: {error}")))?;
        tx.execute(
            "UPDATE payments SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![payment_id, next.as_str(), now],
        )
        .map_err(|error| DarajaError::Config(format!("update payment status: {error}")))?;
        tx.execute(
            "INSERT INTO payment_events
                (payment_id, from_status, to_status, detail, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                payment_id,
                current.status.as_str(),
                next.as_str(),
                detail,
                now
            ],
        )
        .map_err(|error| DarajaError::Config(format!("record payment transition: {error}")))?;
        tx.commit()
            .map_err(|error| DarajaError::Config(format!("commit payment transition: {error}")))?;
        self.get(payment_id)?.ok_or_else(|| {
            DarajaError::Config("payment transitioned but could not be reloaded".into())
        })
    }

    /// Attach provider identifiers without changing lifecycle state.
    pub fn attach_provider_ids(
        &mut self,
        payment_id: &str,
        merchant_request_id: Option<&str>,
        checkout_request_id: Option<&str>,
        transaction_id: Option<&str>,
        receipt_number: Option<&str>,
    ) -> Result<Payment, DarajaError> {
        let now = unix_now();
        let changed = self
            .conn
            .execute(
                "UPDATE payments SET
                    merchant_request_id = COALESCE(?2, merchant_request_id),
                    checkout_request_id = COALESCE(?3, checkout_request_id),
                    transaction_id = COALESCE(?4, transaction_id),
                    receipt_number = COALESCE(?5, receipt_number),
                    updated_at = ?6
                 WHERE id = ?1",
                params![
                    payment_id,
                    merchant_request_id,
                    checkout_request_id,
                    transaction_id,
                    receipt_number,
                    now
                ],
            )
            .map_err(|error| DarajaError::Config(format!("attach payment identifiers: {error}")))?;
        if changed == 0 {
            return Err(DarajaError::TransactionNotFound(payment_id.into()));
        }
        self.get(payment_id)?.ok_or_else(|| {
            DarajaError::Config("payment identifiers saved but could not be reloaded".into())
        })
    }

    /// Look up a payment by its durable identifier.
    pub fn get(&self, payment_id: &str) -> Result<Option<Payment>, DarajaError> {
        self.conn
            .query_row(
                "SELECT id, environment, shortcode, amount_kes, phone, reference,
                        idempotency_key, merchant_request_id, checkout_request_id,
                        transaction_id, receipt_number, status, created_at, updated_at
                 FROM payments WHERE id = ?1",
                params![payment_id],
                payment_from_row,
            )
            .optional()
            .map_err(|error| DarajaError::Config(format!("read payment: {error}")))
    }

    /// Look up a payment by the caller's idempotency key.
    pub fn get_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<Payment>, DarajaError> {
        self.conn
            .query_row(
                "SELECT id, environment, shortcode, amount_kes, phone, reference,
                        idempotency_key, merchant_request_id, checkout_request_id,
                        transaction_id, receipt_number, status, created_at, updated_at
                 FROM payments WHERE idempotency_key = ?1",
                params![idempotency_key],
                payment_from_row,
            )
            .optional()
            .map_err(|error| {
                DarajaError::Config(format!("read payment by idempotency key: {error}"))
            })
    }

    /// Return the immutable event history for a payment.
    pub fn events(&self, payment_id: &str) -> Result<Vec<PaymentEvent>, DarajaError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT from_status, to_status, detail, created_at
                 FROM payment_events WHERE payment_id = ?1 ORDER BY id",
            )
            .map_err(|error| DarajaError::Config(format!("prepare payment events: {error}")))?;
        let rows = statement
            .query_map(params![payment_id], |row| {
                Ok(PaymentEvent {
                    from: row.get(0)?,
                    to: row.get(1)?,
                    detail: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|error| DarajaError::Config(format!("read payment events: {error}")))?;
        rows.map(|row| {
            row.map_err(|error| DarajaError::Config(format!("decode payment event: {error}")))
        })
        .collect()
    }

    /// List payments, optionally narrowed to one lifecycle status.
    pub fn list(&self, status: Option<PaymentStatus>) -> Result<Vec<Payment>, DarajaError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, environment, shortcode, amount_kes, phone, reference,
                        idempotency_key, merchant_request_id, checkout_request_id,
                        transaction_id, receipt_number, status, created_at, updated_at
                 FROM payments
                 WHERE (?1 IS NULL OR status = ?1)
                 ORDER BY created_at DESC, id DESC",
            )
            .map_err(|error| DarajaError::Config(format!("prepare payment list: {error}")))?;
        let rows = statement
            .query_map(params![status.map(PaymentStatus::as_str)], payment_from_row)
            .map_err(|error| DarajaError::Config(format!("read payment list: {error}")))?;
        rows.map(|row| {
            row.map_err(|error| DarajaError::Config(format!("decode payment list: {error}")))
        })
        .collect()
    }
}

/// One immutable payment status event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentEvent {
    pub from: Option<String>,
    pub to: String,
    pub detail: Option<String>,
    pub created_at: i64,
}

/// Provider outcome obtained while reconciling an ambiguous payment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationOutcome {
    Pending {
        detail: String,
    },
    Succeeded {
        transaction_id: Option<String>,
        receipt_number: Option<String>,
        detail: String,
    },
    Failed {
        detail: String,
    },
}

impl PaymentStore {
    /// Apply an authoritative provider outcome without creating duplicate
    /// lifecycle events when the same result is observed again.
    pub fn apply_reconciliation_outcome(
        &mut self,
        payment_id: &str,
        outcome: ReconciliationOutcome,
    ) -> Result<Payment, DarajaError> {
        let current = self
            .get(payment_id)?
            .ok_or_else(|| DarajaError::TransactionNotFound(payment_id.into()))?;
        match outcome {
            ReconciliationOutcome::Pending { detail } => {
                if current.status == PaymentStatus::Pending {
                    return Ok(current);
                }
                self.transition(payment_id, PaymentStatus::Pending, Some(&detail))
            }
            ReconciliationOutcome::Succeeded {
                transaction_id,
                receipt_number,
                detail,
            } => {
                self.attach_provider_ids(
                    payment_id,
                    None,
                    None,
                    transaction_id.as_deref(),
                    receipt_number.as_deref(),
                )?;
                if current.status == PaymentStatus::Succeeded {
                    return self.get(payment_id)?.ok_or_else(|| {
                        DarajaError::Config("succeeded payment could not be reloaded".into())
                    });
                }
                self.transition(payment_id, PaymentStatus::Succeeded, Some(&detail))
            }
            ReconciliationOutcome::Failed { detail } => {
                if current.status == PaymentStatus::Failed {
                    return Ok(current);
                }
                self.transition(payment_id, PaymentStatus::Failed, Some(&detail))
            }
        }
    }
}

fn payment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Payment> {
    let amount_kes: i64 = row.get(3)?;
    Ok(Payment {
        id: row.get(0)?,
        environment: row.get(1)?,
        shortcode: row.get(2)?,
        amount_kes: amount_kes as u64,
        phone: row.get(4)?,
        reference: row.get(5)?,
        idempotency_key: row.get(6)?,
        merchant_request_id: row.get(7)?,
        checkout_request_id: row.get(8)?,
        transaction_id: row.get(9)?,
        receipt_number: row.get(10)?,
        status: PaymentStatus::parse(&row.get::<_, String>(11)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                11,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
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

    fn store() -> (tempfile::TempDir, PaymentStore) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payments.db");
        (directory, PaymentStore::new(path).unwrap())
    }

    fn payment() -> NewPayment {
        NewPayment {
            id: "pay_001".into(),
            environment: "sandbox".into(),
            shortcode: "174379".into(),
            amount_kes: 500,
            phone: Some("254712345678".into()),
            reference: "ORDER-123".into(),
            idempotency_key: Some("order:ORDER-123:stk".into()),
        }
    }

    #[test]
    fn create_records_initial_state_and_event() {
        let (_directory, mut store) = store();
        let created = store.create(&payment()).unwrap();
        assert_eq!(created.status, PaymentStatus::Created);
        let events = store.events("pay_001").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].to, "created");
        assert!(events[0].from.is_none());
    }

    #[test]
    fn happy_path_preserves_provider_ids_and_history() {
        let (_directory, mut store) = store();
        store.create(&payment()).unwrap();
        store
            .transition(
                "pay_001",
                PaymentStatus::Submitted,
                Some("STK request accepted"),
            )
            .unwrap();
        store
            .attach_provider_ids("pay_001", Some("mr-1"), Some("ws_CO_1"), None, None)
            .unwrap();
        store
            .transition("pay_001", PaymentStatus::Pending, None)
            .unwrap();
        store
            .attach_provider_ids("pay_001", None, None, Some("QAB123"), Some("QAB123"))
            .unwrap();
        let succeeded = store
            .transition(
                "pay_001",
                PaymentStatus::Succeeded,
                Some("callback matched"),
            )
            .unwrap();
        assert_eq!(succeeded.checkout_request_id.as_deref(), Some("ws_CO_1"));
        assert_eq!(succeeded.receipt_number.as_deref(), Some("QAB123"));
        assert_eq!(store.events("pay_001").unwrap().len(), 4);
    }

    #[test]
    fn invalid_transition_is_rejected_without_mutating_state() {
        let (_directory, mut store) = store();
        store.create(&payment()).unwrap();
        let error = store
            .transition("pay_001", PaymentStatus::Reversed, None)
            .unwrap_err();
        assert!(matches!(
            error,
            DarajaError::InvalidPaymentTransition { .. }
        ));
        assert_eq!(
            store.get("pay_001").unwrap().unwrap().status,
            PaymentStatus::Created
        );
        assert_eq!(store.events("pay_001").unwrap().len(), 1);
    }

    #[test]
    fn unknown_outcome_can_be_reconciled_once() {
        let (_directory, mut store) = store();
        store.create(&payment()).unwrap();
        store
            .transition("pay_001", PaymentStatus::Submitted, None)
            .unwrap();
        store
            .transition("pay_001", PaymentStatus::Unknown, Some("request timed out"))
            .unwrap();
        store
            .transition(
                "pay_001",
                PaymentStatus::Succeeded,
                Some("status query matched"),
            )
            .unwrap();
        assert_eq!(
            store.get("pay_001").unwrap().unwrap().status,
            PaymentStatus::Succeeded
        );
    }

    #[test]
    fn repeated_reconciliation_is_idempotent() {
        let (_directory, mut store) = store();
        store.create(&payment()).unwrap();
        store
            .transition("pay_001", PaymentStatus::Submitted, None)
            .unwrap();
        let outcome = ReconciliationOutcome::Succeeded {
            transaction_id: Some("QAB123".into()),
            receipt_number: Some("QAB123".into()),
            detail: "status query matched".into(),
        };
        store
            .apply_reconciliation_outcome("pay_001", outcome.clone())
            .unwrap();
        store
            .apply_reconciliation_outcome("pay_001", outcome)
            .unwrap();
        assert_eq!(store.events("pay_001").unwrap().len(), 3);
    }

    #[test]
    fn idempotency_key_lookup_survives_reopening_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payments.db");
        {
            let mut store = PaymentStore::new(&path).unwrap();
            store.create(&payment()).unwrap();
        }
        let store = PaymentStore::new(&path).unwrap();
        assert_eq!(
            store
                .get_by_idempotency_key("order:ORDER-123:stk")
                .unwrap()
                .unwrap()
                .id,
            "pay_001"
        );
    }
}
