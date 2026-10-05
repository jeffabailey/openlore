//! `adapter-review-store` — the review app's private, owner-scoped store
//! (ADR-074): one DuckDB file, `review-app.duckdb`, separate from the user's
//! claim store and the indexer's index.
//!
//! Secret blobs (OAuth tokens, DPoP keys, session state) are sealed with
//! XChaCha20-Poly1305 under the operator's data key; the associated data
//! binds a blob to its owner and slot, so a blob copied to another owner's
//! row does not open. Linked ONLY by `openlore-review-app` (check-arch).

#![forbid(unsafe_code)]

mod expiry;
mod github_links;
mod kpi;
mod plans;
mod probe;
mod purge;
mod scan_runs;
pub mod schema;
mod secrets;
mod sessions;
mod suggestions;

use std::path::Path;
use std::sync::Mutex;

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use duckdb::Connection;
use ports::{ProbeOutcome, ReviewStorePort};

use schema::{schema_verdict, SchemaVerdict, SCHEMA_VERSION};

/// Length of the XChaCha20 nonce prefixed to every sealed blob.
const NONCE_LEN: usize = 24;

/// The operator's 32-byte data key and its key id (SSM `data-key`,
/// `{"kid":"d1","key":"<base64>"}`). Every sealed blob is tagged with the
/// kid that sealed it, so a rotation can find the rows still on an old key.
pub struct DataKey {
    kid: String,
    key: [u8; 32],
}

/// The kid of a key given without one (legacy hex format).
pub const LEGACY_KID: &str = "legacy";

/// Key ids are short printable labels (they prefix every sealed blob).
const MAX_KID_LEN: usize = 32;

impl DataKey {
    /// A key named `kid`; `None` when the kid is empty, too long or not
    /// printable ASCII.
    pub fn new(kid: &str, key: [u8; 32]) -> Option<Self> {
        let printable = kid.bytes().all(|b| b.is_ascii_graphic());
        (!kid.is_empty() && kid.len() <= MAX_KID_LEN && printable).then(|| Self {
            kid: kid.to_string(),
            key,
        })
    }

    /// A key without an operator kid (the legacy bare 64-hex `data-key`),
    /// tagged `legacy` so a rotation can move its rows to a named key.
    pub fn from_bytes(key: [u8; 32]) -> Self {
        Self {
            kid: LEGACY_KID.to_string(),
            key,
        }
    }

    /// A throwaway key (the image self-test).
    pub fn generate() -> Self {
        Self {
            kid: "ephemeral".to_string(),
            key: XChaCha20Poly1305::generate_key(&mut OsRng).into(),
        }
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(&self.key.into())
    }
}

/// `kid_len ‖ kid ‖ nonce ‖ ciphertext` — split into the kid and the rest.
pub(crate) fn split_kid(blob: &[u8]) -> Option<(&[u8], &[u8])> {
    let (&len, rest) = blob.split_first()?;
    let len = usize::from(len);
    (rest.len() >= len).then(|| rest.split_at(len))
}

/// Seal under `key`, bound to `aad`, tagged with the key's kid.
pub(crate) fn seal_with(
    key: &DataKey,
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, StoreError> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = key
        .cipher()
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| StoreError::Seal)?;
    let kid_len = u8::try_from(key.kid.len()).map_err(|_| StoreError::Seal)?;
    Ok([
        &[kid_len][..],
        key.kid.as_bytes(),
        nonce.as_slice(),
        sealed.as_slice(),
    ]
    .concat())
}

/// Open a blob sealed by [`seal_with`] under the same key and `aad`.
pub(crate) fn unseal_with(key: &DataKey, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, StoreError> {
    let (kid, rest) = split_kid(blob).ok_or(StoreError::Seal)?;
    if kid != key.kid.as_bytes() || rest.len() < NONCE_LEN {
        return Err(StoreError::Seal);
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    key.cipher()
        .decrypt(XNonce::from_slice(nonce), Payload { msg: sealed, aad })
        .map_err(|_| StoreError::Seal)
}

/// Why the store could not be opened or used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// DuckDB refused to open or run a statement.
    Database(String),
    /// The file was written by a newer build.
    SchemaTooNew { found: i64, supported: i64 },
    /// The file records a version this build never wrote.
    SchemaUnknown { found: i64 },
    /// Sealing or opening a secret blob failed.
    Seal,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(e) => write!(f, "review store database error: {e}"),
            Self::SchemaTooNew { found, supported } => write!(
                f,
                "review store schema version {found} is newer than this build supports ({supported})"
            ),
            Self::SchemaUnknown { found } => {
                write!(f, "review store schema version {found} is unknown")
            }
            Self::Seal => write!(f, "review store could not seal or open a secret blob"),
        }
    }
}

impl std::error::Error for StoreError {}

fn db_error(e: duckdb::Error) -> StoreError {
    StoreError::Database(e.to_string())
}

/// The private store: one DuckDB connection plus the AEAD cipher.
pub struct ReviewStore {
    conn: Mutex<Connection>,
    key: DataKey,
}

impl ReviewStore {
    /// Open (or create) the store file and bring it to the v1 schema,
    /// refusing a file written by a newer build.
    pub fn open(path: &Path, key: DataKey) -> Result<Self, StoreError> {
        Self::from_connection(Connection::open(path).map_err(db_error)?, key)
    }

    /// An in-memory store (the image self-test: no disk, no network).
    pub fn open_in_memory(key: DataKey) -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory().map_err(db_error)?, key)
    }

    fn from_connection(conn: Connection, key: DataKey) -> Result<Self, StoreError> {
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            key,
        })
    }

    /// Seal `plaintext` bound to `aad` under the active key.
    pub(crate) fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        seal_with(&self.key, aad, plaintext)
    }

    /// Open a blob sealed under the active key with the same `aad`.
    pub(crate) fn unseal(&self, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, StoreError> {
        unseal_with(&self.key, aad, blob)
    }

    pub(crate) fn active_key(&self) -> &DataKey {
        &self.key
    }

    pub(crate) fn with_connection<T>(
        &self,
        work: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| StoreError::Database("connection lock poisoned".into()))?;
        work(&conn)
    }
}

impl ReviewStorePort for ReviewStore {
    fn probe(&self) -> ProbeOutcome {
        probe::run(self)
    }
}

/// The recorded schema version, if any.
pub(crate) fn recorded_version(conn: &Connection) -> Result<Option<i64>, StoreError> {
    conn.execute_batch(schema::CREATE_SCHEMA_VERSION)
        .map_err(db_error)?;
    conn.query_row(schema::READ_SCHEMA_VERSION, [], |row| row.get(0))
        .map_err(db_error)
}

/// Bring a connection to the v1 schema, or refuse it.
fn migrate(conn: &Connection) -> Result<(), StoreError> {
    match schema_verdict(recorded_version(conn)?) {
        SchemaVerdict::Current => Ok(()),
        SchemaVerdict::Fresh => {
            for ddl in schema::V1_TABLES {
                conn.execute_batch(ddl).map_err(db_error)?;
            }
            conn.execute(schema::RECORD_SCHEMA_VERSION, [SCHEMA_VERSION])
                .map_err(db_error)?;
            Ok(())
        }
        SchemaVerdict::TooNew { found } => Err(StoreError::SchemaTooNew {
            found,
            supported: SCHEMA_VERSION,
        }),
        SchemaVerdict::Unknown { found } => Err(StoreError::SchemaUnknown { found }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // bypass: one integration example per refusal — the wiring of the
    // version gate into `open` (the verdict itself is property-tested).
    #[test]
    fn a_file_written_by_a_newer_build_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review-app.duckdb");
        drop(ReviewStore::open(&path, DataKey::generate()).unwrap());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(schema::RECORD_SCHEMA_VERSION, [SCHEMA_VERSION + 1])
                .unwrap();
        }
        let refused = ReviewStore::open(&path, DataKey::generate()).err();
        assert_eq!(
            refused,
            Some(StoreError::SchemaTooNew {
                found: SCHEMA_VERSION + 1,
                supported: SCHEMA_VERSION
            })
        );
    }
}
