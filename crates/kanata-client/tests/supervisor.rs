//! Process tests for the launcher and supervisor, against `fake-kanata.sh`
//! (no keyboard, no permissions; constitution V).

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use kanata_client::launcher::{self, LaunchOptions, terminate, with_parent_death_signal};
use kanata_client::{
    Failure, Probe, SessionState, Supervisor, SupervisorEvent, SupervisorOptions, probe,
};

fn fake() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fake-kanata.sh")
}

fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Fast timings so the tests take seconds, not minutes.
fn quick() -> SupervisorOptions {
    SupervisorOptions {
        ready_timeout: Duration::from_secs(3),
        backoff_unit: Duration::from_millis(100),
        stop_grace: Duration::from_millis(500),
    }
}

/// A supervisor running the fake with `env`, plus its events.
fn supervise(
    env: &[(&str, &str)],
    timings: SupervisorOptions,
) -> (Supervisor, Receiver<SupervisorEvent>) {
    let mut opts = LaunchOptions::new("/dev/null", launcher::free_port().unwrap());
    opts.kanata_bin = fake();
    // The fake reads its mode from the environment; the supervisor inherits ours.
    for (k, v) in env {
        // SAFETY: each test sets its own variables before spawning; tests that
        // use FAKE_* run in separate processes via `serial` groups below.
        unsafe { std::env::set_var(k, v) };
    }
    let (tx, rx) = mpsc::channel();
    let sup = Supervisor::start(opts, timings, move |e| {
        let _ = tx.send(e);
    });
    (sup, rx)
}

/// Wait for an event matching `pred`, failing after `within`.
fn expect(
    rx: &Receiver<SupervisorEvent>,
    within: Duration,
    pred: impl Fn(&SupervisorEvent) -> bool,
) -> SupervisorEvent {
    let deadline = Instant::now() + within;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(e) if pred(&e) => return e,
            Ok(_) => {}
            Err(_) => panic!("no matching event within {within:?}"),
        }
    }
}

fn state(s: SessionState) -> impl Fn(&SupervisorEvent) -> bool {
    move |e| *e == SupervisorEvent::State(s.clone())
}

// ── launcher ─────────────────────────────────────────────────────────────

#[test]
fn child_dies_with_its_spawning_thread() {
    let pid = thread::spawn(|| {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        with_parent_death_signal(&mut cmd);
        let child = cmd.spawn().unwrap();
        let pid = child.id();
        std::mem::forget(child); // the thread ends without waiting
        pid
    })
    .join()
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    // The child becomes a zombie of this process once killed; reap it.
    while Instant::now() < deadline {
        let mut status = 0;
        // SAFETY: waitpid on our own child pid.
        let r = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG) };
        if r == pid as libc::pid_t {
            assert!(libc::WIFSIGNALED(status) && libc::WTERMSIG(status) == libc::SIGTERM);
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!(
        "child {pid} outlived the thread that spawned it (alive: {})",
        alive(pid)
    );
}

#[test]
fn terminate_stops_within_grace() {
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let t = Instant::now();
    terminate(&mut child, Duration::from_millis(1500)).unwrap();
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn terminate_kills_a_child_that_ignores_sigterm() {
    let mut child = Command::new("sh")
        .args(["-c", "trap '' TERM; sleep 30"])
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(200));
    let t = Instant::now();
    let status = terminate(&mut child, Duration::from_millis(300)).unwrap();
    assert!(!status.success());
    assert!(t.elapsed() < Duration::from_secs(2));
}

// ── supervisor ───────────────────────────────────────────────────────────
//
// FAKE_* variables are process-wide, so all supervisor scenarios run in one
// test, one after the other.

#[test]
fn supervisor_scenarios() {
    starts_and_becomes_ready_then_stops();
    crash_restarts_with_backoff();
    gives_up_after_five();
    invalid_config_is_not_retried();
    permissions_are_not_retried();
    never_answer_fails();
    waits_for_a_keyboard();
    not_installed();
    attaches_when_another_kanata_wins_the_port();
}

fn clear_env() {
    for k in ["FAKE_CHECK", "FAKE_START", "FAKE_CRASH_AFTER"] {
        // SAFETY: single-threaded with respect to other env users in this test.
        unsafe { std::env::remove_var(k) };
    }
}

fn starts_and_becomes_ready_then_stops() {
    clear_env();
    let (sup, rx) = supervise(&[], quick());
    expect(&rx, Duration::from_secs(1), state(SessionState::Checking));
    let SupervisorEvent::Ready(addr) = expect(&rx, Duration::from_secs(5), |e| {
        matches!(e, SupervisorEvent::Ready(_))
    }) else {
        unreachable!()
    };
    expect(&rx, Duration::from_secs(1), state(SessionState::Running));
    assert_eq!(probe(addr, Duration::from_millis(500)), Probe::Kanata);
    assert!(
        sup.log().iter().any(|l| l == "[INFO] fake kanata ready"),
        "log keeps output, ANSI stripped: {:?}",
        sup.log()
    );

    let t = Instant::now();
    sup.stop();
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "stop took {:?}",
        t.elapsed()
    );
    assert_eq!(
        probe(addr, Duration::from_millis(300)),
        Probe::Nothing,
        "kanata gone after stop"
    );
}

fn crash_restarts_with_backoff() {
    clear_env();
    let (sup, rx) = supervise(&[("FAKE_CRASH_AFTER", "0.5")], quick());
    expect(&rx, Duration::from_secs(5), state(SessionState::Running));
    expect(&rx, Duration::from_secs(3), |e| {
        matches!(
            e,
            SupervisorEvent::State(SessionState::Restarting { attempt: 1, .. })
        )
    });
    expect(&rx, Duration::from_secs(5), state(SessionState::Running));
    drop(sup); // Drop stops it too.
}

fn gives_up_after_five() {
    clear_env();
    let (sup, rx) = supervise(&[("FAKE_CRASH_AFTER", "0.2")], quick());
    expect(&rx, Duration::from_secs(20), |e| {
        matches!(
            e,
            SupervisorEvent::State(SessionState::Failed(Failure::GaveUp(_)))
        )
    });
    // A user restart clears the history and tries again.
    clear_env();
    sup.restart();
    expect(&rx, Duration::from_secs(5), state(SessionState::Running));
    sup.stop();
}

fn invalid_config_is_not_retried() {
    clear_env();
    let (sup, rx) = supervise(&[("FAKE_CHECK", "fail")], quick());
    let e = expect(&rx, Duration::from_secs(3), |e| {
        matches!(e, SupervisorEvent::State(SessionState::Failed(_)))
    });
    assert_eq!(
        e,
        SupervisorEvent::State(SessionState::Failed(Failure::InvalidConfig {
            summary: "Unknown key/action: nosuchkey".into()
        }))
    );
    // No Starting afterwards: it never spawned Kanata.
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    sup.stop();
}

fn permissions_are_not_retried() {
    clear_env();
    let (sup, rx) = supervise(&[("FAKE_START", "permission")], quick());
    expect(
        &rx,
        Duration::from_secs(3),
        state(SessionState::Failed(Failure::Permissions)),
    );
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    sup.stop();
}

fn never_answer_fails() {
    clear_env();
    let timings = SupervisorOptions {
        ready_timeout: Duration::from_millis(800),
        ..quick()
    };
    let (sup, rx) = supervise(&[("FAKE_START", "never-answer")], timings);
    expect(&rx, Duration::from_secs(4), |e| {
        matches!(
            e,
            SupervisorEvent::State(SessionState::Restarting { attempt: 1, .. })
        )
    });
    sup.stop();
}

fn waits_for_a_keyboard() {
    clear_env();
    let timings = SupervisorOptions {
        ready_timeout: Duration::from_millis(500),
        ..quick()
    };
    let (sup, rx) = supervise(&[("FAKE_START", "no-keyboard")], timings);
    expect(
        &rx,
        Duration::from_secs(3),
        state(SessionState::WaitingForKeyboard),
    );
    // The paused timer means no NoAnswer restart even after several timeouts.
    thread::sleep(Duration::from_millis(1500));
    assert!(
        rx.try_iter()
            .all(|e| !matches!(e, SupervisorEvent::State(SessionState::Restarting { .. })))
    );
    sup.stop();
}

fn not_installed() {
    clear_env();
    let mut opts = LaunchOptions::new("/dev/null", 1);
    opts.kanata_bin = "/nonexistent/kanata".into();
    let (tx, rx) = mpsc::channel();
    let sup = Supervisor::start(opts, quick(), move |e| {
        let _ = tx.send(e);
    });
    expect(&rx, Duration::from_secs(2), |e| {
        matches!(
            e,
            SupervisorEvent::State(SessionState::Failed(Failure::NotInstalled { .. }))
        )
    });
    sup.stop();
}

fn attaches_when_another_kanata_wins_the_port() {
    clear_env();
    // Another overlay's Kanata (the fake) already owns the port.
    let port = launcher::free_port().unwrap();
    let mut other = Command::new(fake())
        .args(["--port", &format!("127.0.0.1:{port}")])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let deadline = Instant::now() + Duration::from_secs(3);
    while probe(addr, Duration::from_millis(200)) != Probe::Kanata {
        assert!(Instant::now() < deadline, "fake did not start");
    }

    let mut opts = LaunchOptions::new("/dev/null", port);
    opts.kanata_bin = fake();
    let (tx, rx) = mpsc::channel();
    let sup = Supervisor::start(opts, quick(), move |e| {
        let _ = tx.send(e);
    });
    let e = expect(&rx, Duration::from_secs(5), |e| {
        matches!(e, SupervisorEvent::Attach(_))
    });
    assert_eq!(e, SupervisorEvent::Attach(addr));
    sup.stop();
    terminate(&mut other, Duration::from_millis(500)).unwrap();
}
