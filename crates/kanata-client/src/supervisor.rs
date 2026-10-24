//! Keeps a managed Kanata running: `--check`, start, wait until it answers,
//! restart with backoff, stop (data-model › SessionState).
//!
//! Everything runs on one `kanata-supervisor` thread. That matters:
//! `PR_SET_PDEATHSIG` fires when the *thread* that forked the child exits, so
//! every Kanata (including `--check` runs and restarts) is spawned here, and
//! this thread lives until [`Supervisor::stop`] (research R1).
//!
//! Kanata's output goes into an in-memory ring only; it is never logged or
//! written anywhere (constitution VI).

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::net::SocketAddr;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::backoff::{Backoff, Next};
use crate::failure::{self, Failure, strip_ansi};
use crate::launcher::{self, LaunchOptions};
use crate::probe::{Probe, probe};

/// Lines of Kanata output kept in memory ("at least the last 500", FR-010).
const LOG_LINES: usize = 1000;
const POLL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Checking,
    Starting,
    /// Kanata found no keyboard yet; the readiness timer is paused.
    WaitingForKeyboard,
    Running,
    Restarting {
        attempt: u32,
        in_secs: u64,
    },
    Failed(Failure),
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorEvent {
    State(SessionState),
    /// Kanata answers here; connect to this address.
    Ready(SocketAddr),
    /// Another Kanata took the port first (e.g. a second overlay); supervision
    /// has ended and the app should attach to it.
    Attach(SocketAddr),
    /// New output in [`Supervisor::log`].
    LogChanged,
}

/// Timings, adjustable for tests.
#[derive(Debug, Clone)]
pub struct SupervisorOptions {
    /// "If it does not answer within 10 seconds, the start has failed" (FR-004).
    pub ready_timeout: Duration,
    /// Backoff delays are multiples of this (1 s → 1, 2, 4 … 30 s, FR-006).
    pub backoff_unit: Duration,
    /// SIGTERM, then SIGKILL after this long.
    pub stop_grace: Duration,
    /// Retry a Kanata whose devices are missing this often (or when one appears).
    pub device_retry: Duration,
}

impl Default for SupervisorOptions {
    fn default() -> Self {
        Self {
            ready_timeout: Duration::from_secs(10),
            backoff_unit: Duration::from_secs(1),
            stop_grace: Duration::from_millis(1500),
            device_retry: Duration::from_secs(10),
        }
    }
}

enum Command {
    Restart,
    Stop,
}

type Log = Arc<Mutex<VecDeque<String>>>;
type Emit = Arc<dyn Fn(SupervisorEvent) + Send + Sync>;

/// A managed Kanata. Dropping it stops Kanata.
pub struct Supervisor {
    commands: Sender<Command>,
    thread: Option<JoinHandle<()>>,
    log: Log,
}

impl Supervisor {
    /// Start supervising. Returns at once; `on_event` runs on the supervisor
    /// thread (and on output reader threads, for `LogChanged`) and must be cheap.
    pub fn start(
        opts: LaunchOptions,
        timings: SupervisorOptions,
        on_event: impl Fn(SupervisorEvent) + Send + Sync + 'static,
    ) -> Self {
        let (commands, rx) = mpsc::channel();
        let log: Log = Arc::default();
        let emit: Emit = Arc::new(on_event);
        let thread = thread::Builder::new()
            .name("kanata-supervisor".into())
            .spawn({
                let log = log.clone();
                move || Run::new(opts, timings, log, rx, emit).run()
            })
            .expect("spawn kanata-supervisor thread");
        Self {
            commands,
            thread: Some(thread),
            log,
        }
    }

    /// Stop Kanata and start it again, clearing the failure history (FR-009).
    pub fn restart(&self) {
        let _ = self.commands.send(Command::Restart);
    }

    /// Stop Kanata (SIGTERM, then SIGKILL) and end supervision.
    pub fn stop(mut self) {
        self.shutdown();
    }

    /// The recent output, oldest first.
    pub fn log(&self) -> Vec<String> {
        self.log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn shutdown(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// How one stage of the run loop ended.
enum Outcome {
    Stop,
    Restart,
    Failed(Failure),
}

struct Run {
    opts: LaunchOptions,
    timings: SupervisorOptions,
    log: Log,
    rx: Receiver<Command>,
    emit: Emit,
    backoff: Backoff,
    configured_port: u16,
    tried_free_port: bool,
}

impl Run {
    fn new(
        opts: LaunchOptions,
        timings: SupervisorOptions,
        log: Log,
        rx: Receiver<Command>,
        emit: Emit,
    ) -> Self {
        let backoff = Backoff::new(timings.backoff_unit);
        let configured_port = opts.port;
        Self {
            opts,
            timings,
            log,
            rx,
            emit,
            backoff,
            configured_port,
            tried_free_port: false,
        }
    }

    fn state(&self, s: SessionState) {
        (self.emit)(SupervisorEvent::State(s));
    }

    fn run(mut self) {
        let mut attempt = 0u32;
        loop {
            let failure = match self.check().and_then(|()| self.start_and_run()) {
                Ok(()) => unreachable!("start_and_run only returns on an outcome"),
                Err(Outcome::Stop) => break,
                Err(Outcome::Restart) => {
                    self.backoff.clear();
                    attempt = 0;
                    continue;
                }
                Err(Outcome::Failed(f)) => f,
            };

            if let Failure::PortInUse { port } = failure {
                let configured = launcher_addr(self.configured_port);
                if probe(configured, Duration::from_millis(500)) == Probe::Kanata {
                    (self.emit)(SupervisorEvent::Attach(configured));
                    break;
                }
                if !self.tried_free_port
                    && let Ok(free) = launcher::free_port()
                {
                    self.tried_free_port = true;
                    tracing::info!("port {port} is taken, starting Kanata on {free}");
                    self.opts.port = free;
                    continue;
                }
            }

            if failure == Failure::NoDevice {
                // Not a crash loop: wait for the device, without counting
                // towards giving up (feature 004).
                self.state(SessionState::WaitingForKeyboard);
                if self.wait_for_device() {
                    continue;
                }
                break;
            }

            if !failure.retryable() {
                self.state(SessionState::Failed(failure));
                if self.wait_for_restart() {
                    self.backoff.clear();
                    attempt = 0;
                    continue;
                }
                break;
            }
            match self.backoff.failure(Instant::now()) {
                Next::GiveUp => {
                    self.state(SessionState::Failed(Failure::GaveUp(Box::new(failure))));
                    if self.wait_for_restart() {
                        self.backoff.clear();
                        attempt = 0;
                        continue;
                    }
                    break;
                }
                Next::Retry(delay) => {
                    attempt += 1;
                    self.state(SessionState::Restarting {
                        attempt,
                        in_secs: delay.as_secs().max(1),
                    });
                    match self.rx.recv_timeout(delay) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(Command::Restart) => {
                            self.backoff.clear();
                            attempt = 0;
                        }
                        Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            }
        }
        self.state(SessionState::Stopped);
    }

    /// Wait up to `device_retry`, or until `/dev/input` changes; false on Stop.
    fn wait_for_device(&mut self) -> bool {
        let before = input_listing();
        let deadline = Instant::now() + self.timings.device_retry;
        while Instant::now() < deadline {
            let step = Duration::from_secs(1).min(self.timings.device_retry);
            match self.rx.recv_timeout(step) {
                Err(RecvTimeoutError::Timeout) => {}
                Ok(Command::Restart) => {
                    self.backoff.clear();
                    return true;
                }
                Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return false,
            }
            if input_listing() != before {
                return true;
            }
        }
        true
    }

    /// Failed state: wait for a user restart (true) or stop (false).
    fn wait_for_restart(&self) -> bool {
        matches!(self.rx.recv(), Ok(Command::Restart))
    }

    /// `kanata --check` (research R4).
    fn check(&self) -> Result<(), Outcome> {
        self.state(SessionState::Checking);
        let output = match self.opts.check_command().output() {
            Ok(o) => o,
            Err(e) => return Err(Outcome::Failed(spawn_failure(&self.opts, e))),
        };
        let lines: Vec<String> = [&output.stdout, &output.stderr]
            .iter()
            .flat_map(|bytes| {
                String::from_utf8_lossy(bytes)
                    .lines()
                    .map(strip_ansi)
                    .collect::<Vec<_>>()
            })
            .collect();
        self.push_log(lines.iter().cloned());
        match failure::classify_check(output.status.success(), &lines) {
            Some(f) => Err(Outcome::Failed(f)),
            None => Ok(()),
        }
    }

    /// Spawn, wait until ready, then run until something ends it.
    fn start_and_run(&mut self) -> Result<(), Outcome> {
        self.state(SessionState::Starting);
        let mut child = match self.opts.spawn() {
            Ok(c) => c,
            Err(crate::Error::Io(e)) => return Err(Outcome::Failed(spawn_failure(&self.opts, e))),
            Err(_) => return Err(Outcome::Failed(Failure::Exited { code: None })),
        };
        let log_start = self.log_len();
        self.capture(&mut child);
        let result = self
            .wait_ready(&mut child, log_start)
            .and_then(|()| self.watch(&mut child, log_start));
        let _ = launcher::terminate(&mut child, self.timings.stop_grace);
        result
    }

    fn wait_ready(&mut self, child: &mut Child, log_start: usize) -> Result<(), Outcome> {
        let addr = self.opts.address();
        let mut deadline = Instant::now() + self.timings.ready_timeout;
        let mut waiting_for_keyboard = false;
        loop {
            if let Some(status) = child.try_wait().ok().flatten() {
                // Let the reader threads catch the last lines before classifying.
                thread::sleep(Duration::from_millis(100));
                let output = self.log_since(log_start);
                return Err(Outcome::Failed(failure::classify_exit(
                    status.code(),
                    &output,
                    self.opts.port,
                )));
            }
            if probe(addr, Duration::from_millis(300)) == Probe::Kanata {
                self.backoff.running(Instant::now());
                (self.emit)(SupervisorEvent::Ready(addr));
                self.state(SessionState::Running);
                return Ok(());
            }
            let no_keyboard = self
                .log_since(log_start)
                .last()
                .is_some_and(|l| l.contains("no keyboard devices found"));
            if no_keyboard {
                if !waiting_for_keyboard {
                    self.state(SessionState::WaitingForKeyboard);
                    waiting_for_keyboard = true;
                }
                deadline = Instant::now() + self.timings.ready_timeout;
            } else if Instant::now() >= deadline {
                return Err(Outcome::Failed(Failure::NoAnswer));
            }
            self.poll_command()?;
        }
    }

    fn watch(&mut self, child: &mut Child, log_start: usize) -> Result<(), Outcome> {
        loop {
            if let Some(status) = child.try_wait().ok().flatten() {
                thread::sleep(Duration::from_millis(100));
                let output = self.log_since(log_start);
                return Err(Outcome::Failed(failure::classify_exit(
                    status.code(),
                    &output,
                    self.opts.port,
                )));
            }
            self.poll_command()?;
        }
    }

    /// Sleep one poll interval, returning early on a command.
    fn poll_command(&self) -> Result<(), Outcome> {
        match self.rx.recv_timeout(POLL) {
            Err(RecvTimeoutError::Timeout) => Ok(()),
            Ok(Command::Restart) => Err(Outcome::Restart),
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => Err(Outcome::Stop),
        }
    }

    /// Read the child's stdout and stderr into the log on helper threads.
    fn capture(&self, child: &mut Child) {
        let streams: [Option<Box<dyn Read + Send>>; 2] = [
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
        ];
        for stream in streams.into_iter().flatten() {
            let log = self.log.clone();
            let emit = self.emit.clone();
            thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    push(&log, std::iter::once(strip_ansi(&line)));
                    emit(SupervisorEvent::LogChanged);
                }
            });
        }
    }

    fn push_log(&self, lines: impl Iterator<Item = String>) {
        push(&self.log, lines);
        (self.emit)(SupervisorEvent::LogChanged);
    }

    fn log_len(&self) -> usize {
        self.log.lock().map(|l| l.len()).unwrap_or(0)
    }

    /// Lines added since `start` (best effort once the ring has wrapped).
    fn log_since(&self, start: usize) -> Vec<String> {
        self.log
            .lock()
            .map(|l| l.iter().skip(start.min(l.len())).cloned().collect())
            .unwrap_or_default()
    }
}

fn push(log: &Log, lines: impl Iterator<Item = String>) {
    if let Ok(mut log) = log.lock() {
        for line in lines {
            if log.len() == LOG_LINES {
                log.pop_front();
            }
            log.push_back(line);
        }
    }
}

/// The `event*` files in `/dev/input`, to notice a device being plugged in.
fn input_listing() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir("/dev/input")
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("event"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn launcher_addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn spawn_failure(opts: &LaunchOptions, e: std::io::Error) -> Failure {
    if e.kind() == ErrorKind::NotFound {
        Failure::NotInstalled {
            program: opts.kanata_bin.display().to_string(),
        }
    } else {
        Failure::Exited { code: None }
    }
}
