use super::*;

use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

/// A fresh, short directory for this test's socket files (Unix socket paths
/// are length-limited).
fn scratch_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "olc-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Sets `stop` when dropped, so a failing test still ends the accept loop.
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[test]
fn each_unreachable_cause_reads_as_its_event_token() {
    assert_eq!(
        Unreachable::SocketNotConfigured.token(),
        "socket_not_configured"
    );
    assert_eq!(Unreachable::ConnectFailed.token(), "connect_failed");
    assert_eq!(Unreachable::ConnectionLost.token(), "connection_lost");
    assert_eq!(Unreachable::UnexpectedReply.token(), "unexpected_reply");
}

#[test]
fn a_blank_or_relative_socket_is_not_configured_and_a_missing_one_cannot_connect() {
    for socket in ["", "   ", "indexer.sock", "run/indexer.sock"] {
        assert_eq!(
            request_one_pass(socket),
            TriggerOutcome::Unreachable(Unreachable::SocketNotConfigured),
            "{socket:?}"
        );
    }
    let missing = scratch_dir().join("absent.sock");
    assert_eq!(
        request_one_pass(missing.to_str().expect("utf-8 path")),
        TriggerOutcome::Unreachable(Unreachable::ConnectFailed)
    );
}

#[test]
fn a_trigger_with_no_socket_exits_four() {
    assert_eq!(trigger(None), 4);
}

#[test]
fn bind_replaces_a_stale_socket_or_a_leftover_file() {
    let dir = scratch_dir();
    let socket = dir.join("c.sock");

    drop(UnixListener::bind(&socket).expect("first bind"));
    assert!(socket.exists(), "a dropped listener leaves its socket file");
    drop(bind(&socket).expect("a stale socket is replaced"));

    std::fs::remove_file(&socket).expect("clean");
    std::fs::write(&socket, b"leftover").expect("leftover file");
    let listener = bind(&socket).expect("a leftover regular file is replaced");
    let mode = std::fs::metadata(&socket)
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    drop(listener);
}

#[test]
fn removing_a_stale_socket_leaves_a_directory_and_tolerates_nothing_there() {
    let dir = scratch_dir();
    let sub = dir.join("d");
    std::fs::create_dir(&sub).expect("dir");
    assert!(remove_stale_socket(&sub).is_ok());
    assert!(sub.is_dir(), "a directory is never removed");

    assert!(remove_stale_socket(&dir.join("absent")).is_ok());

    let file = dir.join("f");
    std::fs::write(&file, b"x").expect("file");
    assert!(
        remove_stale_socket(&file.join("under-a-file")).is_err(),
        "an error other than not-found is reported"
    );
}

#[test]
fn a_probe_is_refused_by_a_socket_that_never_answers_ready() {
    let dir = scratch_dir();
    let socket = dir.join("mute.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let mute = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        drop(stream);
    });
    assert!(probe_round_trip(&socket).is_err());
    mute.join().expect("mute server");
}

#[test]
fn serve_answers_the_probe_and_a_trigger_gets_the_pass_s_own_exit_code() {
    let dir = scratch_dir();
    let socket = dir.join("serve.sock");
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let runner = Arc::new(PassRunner::spawn_scoped(scope, |_label| 3));
        let listener = bind(&socket).expect("bind");
        let _stop = StopOnDrop(&stop);
        let answering = Arc::clone(&runner);
        let stop_ref = &stop;
        scope.spawn(move || answer_triggers(&listener, &answering, stop_ref));

        probe_round_trip(&socket).expect("serve answers its own probe");
        assert_eq!(
            request_one_pass(socket.to_str().expect("utf-8 path")),
            TriggerOutcome::Completed { exit_code: 3 }
        );
    });
}
