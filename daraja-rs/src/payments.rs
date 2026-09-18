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

/// Money-moving operation represented by a payment record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentKind {
    Stk,
    B2c,
    Reversal,
}

impl PaymentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stk => "stk",
            Self::B2c => "b2c",
            Self::Reversal => "reversal",
        }
    }

    fn parse(value: &str) -> Result<Self, DarajaError> {
        match value {
            "stk" => Ok(Self::Stk),
            "b2c" => Ok(Self::B2c),
            "reversal" => Ok(Self::Reversal),
            other => Err(DarajaError::Config(format!(
                "unknown persisted payment kind: {other}"
            ))),
        }
    }
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
    pub kind: PaymentKind,
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
    pub kind: PaymentKind,
    pub environment: String,
    pub shortcode: String,
    pub amount_kes: u64,
    pub phone: Option<String>,
    pub reference: String,
    pub idempotency_key: Option<String>,
    pub merchant_request_id: Option<String>,
    pub checkout_request_id: Option<String>,
    pub conversation_id: Option<String>,
    pub originator_conversation_id: Option<String>,
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
                kind TEXT NOT NULL DEFAULT 'stk',
                environment TEXT NOT NULL,
                shortcode TEXT NOT NULL,
                amount_kes INTEGER NOT NULL CHECK (amount_kes > 0),
                phone TEXT,
                reference TEXT NOT NULL,
                idempotency_key TEXT UNIQUE,
                merchant_request_id TEXT,
                checkout_request_id TEXT,
                conversation_id TEXT,
                originator_conversation_id TEXT,
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
        ensure_column(&conn, "payments", "kind", "TEXT NOT NULL DEFAULT 'stk'")?;
        ensure_column(&conn, "payments", "conversation_id", "TEXT")?;
        ensure_column(&conn, "payments", "originator_conversation_id", "TEXT")?;
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
                id, kind, environment, shortcode, amount_kes, phone, reference,
                idempotency_key, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                input.id,
                input.kind.as_str(),
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

    /// Attach asynchronous conversation identifiers returned by B2C and
    /// reversal endpoints.
    pub fn attach_conversation_ids(
        &mut self,
        payment_id: &str,
        conversation_id: Option<&str>,
        originator_conversation_id: Option<&str>,
    ) -> Result<Payment, DarajaError> {
        let now = unix_now();
        let changed = self
            .conn
            .execute(
                "UPDATE payments SET
                    conversation_id = COALESCE(?2, conversation_id),
                    originator_conversation_id = COALESCE(?3, originator_conversation_id),
                    updated_at = ?4
                 WHERE id = ?1",
                params![payment_id, conversation_id, originator_conversation_id, now],
            )
            .map_err(|error| {
                DarajaError::Config(format!("attach conversation identifiers: {error}"))
            })?;
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
                "SELECT id, kind, environment, shortcode, amount_kes, phone, reference,
                        idempotency_key, merchant_request_id, checkout_request_id,
                        conversation_id, originator_conversation_id,
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
                "SELECT id, kind, environment, shortcode, amount_kes, phone, reference,
                        idempotency_key, merchant_request_id, checkout_request_id,
                        conversation_id, originator_conversation_id,
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
                "SELECT id, kind, environment, shortcode, amount_kes, phone, reference,
                        idempotency_key, merchant_request_id, checkout_request_id,
                        conversation_id, originator_conversation_id,
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
    let amount_kes: i64 = row.get(4)?;
    Ok(Payment {
        id: row.get(0)?,
        kind: PaymentKind::parse(&row.get::<_, String>(1)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        environment: row.get(2)?,
        shortcode: row.get(3)?,
        amount_kes: amount_kes as u64,
        phone: row.get(5)?,
        reference: row.get(6)?,
        idempotency_key: row.get(7)?,
        merchant_request_id: row.get(8)?,
        checkout_request_id: row.get(9)?,
        conversation_id: row.get(10)?,
        originator_conversation_id: row.get(11)?,
        transaction_id: row.get(12)?,
        receipt_number: row.get(13)?,
        status: PaymentStatus::parse(&row.get::<_, String>(14)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                14,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), DarajaError> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| DarajaError::Config(format!("inspect payment schema: {error}")))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| DarajaError::Config(format!("read payment schema: {error}")))?;
    for existing in columns {
        if existing
            .map_err(|error| DarajaError::Config(format!("decode payment schema: {error}")))?
            == column
        {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )
    .map_err(|error| DarajaError::Config(format!("migrate payment schema: {error}")))?;
    Ok(())
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
            kind: PaymentKind::Stk,
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

    fn legacy_column_names(conn: &rusqlite::Connection) -> Vec<String> {
        let mut statement = conn.prepare("PRAGMA table_info(payments)").unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(|value| value.unwrap())
            .collect()
    }

    fn create_legacy_db_without_kind_or_conversation_ids(path: &std::path::Path) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000;
             CREATE TABLE payments (
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
             CREATE INDEX payments_status_idx ON payments(status);
             CREATE INDEX payments_checkout_idx ON payments(checkout_request_id);
             CREATE TABLE payment_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payment_id TEXT NOT NULL,
                from_status TEXT,
                to_status TEXT NOT NULL,
                detail TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(payment_id) REFERENCES payments(id)
             );
             CREATE INDEX payment_events_payment_idx
                ON payment_events(payment_id, id);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payments (
                id, environment, shortcode, amount_kes, phone, reference,
                idempotency_key, merchant_request_id, checkout_request_id,
                transaction_id, receipt_number, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                "legacy_stk_001",
                "sandbox",
                "174379",
                500_i64,
                "254712345678",
                "LEGACY-ORDER-1",
                "legacy:LEGACY-ORDER-1:stk",
                "mr-legacy-1",
                "ws_CO_legacy_1",
                Option::<String>::None,
                Option::<String>::None,
                "submitted",
                1_700_000_000_i64,
                1_700_000_060_i64,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payment_events
                (payment_id, from_status, to_status, detail, created_at)
             VALUES (?1, NULL, ?2, ?3, ?4)",
            rusqlite::params![
                "legacy_stk_001",
                "created",
                "payment created",
                1_700_000_000_i64,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payment_events
                (payment_id, from_status, to_status, detail, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "legacy_stk_001",
                "created",
                "submitted",
                "STK request accepted",
                1_700_000_060_i64,
            ],
        )
        .unwrap();
    }

    fn create_legacy_db_with_kind_without_conversation_ids(path: &std::path::Path) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000;
             CREATE TABLE payments (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'stk',
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
             CREATE INDEX payments_status_idx ON payments(status);
             CREATE INDEX payments_checkout_idx ON payments(checkout_request_id);
             CREATE TABLE payment_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payment_id TEXT NOT NULL,
                from_status TEXT,
                to_status TEXT NOT NULL,
                detail TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(payment_id) REFERENCES payments(id)
             );
             CREATE INDEX payment_events_payment_idx
                ON payment_events(payment_id, id);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payments (
                id, kind, environment, shortcode, amount_kes, phone, reference,
                idempotency_key, merchant_request_id, checkout_request_id,
                transaction_id, receipt_number, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                "legacy_b2c_001",
                "b2c",
                "sandbox",
                "600000",
                1_000_i64,
                "254722000001",
                "B2C-LEGACY-1",
                "legacy:B2C-LEGACY-1:b2c",
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                "submitted",
                1_700_000_100_i64,
                1_700_000_160_i64,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payment_events
                (payment_id, from_status, to_status, detail, created_at)
             VALUES (?1, NULL, ?2, ?3, ?4)",
            rusqlite::params![
                "legacy_b2c_001",
                "created",
                "payment created",
                1_700_000_100_i64,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payment_events
                (payment_id, from_status, to_status, detail, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "legacy_b2c_001",
                "created",
                "submitted",
                "B2C request accepted",
                1_700_000_160_i64,
            ],
        )
        .unwrap();
    }

    #[test]
    fn legacy_schema_without_kind_and_conversation_ids_upgrades_without_data_loss() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payments.db");
        create_legacy_db_without_kind_or_conversation_ids(&path);
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let columns = legacy_column_names(&conn);
            assert!(!columns.contains(&"kind".to_string()));
            assert!(!columns.contains(&"conversation_id".to_string()));
            assert!(!columns.contains(&"originator_conversation_id".to_string()));
        }
        let mut store = PaymentStore::new(&path).unwrap();
        let migrated = store.get("legacy_stk_001").unwrap().unwrap();
        assert_eq!(migrated.id, "legacy_stk_001");
        assert_eq!(migrated.kind, PaymentKind::Stk);
        assert_eq!(migrated.environment, "sandbox");
        assert_eq!(migrated.shortcode, "174379");
        assert_eq!(migrated.amount_kes, 500);
        assert_eq!(migrated.phone.as_deref(), Some("254712345678"));
        assert_eq!(migrated.reference, "LEGACY-ORDER-1");
        assert_eq!(
            migrated.idempotency_key.as_deref(),
            Some("legacy:LEGACY-ORDER-1:stk")
        );
        assert_eq!(
            migrated.merchant_request_id.as_deref(),
            Some("mr-legacy-1")
        );
        assert_eq!(
            migrated.checkout_request_id.as_deref(),
            Some("ws_CO_legacy_1")
        );
        assert_eq!(migrated.conversation_id, None);
        assert_eq!(migrated.originator_conversation_id, None);
        assert_eq!(migrated.transaction_id, None);
        assert_eq!(migrated.receipt_number, None);
        assert_eq!(migrated.status, PaymentStatus::Submitted);
        assert_eq!(migrated.created_at, 1_700_000_000);
        assert_eq!(migrated.updated_at, 1_700_000_060);
        let events = store.events("legacy_stk_001").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].from, None);
        assert_eq!(events[0].to, "created");
        assert_eq!(events[1].from.as_deref(), Some("created"));
        assert_eq!(events[1].to, "submitted");
        assert_eq!(
            store
                .get_by_idempotency_key("legacy:LEGACY-ORDER-1:stk")
                .unwrap()
                .unwrap()
                .id,
            "legacy_stk_001"
        );
        let pending = store
            .transition("legacy_stk_001", PaymentStatus::Pending, Some("callback waiting"))
            .unwrap();
        assert_eq!(pending.status, PaymentStatus::Pending);
        assert_eq!(store.events("legacy_stk_001").unwrap().len(), 3);
        let rejected = store
            .transition("legacy_stk_001", PaymentStatus::Created, None)
            .unwrap_err();
        assert!(matches!(
            rejected,
            DarajaError::InvalidPaymentTransition { .. }
        ));
        assert_eq!(
            store.get("legacy_stk_001").unwrap().unwrap().status,
            PaymentStatus::Pending
        );
        drop(store);
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let columns = legacy_column_names(&conn);
            assert!(columns.contains(&"kind".to_string()));
            assert!(columns.contains(&"conversation_id".to_string()));
            assert!(columns.contains(&"originator_conversation_id".to_string()));
        }
        let store = PaymentStore::new(&path).unwrap();
        let reopened = store.get("legacy_stk_001").unwrap().unwrap();
        assert_eq!(reopened.status, PaymentStatus::Pending);
        assert_eq!(reopened.kind, PaymentKind::Stk);
        assert_eq!(reopened.checkout_request_id.as_deref(), Some("ws_CO_legacy_1"));
        assert_eq!(store.events("legacy_stk_001").unwrap().len(), 3);
    }

    #[test]
    fn legacy_schema_with_kind_but_without_conversation_ids_upgrades_preserving_kind() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payments.db");
        create_legacy_db_with_kind_without_conversation_ids(&path);
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let columns = legacy_column_names(&conn);
            assert!(columns.contains(&"kind".to_string()));
            assert!(!columns.contains(&"conversation_id".to_string()));
            assert!(!columns.contains(&"originator_conversation_id".to_string()));
        }
        let mut store = PaymentStore::new(&path).unwrap();
        let migrated = store.get("legacy_b2c_001").unwrap().unwrap();
        assert_eq!(migrated.kind, PaymentKind::B2c);
        assert_eq!(migrated.environment, "sandbox");
        assert_eq!(migrated.shortcode, "600000");
        assert_eq!(migrated.amount_kes, 1_000);
        assert_eq!(migrated.status, PaymentStatus::Submitted);
        assert_eq!(migrated.conversation_id, None);
        assert_eq!(migrated.originator_conversation_id, None);
        assert_eq!(migrated.created_at, 1_700_000_100);
        assert_eq!(migrated.updated_at, 1_700_000_160);
        assert_eq!(store.events("legacy_b2c_001").unwrap().len(), 2);
        let attached = store
            .attach_conversation_ids(
                "legacy_b2c_001",
                Some("AG_20240101_0001AbCd"),
                Some("26702-12345678-1"),
            )
            .unwrap();
        assert_eq!(
            attached.conversation_id.as_deref(),
            Some("AG_20240101_0001AbCd")
        );
        assert_eq!(
            attached.originator_conversation_id.as_deref(),
            Some("26702-12345678-1")
        );
        drop(store);
        let store = PaymentStore::new(&path).unwrap();
        let reloaded = store.get("legacy_b2c_001").unwrap().unwrap();
        assert_eq!(reloaded.kind, PaymentKind::B2c);
        assert_eq!(
            reloaded.conversation_id.as_deref(),
            Some("AG_20240101_0001AbCd")
        );
        assert_eq!(
            reloaded.originator_conversation_id.as_deref(),
            Some("26702-12345678-1")
        );
        assert_eq!(store.events("legacy_b2c_001").unwrap().len(), 2);
    }

    #[test]
    fn migrated_store_persists_b2c_conversation_ids_and_guarded_events_across_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payments.db");
        create_legacy_db_without_kind_or_conversation_ids(&path);
        let mut store = PaymentStore::new(&path).unwrap();
        let input = NewPayment {
            id: "b2c_after_migrate_001".into(),
            kind: PaymentKind::B2c,
            environment: "sandbox".into(),
            shortcode: "600000".into(),
            amount_kes: 1_000,
            phone: Some("254722000001".into()),
            reference: "B2C-AFTER-MIGRATE-1".into(),
            idempotency_key: Some("b2c:B2C-AFTER-MIGRATE-1".into()),
        };
        store.create(&input).unwrap();
        store
            .attach_conversation_ids(
                "b2c_after_migrate_001",
                Some("AG_20240101_0001AbCd"),
                Some("26702-12345678-1"),
            )
            .unwrap();
        store
            .transition(
                "b2c_after_migrate_001",
                PaymentStatus::Submitted,
                Some("B2C request accepted"),
            )
            .unwrap();
        store
            .transition("b2c_after_migrate_001", PaymentStatus::Pending, None)
            .unwrap();
        let succeeded = store
            .transition(
                "b2c_after_migrate_001",
                PaymentStatus::Succeeded,
                Some("result matched"),
            )
            .unwrap();
        assert_eq!(succeeded.kind, PaymentKind::B2c);
        assert_eq!(
            succeeded.conversation_id.as_deref(),
            Some("AG_20240101_0001AbCd")
        );
        assert_eq!(
            succeeded.originator_conversation_id.as_deref(),
            Some("26702-12345678-1")
        );
        let events = store.events("b2c_after_migrate_001").unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(
            events.iter().map(|event| event.to.as_str()).collect::<Vec<_>>(),
            vec!["created", "submitted", "pending", "succeeded"]
        );
        drop(store);
        let mut store = PaymentStore::new(&path).unwrap();
        let reloaded = store.get("b2c_after_migrate_001").unwrap().unwrap();
        assert_eq!(reloaded.kind, PaymentKind::B2c);
        assert_eq!(reloaded.status, PaymentStatus::Succeeded);
        assert_eq!(
            reloaded.conversation_id.as_deref(),
            Some("AG_20240101_0001AbCd")
        );
        assert_eq!(
            reloaded.originator_conversation_id.as_deref(),
            Some("26702-12345678-1")
        );
        assert_eq!(store.events("b2c_after_migrate_001").unwrap().len(), 4);
        assert_eq!(
            store.get("legacy_stk_001").unwrap().unwrap().kind,
            PaymentKind::Stk
        );
        let rejected = store
            .transition("b2c_after_migrate_001", PaymentStatus::Submitted, None)
            .unwrap_err();
        assert!(matches!(
            rejected,
            DarajaError::InvalidPaymentTransition { .. }
        ));
        assert_eq!(store.events("b2c_after_migrate_001").unwrap().len(), 4);
    }

    #[test]
    fn migrated_store_persists_reversal_conversation_ids_and_guarded_events_across_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payments.db");
        create_legacy_db_without_kind_or_conversation_ids(&path);
        let mut store = PaymentStore::new(&path).unwrap();
        let input = NewPayment {
            id: "rev_after_migrate_001".into(),
            kind: PaymentKind::Reversal,
            environment: "sandbox".into(),
            shortcode: "174379".into(),
            amount_kes: 500,
            phone: None,
            reference: "REV-AFTER-MIGRATE-1".into(),
            idempotency_key: Some("reversal:REV-AFTER-MIGRATE-1".into()),
        };
        store.create(&input).unwrap();
        store
            .attach_conversation_ids(
                "rev_after_migrate_001",
                Some("AG_20240102_0002EfGh"),
                Some("26702-87654321-1"),
            )
            .unwrap();
        store
            .transition(
                "rev_after_migrate_001",
                PaymentStatus::Submitted,
                Some("reversal request accepted"),
            )
            .unwrap();
        store
            .transition(
                "rev_after_migrate_001",
                PaymentStatus::Succeeded,
                Some("reversal accepted"),
            )
            .unwrap();
        let reversed = store
            .transition(
                "rev_after_migrate_001",
                PaymentStatus::Reversed,
                Some("original transaction reversed"),
            )
            .unwrap();
        assert_eq!(reversed.kind, PaymentKind::Reversal);
        assert_eq!(
            reversed.conversation_id.as_deref(),
            Some("AG_20240102_0002EfGh")
        );
        assert_eq!(
            reversed.originator_conversation_id.as_deref(),
            Some("26702-87654321-1")
        );
        let events = store.events("rev_after_migrate_001").unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(
            events.iter().map(|event| event.to.as_str()).collect::<Vec<_>>(),
            vec!["created", "submitted", "succeeded", "reversed"]
        );
        drop(store);
        let mut store = PaymentStore::new(&path).unwrap();
        let reloaded = store.get("rev_after_migrate_001").unwrap().unwrap();
        assert_eq!(reloaded.kind, PaymentKind::Reversal);
        assert_eq!(reloaded.status, PaymentStatus::Reversed);
        assert_eq!(
            reloaded.conversation_id.as_deref(),
            Some("AG_20240102_0002EfGh")
        );
        assert_eq!(
            reloaded.originator_conversation_id.as_deref(),
            Some("26702-87654321-1")
        );
        assert_eq!(store.events("rev_after_migrate_001").unwrap().len(), 4);
        let rejected = store
            .transition("rev_after_migrate_001", PaymentStatus::Succeeded, None)
            .unwrap_err();
        assert!(matches!(
            rejected,
            DarajaError::InvalidPaymentTransition { .. }
        ));
        assert_eq!(store.events("rev_after_migrate_001").unwrap().len(), 4);
    }
}
