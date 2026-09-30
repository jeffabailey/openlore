//! The shared store handle: holds the DuckDB file open only while in use.
//!
//! DuckDB lets ONE process hold a database file open read-write, and holding
//! it at all excludes every other process. A long-lived connection therefore
//! locks the store for the process's whole life, so `openlore ui` would
//! block every CLI verb (and vice versa). [`SharedConn`] opens the file on
//! demand: [`SharedConn::lock`] serializes the adapters of one process (as
//! the old `Arc<Mutex<Connection>>` did) and opens the file if it is closed;
//! once no guard has been handed out for [`IDLE_CLOSE`], a reaper thread
//! closes it and releases the file lock. A burst of operations (the queries
//! of one viewer page, the writes of one CLI verb) shares one open.
//!
//! When another process holds the file, `lock` retries with a short backoff
//! for up to the busy timeout — each process holds the file only while it is
//! busy plus [`IDLE_CLOSE`], so the wait is normally well under a second.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use duckdb::Connection;

/// How long [`SharedConn::lock`] waits for another process to release the
/// file before giving up.
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(15);
const FIRST_RETRY_DELAY: Duration = Duration::from_millis(2);
const MAX_RETRY_DELAY: Duration = Duration::from_millis(50);

/// How long an unused connection stays open before the reaper closes it.
pub(crate) const IDLE_CLOSE: Duration = Duration::from_millis(100);

/// DuckDB's message when another process holds the file lock.
const LOCK_CONFLICT_MARKER: &str = "Could not set lock on file";

/// A cloneable handle to one DuckDB file; every adapter over the same store
/// shares one (the single-writer constraint, Q-DELIVER-3).
#[derive(Clone)]
pub(crate) struct SharedConn {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    busy_timeout: Duration,
    slot: Mutex<Slot>,
}

/// The open connection (if any) and when a guard last released it.
struct Slot {
    conn: Option<Connection>,
    last_used: Instant,
}

/// Why [`SharedConn::lock`] could not hand out a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConnError {
    /// Another process held the file for the whole busy timeout.
    Busy { path: PathBuf, waited: Duration },
    /// DuckDB refused to open the file for another reason.
    Open { path: PathBuf, message: String },
}

impl fmt::Display for ConnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnError::Busy { path, waited } => write!(
                f,
                "the store at {} stayed busy (another openlore process held it) for {:.1}s",
                path.display(),
                waited.as_secs_f64()
            ),
            ConnError::Open { path, message } => {
                write!(f, "open duckdb at {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ConnError {}

impl SharedConn {
    /// A handle to the DuckDB file at `path`. Opens nothing yet.
    pub(crate) fn new(path: &Path) -> Self {
        Self::with_busy_timeout(path, DEFAULT_BUSY_TIMEOUT)
    }

    pub(crate) fn with_busy_timeout(path: &Path, busy_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                path: path.to_path_buf(),
                busy_timeout,
                slot: Mutex::new(Slot {
                    conn: None,
                    last_used: Instant::now(),
                }),
            }),
        }
    }

    /// A connection for one store operation, opening the file if it is
    /// closed. The file stays open (and locked against other processes)
    /// while the guard lives and for [`IDLE_CLOSE`] after the last one drops.
    pub(crate) fn lock(&self) -> Result<ConnGuard<'_>, ConnError> {
        let mut slot = self
            .inner
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if slot.conn.is_none() {
            slot.conn = Some(open_waiting_out_other_processes(
                &self.inner.path,
                self.inner.busy_timeout,
            )?);
            spawn_idle_reaper(Arc::downgrade(&self.inner));
        }
        Ok(ConnGuard { slot })
    }
}

/// An open connection to the store, exclusive within this process.
pub(crate) struct ConnGuard<'a> {
    slot: MutexGuard<'a, Slot>,
}

const OPENED_BY_LOCK: &str = "SharedConn::lock opens the connection before handing out a guard";

impl Deref for ConnGuard<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.slot.conn.as_ref().expect(OPENED_BY_LOCK)
    }
}

impl DerefMut for ConnGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        self.slot.conn.as_mut().expect(OPENED_BY_LOCK)
    }
}

impl Drop for ConnGuard<'_> {
    fn drop(&mut self) {
        self.slot.last_used = Instant::now();
    }
}

/// Close the connection once it has sat unused for [`IDLE_CLOSE`]. One
/// reaper runs per open (the connection is only opened when the slot is
/// empty, and the reaper exits once it empties it); it holds a `Weak` so a
/// dropped handle ends it.
fn spawn_idle_reaper(inner: Weak<Inner>) {
    thread::spawn(move || loop {
        thread::sleep(IDLE_CLOSE / 2);
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let mut slot = match inner.slot.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            // In use right now — not idle.
            Err(TryLockError::WouldBlock) => continue,
        };
        if slot.last_used.elapsed() >= IDLE_CLOSE {
            slot.conn = None;
            return;
        }
    });
}

/// Open `path`, retrying with backoff while another process holds it.
fn open_waiting_out_other_processes(
    path: &Path,
    busy_timeout: Duration,
) -> Result<Connection, ConnError> {
    let started = Instant::now();
    let mut delay = FIRST_RETRY_DELAY;
    loop {
        match Connection::open(path) {
            Ok(conn) => return Ok(conn),
            Err(err) => {
                let message = err.to_string();
                if !message.contains(LOCK_CONFLICT_MARKER) {
                    return Err(ConnError::Open {
                        path: path.to_path_buf(),
                        message,
                    });
                }
                let waited = started.elapsed();
                if waited >= busy_timeout {
                    return Err(ConnError::Busy {
                        path: path.to_path_buf(),
                        waited,
                    });
                }
                thread::sleep(delay.min(busy_timeout - waited));
                delay = (delay * 2).min(MAX_RETRY_DELAY);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// A TEMP table lives exactly as long as its connection, so it shows
    /// whether two guards shared one open.
    fn temp_table_visible(conn: &SharedConn) -> bool {
        conn.lock()
            .expect("open")
            .query_row("SELECT count(*) FROM probe_tmp", [], |r| r.get::<_, i64>(0))
            .is_ok()
    }

    #[test]
    fn guards_in_quick_succession_share_one_open() {
        let dir = tempdir().expect("tempdir");
        let conn = SharedConn::new(&dir.path().join("store.duckdb"));

        conn.lock()
            .expect("open")
            .execute_batch("CREATE TEMP TABLE probe_tmp (x INTEGER)")
            .expect("temp table");
        assert!(
            temp_table_visible(&conn),
            "the second guard reuses the open"
        );
    }

    #[test]
    fn an_idle_connection_is_closed() {
        let dir = tempdir().expect("tempdir");
        let conn = SharedConn::new(&dir.path().join("store.duckdb"));

        conn.lock()
            .expect("open")
            .execute_batch("CREATE TEMP TABLE probe_tmp (x INTEGER)")
            .expect("temp table");
        thread::sleep(IDLE_CLOSE * 4);
        assert!(
            !temp_table_visible(&conn),
            "after sitting idle the connection was closed and reopened"
        );
    }

    const HOLD_DB_ENV: &str = "OPENLORE_TEST_HOLD_DB";
    const HOLD_MS_ENV: &str = "OPENLORE_TEST_HOLD_MS";

    /// Not a test on its own: the child process the cross-process tests
    /// spawn. It opens the store, writes `<db>.held`, holds the file for
    /// `HOLD_MS`, then exits (closing it).
    #[test]
    #[ignore = "child process for the cross-process tests"]
    fn hold_the_store_as_another_process() {
        let (Ok(db), Ok(ms)) = (std::env::var(HOLD_DB_ENV), std::env::var(HOLD_MS_ENV)) else {
            return;
        };
        let db = PathBuf::from(db);
        let store = SharedConn::new(&db);
        let _held = store.lock().expect("child opens the store");
        std::fs::write(db.with_extension("held"), b"").expect("signal held");
        thread::sleep(Duration::from_millis(ms.parse().expect("hold ms")));
    }

    /// Start another process holding `db` for `ms`; return once it holds it.
    fn another_process_holds(db: &Path, ms: u64) -> std::process::Child {
        let child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "conn::tests::hold_the_store_as_another_process",
                "--ignored",
                "--nocapture",
            ])
            .env(HOLD_DB_ENV, db)
            .env(HOLD_MS_ENV, ms.to_string())
            .spawn()
            .expect("spawn holder");
        let started = Instant::now();
        while !db.with_extension("held").exists() {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "holder never opened"
            );
            thread::sleep(Duration::from_millis(10));
        }
        child
    }

    #[test]
    fn a_handle_waits_for_another_process_then_proceeds() {
        let dir = tempdir().expect("tempdir");
        let db = dir.path().join("store.duckdb");
        let mut holder = another_process_holds(&db, 300);

        let started = Instant::now();
        SharedConn::new(&db)
            .lock()
            .expect("opens once the other process releases the file");
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "the open must have waited for the holder"
        );
        holder.wait().expect("holder exits");
    }

    #[test]
    fn a_handle_gives_up_as_busy_after_the_timeout() {
        let dir = tempdir().expect("tempdir");
        let db = dir.path().join("store.duckdb");
        let mut holder = another_process_holds(&db, 2_000);

        let outcome = SharedConn::with_busy_timeout(&db, Duration::from_millis(50))
            .lock()
            .map(|_| ());
        assert!(
            matches!(outcome, Err(ConnError::Busy { .. })),
            "expected Busy, got {outcome:?}"
        );
        holder.wait().expect("holder exits");
    }
}
