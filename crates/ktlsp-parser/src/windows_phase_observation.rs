//! Private, synthetic-only Windows investigation; never part of the worker protocol.
use crate::{ParserError, TIMEOUT};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const SOURCE: &str = "class Box {}\nfun f(p: Long) { val x = 1; val y = x }\n";
const REQUEST_CAP: u64 = 16;
const PACKET_CAP: usize = 32;
const RECEIVE_CAP: usize = 64;
const FILE_CAP: usize = 32 * 1024;
static REQUESTS: AtomicU64 = AtomicU64::new(0);

fn ns(origin: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(origin)
        .as_nanos()
        .min(u64::MAX as u128) as u64
}
fn wall() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct IoResult {
    ok: bool,
    raw_os_error: Option<i32>,
}
impl IoResult {
    fn from<T>(result: &io::Result<T>) -> Self {
        Self {
            ok: result.is_ok(),
            raw_os_error: result.as_ref().err().and_then(|e| e.raw_os_error()),
        }
    }
}

#[derive(Default)]
pub(super) struct IoProgress {
    started: AtomicBool,
    start_ns: AtomicU64,
    return_ns: AtomicU64,
    bytes: AtomicU64,
    done: AtomicU8,
    operation_return_ns: AtomicU64,
    operation_done: AtomicU8,
    read_zero_observed: AtomicBool,
    has_os_error: AtomicBool,
    os_error: AtomicI32,
}
#[derive(Serialize)]
struct IoSample {
    sample_begin_ns: u64,
    sample_end_ns: u64,
    started: bool,
    start_ns: Option<u64>,
    accepted_or_read_bytes: u64,
    returned: bool,
    return_ns: Option<u64>,
    error: Option<bool>,
    raw_os_error: Option<i32>,
    operation_returned: bool,
    operation_return_ns: Option<u64>,
    operation_error: Option<bool>,
    read_zero_observed: bool,
}
impl IoProgress {
    pub(super) fn start(&self, origin: Instant) {
        self.start_ns
            .store(ns(origin, Instant::now()), Ordering::Relaxed);
        self.started.store(true, Ordering::Release);
    }
    fn operation_return(&self, origin: Instant, error: bool, raw_os_error: Option<i32>) {
        if let Some(code) = raw_os_error {
            self.os_error.store(code, Ordering::Relaxed);
            self.has_os_error.store(true, Ordering::Relaxed);
        }
        self.operation_return_ns
            .store(ns(origin, Instant::now()), Ordering::Relaxed);
        self.operation_done
            .store(if error { 2 } else { 1 }, Ordering::Release);
    }
    pub(super) fn operation_finished<T>(&self, origin: Instant, result: &io::Result<T>) {
        self.operation_return(
            origin,
            result.is_err(),
            result.as_ref().err().and_then(|error| error.raw_os_error()),
        );
    }
    pub(super) fn finish(&self, origin: Instant, error: bool, raw_os_error: Option<i32>) {
        if self.operation_done.load(Ordering::Relaxed) == 0 {
            self.operation_return(origin, error, raw_os_error);
        }
        self.return_ns
            .store(ns(origin, Instant::now()), Ordering::Relaxed);
        self.done
            .store(if error { 2 } else { 1 }, Ordering::Release);
    }
    fn sample(&self, origin: Instant) -> IoSample {
        let begin = ns(origin, Instant::now());
        let done = self.done.load(Ordering::Acquire);
        let started = self.started.load(Ordering::Acquire);
        let operation_done = self.operation_done.load(Ordering::Acquire);
        let sample = IoSample {
            sample_begin_ns: begin,
            sample_end_ns: 0,
            started,
            start_ns: started.then(|| self.start_ns.load(Ordering::Relaxed)),
            accepted_or_read_bytes: self.bytes.load(Ordering::Relaxed),
            returned: done != 0,
            return_ns: (done != 0).then(|| self.return_ns.load(Ordering::Relaxed)),
            error: (done != 0).then_some(done == 2),
            raw_os_error: (operation_done != 0 && self.has_os_error.load(Ordering::Relaxed))
                .then(|| self.os_error.load(Ordering::Relaxed)),
            operation_returned: operation_done != 0,
            operation_return_ns: (operation_done != 0)
                .then(|| self.operation_return_ns.load(Ordering::Relaxed)),
            operation_error: (operation_done != 0).then_some(operation_done == 2),
            read_zero_observed: self.read_zero_observed.load(Ordering::Acquire),
        };
        IoSample {
            sample_end_ns: ns(origin, Instant::now()),
            ..sample
        }
    }
}

pub(super) struct CountIo<'a, T>(pub T, pub &'a IoProgress);
impl<T: Write> Write for CountIo<'_, T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.0.write(bytes);
        if let Ok(count) = result {
            self.1.bytes.fetch_add(count as u64, Ordering::Relaxed);
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl<T: Read> Read for CountIo<'_, T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let result = self.0.read(bytes);
        if let Ok(count) = result {
            self.1.bytes.fetch_add(count as u64, Ordering::Relaxed);
            if count == 0 && !bytes.is_empty() {
                self.1.read_zero_observed.store(true, Ordering::Release);
            }
        }
        result
    }
}

#[derive(Default, Serialize)]
struct Times {
    setup_begin_ns: u64,
    setup_end_ns: u64,
    spawn_begin_ns: Option<u64>,
    spawn_end_ns: Option<u64>,
    cleanup_begin_ns: Option<u64>,
    cleanup_end_ns: Option<u64>,
    guard_drop_begin_ns: Option<u64>,
    guard_drop_end_ns: Option<u64>,
    scope_return_ns: Option<u64>,
    return_boundary_ns: Option<u64>,
}
#[derive(Clone, Copy, Serialize)]
struct Outcome {
    at_ns: u64,
    site: &'static str,
    error: Option<&'static str>,
}
fn error_name(error: ParserError) -> &'static str {
    match error {
        ParserError::InputLimit => "InputLimit",
        ParserError::NestingLimit => "NestingLimit",
        ParserError::OutputLimit => "OutputLimit",
        ParserError::AllocationLimit => "AllocationLimit",
        ParserError::Coverage => "Coverage",
        ParserError::InvalidTree => "InvalidTree",
        ParserError::Cancelled => "Cancelled",
        ParserError::Timeout => "Timeout",
        ParserError::PipedTimeout(crate::PipedTimeoutSite::SupervisorPoll) => {
            "PipedTimeout(SupervisorPoll)"
        }
        ParserError::PipedTimeout(crate::PipedTimeoutSite::WriterJoin) => {
            "PipedTimeout(WriterJoin)"
        }
        ParserError::PipedTimeout(crate::PipedTimeoutSite::ReaderJoin) => {
            "PipedTimeout(ReaderJoin)"
        }
        ParserError::WorkerFailed => "WorkerFailed",
        ParserError::Io => "Io",
        ParserError::StaleSnapshot => "StaleSnapshot",
    }
}
#[derive(Serialize)]
struct Status {
    at_ns: u64,
    site: &'static str,
    success: bool,
    code: Option<i32>,
}
#[derive(Default, Serialize)]
struct CleanupAttempt {
    kill_begin_ns: Option<u64>,
    kill_end_ns: Option<u64>,
    kill: Option<IoResult>,
    wait_begin_ns: Option<u64>,
    wait_end_ns: Option<u64>,
    wait: Option<IoResult>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    v: u8,
    token: u64,
    pid: u32,
    seq: u8,
    stage: u8,
    elapsed_ns: u64,
    #[serde(deserialize_with = "required_wall")]
    unix_ns: Option<u128>,
    detail: u64,
    failed_before: u8,
}
fn required_wall<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u128>, D::Error> {
    Option::<u128>::deserialize(d)
}
#[derive(Serialize)]
struct Event {
    v: u8,
    token_redacted: bool,
    token_and_spawned_pid_match: bool,
    pid: u32,
    seq: u8,
    stage: u8,
    elapsed_ns: u64,
    unix_ns: Option<u128>,
    detail: u64,
    failed_before: u8,
    receipt_after_cleanup_ns: u64,
}
#[derive(Default, Serialize)]
struct WorkerRecords {
    records: [Option<Event>; PACKET_CAP],
    received: u32,
    retained: u32,
    invalid: u32,
    duplicate_sequence: u32,
    duplicate_stage: u32,
    overflow: u32,
    receive_error: Option<IoResult>,
    receive_budget_exhausted: bool,
    queue_observed_empty: bool,
    sequence_gap_mask: u32,
    absent_stage_mask: u32,
    before_exit_observed: bool,
    tail_unknown: bool,
}

#[derive(Serialize)]
pub(super) struct Observation {
    format_version: u8,
    server_pid: u32,
    request_seq: u64,
    request_cap: u64,
    request_cap_reached: bool,
    worker_pid: Option<u32>,
    selected_normalized_source: &'static str,
    parent_setup_unix_ns: Option<u128>,
    parent_return_unix_ns: Option<u128>,
    times: Times,
    setup_error: Option<&'static str>,
    setup_io_error: Option<IoResult>,
    spawn: Option<IoResult>,
    deadline_check_count: u64,
    first_deadline_check_ns: Option<u64>,
    last_deadline_check_ns: Option<u64>,
    first_deadline_check_site: Option<&'static str>,
    last_deadline_check_site: Option<&'static str>,
    child_try_wait_count: u64,
    child_pending_observations: u64,
    first_child_poll_begin_ns: Option<u64>,
    first_child_poll_end_ns: Option<u64>,
    last_child_poll_begin_ns: Option<u64>,
    last_child_poll_end_ns: Option<u64>,
    child_poll_error: Option<IoResult>,
    observed_statuses: [Option<Status>; 3],
    statuses_truncated: bool,
    precleanup_selection: Option<Outcome>,
    returned_supervisor_outcome: Option<Outcome>,
    writer_at_selection: Option<IoSample>,
    reader_at_selection: Option<IoSample>,
    writer_after_scope: Option<IoSample>,
    reader_after_scope: Option<IoSample>,
    cleanup_confirmed: bool,
    scoped_io_joined: bool,
    finish_attempt: CleanupAttempt,
    drop_attempt: CleanupAttempt,
    worker: WorkerRecords,
    limits: [(&'static str, usize); 4],
    limitations: [&'static str; 11],
    #[serde(skip)]
    origin: Instant,
    #[serde(skip)]
    dir: PathBuf,
    #[serde(skip)]
    socket: Option<UdpSocket>,
    #[serde(skip)]
    token: u64,
    #[serde(skip)]
    site: &'static str,
    #[serde(rename = "cleanup_error_selection")]
    cleanup_override: Option<Outcome>,
}
impl Observation {
    pub(super) fn requested(
        command: &mut Command,
        source: &str,
        deadline: Instant,
    ) -> Option<Self> {
        if source != SOURCE
            || Path::new(command.get_program()).file_name()? != "ktlsp-parser-worker.exe"
            || command.get_args().count() != 1
            || command.get_args().next()? != "--source"
            || std::env::var_os("KTLSP_WINDOWS_PHASE_CAPTURE_ENABLE")?.as_os_str() != "1"
        {
            return None;
        }
        let origin = deadline - TIMEOUT;
        let setup_begin_ns = ns(origin, Instant::now());
        let dir = PathBuf::from(std::env::var_os("KTLSP_WINDOWS_PHASE_CAPTURE_DIR")?);
        if dir.as_os_str().len() > 4096 || !dir.is_absolute() || !dir.is_dir() {
            return None;
        }
        let mut current = REQUESTS.load(Ordering::Relaxed);
        let request_seq = loop {
            if current >= REQUEST_CAP {
                return None;
            }
            match REQUESTS.compare_exchange(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break current + 1,
                Err(actual) => current = actual,
            }
        };
        let server_pid = std::process::id();
        let token = RandomState::new().hash_one((server_pid, request_seq));
        let mut observation = Self {
            format_version: 1,
            server_pid,
            request_seq,
            request_cap: REQUEST_CAP,
            request_cap_reached: request_seq == REQUEST_CAP,
            worker_pid: None,
            selected_normalized_source: SOURCE,
            parent_setup_unix_ns: wall(),
            parent_return_unix_ns: None,
            times: Times {
                setup_begin_ns,
                ..Times::default()
            },
            setup_error: None,
            setup_io_error: None,
            spawn: None,
            deadline_check_count: 0,
            first_deadline_check_ns: None,
            last_deadline_check_ns: None,
            first_deadline_check_site: None,
            last_deadline_check_site: None,
            child_try_wait_count: 0,
            child_pending_observations: 0,
            first_child_poll_begin_ns: None,
            first_child_poll_end_ns: None,
            last_child_poll_begin_ns: None,
            last_child_poll_end_ns: None,
            child_poll_error: None,
            observed_statuses: [None, None, None],
            statuses_truncated: false,
            precleanup_selection: None,
            returned_supervisor_outcome: None,
            writer_at_selection: None,
            reader_at_selection: None,
            writer_after_scope: None,
            reader_after_scope: None,
            cleanup_confirmed: false,
            scoped_io_joined: false,
            finish_attempt: CleanupAttempt::default(),
            drop_attempt: CleanupAttempt::default(),
            worker: WorkerRecords::default(),
            limits: [
                ("worker_datagram_bytes", 256),
                ("retained_worker_records", PACKET_CAP),
                ("receive_attempts", RECEIVE_CAP),
                ("parent_file_bytes", FILE_CAP),
            ],
            limitations: [
                "Opt-in synthetic request only; at most 16 attempts per server process; later attempts have no record.",
                "Observation perturbs timing, scheduling, allocation and socket/pipe behavior; overhead is not subtracted.",
                "Parent ns uses the original deadline minus TIMEOUT; u64 nanoseconds saturate at u64::MAX.",
                "Worker elapsed_ns is worker-local, not the parent clock; wall-clock ordering requires stable synchronized clocks.",
                "UDP is lossy; absent phases/final packet do not prove a phase was not reached or a kill cause.",
                "Worker receipts are after scoped cleanup/joins, not emit times; no predeadline readiness query is added.",
                "I/O fields are sampled loads, not one atomic state; operation_return marks read/write completion, return is closure work (reader after channel-send attempt), not thread exit/input-close timing; read_zero marks a nonempty read returning zero.",
                "Statuses come only from original try_wait/wait calls; no OS-cause attribution or postselection readiness probe.",
                "Outcome covers piped supervision only; later parser decoding/validation/cancellation is not observed.",
                "Nonce is redacted; not a raw datagram archive. Missing/partial files can reflect persistence failure or hanging cleanup.",
                "return_boundary_ns is before bounded serialization/persistence, not the instant the caller resumes; scope_return_ns is absent if spawn failed.",
            ],
            origin,
            dir,
            socket: None,
            token,
            site: "capture_setup",
            cleanup_override: None,
        };
        let setup = (|| {
            let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
            socket.set_nonblocking(true)?;
            let address = socket.local_addr()?;
            command
                .env("KTLSP_WINDOWS_PHASE_ADDR", address.to_string())
                .env("KTLSP_WINDOWS_PHASE_TOKEN", token.to_string());
            observation.socket = Some(socket);
            Ok::<_, io::Error>(())
        })();
        if let Err(error) = setup {
            observation.setup_error = Some("loopback_channel_setup");
            observation.setup_io_error = Some(IoResult::from::<()>(&Err(error)));
        }
        observation.times.setup_end_ns = ns(origin, Instant::now());
        Some(observation)
    }
    pub(super) fn origin(&self) -> Instant {
        self.origin
    }
    pub(super) fn site(&mut self, site: &'static str) {
        self.site = site;
    }
    pub(super) fn deadline_check(&mut self, now: Instant, site: &'static str) {
        let at = ns(self.origin, now);
        self.deadline_check_count = self.deadline_check_count.saturating_add(1);
        if self.first_deadline_check_ns.is_none() {
            self.first_deadline_check_ns = Some(at);
            self.first_deadline_check_site = Some(site);
        }
        self.last_deadline_check_ns = Some(at);
        self.last_deadline_check_site = Some(site);
        self.site = site;
    }
    pub(super) fn spawn_begin(&mut self) {
        self.times.spawn_begin_ns = Some(ns(self.origin, Instant::now()));
    }
    pub(super) fn spawn_end<T>(&mut self, result: &io::Result<T>, pid: Option<u32>) {
        self.times.spawn_end_ns = Some(ns(self.origin, Instant::now()));
        self.spawn = Some(IoResult::from(result));
        self.worker_pid = pid;
        self.site = "spawn";
    }
    fn status(&mut self, status: &ExitStatus, site: &'static str) {
        let entry = Status {
            at_ns: ns(self.origin, Instant::now()),
            site,
            success: status.success(),
            code: status.code(),
        };
        if let Some(slot) = self.observed_statuses.iter_mut().find(|s| s.is_none()) {
            *slot = Some(entry);
        } else {
            self.statuses_truncated = true;
        }
    }
    pub(super) fn poll_begin(&mut self) {
        let at = ns(self.origin, Instant::now());
        self.first_child_poll_begin_ns.get_or_insert(at);
        self.last_child_poll_begin_ns = Some(at);
        self.site = "supervisor.child_try_wait";
    }
    pub(super) fn poll(&mut self, result: &io::Result<Option<ExitStatus>>) {
        self.child_try_wait_count = self.child_try_wait_count.saturating_add(1);
        let at = ns(self.origin, Instant::now());
        self.first_child_poll_end_ns.get_or_insert(at);
        self.last_child_poll_end_ns = Some(at);
        match result {
            Ok(Some(status)) => self.status(status, "supervisor.try_wait"),
            Ok(None) => {
                self.child_pending_observations = self.child_pending_observations.saturating_add(1)
            }
            Err(_) => self.child_poll_error = Some(IoResult::from(result)),
        }
    }
    pub(super) fn selected<T>(
        &mut self,
        result: &Result<T, ParserError>,
        writer: &IoProgress,
        reader: &IoProgress,
    ) {
        self.precleanup_selection = Some(Outcome {
            at_ns: ns(self.origin, Instant::now()),
            site: self.site,
            error: result.as_ref().err().copied().map(error_name),
        });
        self.writer_at_selection = Some(writer.sample(self.origin));
        self.reader_at_selection = Some(reader.sample(self.origin));
    }
    pub(super) fn cleanup_begin(&mut self) {
        self.times.cleanup_begin_ns = Some(ns(self.origin, Instant::now()));
    }
    pub(super) fn cleanup_end(&mut self, confirmed: bool) {
        self.times.cleanup_end_ns = Some(ns(self.origin, Instant::now()));
        self.cleanup_confirmed = confirmed;
    }
    pub(super) fn drop_begin(&mut self) {
        self.times.guard_drop_begin_ns = Some(ns(self.origin, Instant::now()));
    }
    pub(super) fn drop_end(&mut self) {
        self.times.guard_drop_end_ns = Some(ns(self.origin, Instant::now()));
    }
    pub(super) fn kill_begin(&mut self, dropping: bool) {
        let at = ns(self.origin, Instant::now());
        let attempt = if dropping {
            &mut self.drop_attempt
        } else {
            &mut self.finish_attempt
        };
        attempt.kill_begin_ns = Some(at);
    }
    pub(super) fn kill_end(&mut self, dropping: bool, result: &io::Result<()>) {
        let at = ns(self.origin, Instant::now());
        let attempt = if dropping {
            &mut self.drop_attempt
        } else {
            &mut self.finish_attempt
        };
        attempt.kill_end_ns = Some(at);
        attempt.kill = Some(IoResult::from(result));
    }
    pub(super) fn wait_begin(&mut self, dropping: bool) {
        let at = ns(self.origin, Instant::now());
        let attempt = if dropping {
            &mut self.drop_attempt
        } else {
            &mut self.finish_attempt
        };
        attempt.wait_begin_ns = Some(at);
    }
    pub(super) fn wait_end(&mut self, dropping: bool, result: &io::Result<ExitStatus>) {
        let at = ns(self.origin, Instant::now());
        let attempt = if dropping {
            &mut self.drop_attempt
        } else {
            &mut self.finish_attempt
        };
        attempt.wait_end_ns = Some(at);
        attempt.wait = Some(IoResult::from(result));
        if let Ok(status) = result {
            self.status(
                status,
                if dropping {
                    "guard.drop.wait"
                } else {
                    "guard.finish.wait"
                },
            );
        } else if !dropping {
            self.cleanup_override = Some(Outcome {
                at_ns: at,
                site: "guard.finish.wait",
                error: Some("Io"),
            });
        }
    }
    pub(super) fn after_scope<T>(
        &mut self,
        result: &Result<T, ParserError>,
        writer: &IoProgress,
        reader: &IoProgress,
        scope_joined: bool,
    ) {
        self.scoped_io_joined = scope_joined;
        self.times.scope_return_ns = scope_joined.then(|| ns(self.origin, Instant::now()));
        self.writer_after_scope = Some(writer.sample(self.origin));
        self.reader_after_scope = Some(reader.sample(self.origin));
        self.returned_supervisor_outcome = Some(Outcome {
            at_ns: ns(self.origin, Instant::now()),
            site: self
                .cleanup_override
                .or(self.precleanup_selection)
                .map_or("unknown", |o| o.site),
            error: result.as_ref().err().copied().map(error_name),
        });
    }
    fn drain(&mut self) {
        let Some(socket) = self.socket.as_ref() else {
            self.worker.absent_stage_mask = (1 << 18) - 1;
            self.worker.tail_unknown = true;
            return;
        };
        let mut bytes = [0u8; 257];
        let mut sequences = 0u32;
        let mut stages = 0u32;
        let mut max_seq = None::<u8>;
        for attempt in 0..RECEIVE_CAP {
            if attempt + 1 == RECEIVE_CAP {
                self.worker.receive_budget_exhausted = true;
            }
            match socket.recv_from(&mut bytes) {
                Ok((count, peer)) => {
                    self.worker.received += 1;
                    let packet = (count <= 256
                        && matches!(peer, SocketAddr::V4(p) if p.ip().is_loopback()))
                    .then(|| serde_json::from_slice::<Packet>(&bytes[..count]).ok())
                    .flatten();
                    let Some(packet) = packet else {
                        self.worker.invalid += 1;
                        continue;
                    };
                    if packet.v != 1
                        || packet.token != self.token
                        || Some(packet.pid) != self.worker_pid
                        || packet.seq >= 32
                        || packet.stage > 17
                        || packet.failed_before > packet.seq
                    {
                        self.worker.invalid += 1;
                        continue;
                    }
                    let seq_bit = 1u32 << packet.seq;
                    let stage_bit = 1u32 << packet.stage;
                    if sequences & seq_bit != 0 {
                        self.worker.duplicate_sequence += 1;
                        continue;
                    }
                    if stages & stage_bit != 0 {
                        self.worker.duplicate_stage += 1;
                    }
                    sequences |= seq_bit;
                    stages |= stage_bit;
                    max_seq = Some(max_seq.map_or(packet.seq, |seq| seq.max(packet.seq)));
                    self.worker.before_exit_observed |= packet.stage == 17;
                    if let Some(slot) = self.worker.records.iter_mut().find(|event| event.is_none())
                    {
                        *slot = Some(Event {
                            v: packet.v,
                            token_redacted: true,
                            token_and_spawned_pid_match: true,
                            pid: packet.pid,
                            seq: packet.seq,
                            stage: packet.stage,
                            elapsed_ns: packet.elapsed_ns,
                            unix_ns: packet.unix_ns,
                            detail: packet.detail,
                            failed_before: packet.failed_before,
                            receipt_after_cleanup_ns: ns(self.origin, Instant::now()),
                        });
                        self.worker.retained += 1;
                    } else {
                        self.worker.overflow += 1;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.worker.queue_observed_empty = true;
                    break;
                }
                Err(error) => {
                    // On Windows an oversized UDP datagram can report WSAEMSGSIZE.
                    self.worker.receive_error = Some(IoResult::from::<()>(&Err(error)));
                    break;
                }
            }
        }
        self.worker.sequence_gap_mask =
            max_seq.map_or(0, |seq| (u32::MAX >> (31 - seq)) & !sequences);
        self.worker.absent_stage_mask = ((1 << 18) - 1) & !stages;
        self.worker.tail_unknown = !self.worker.before_exit_observed
            || !self.worker.queue_observed_empty
            || self.worker.invalid != 0
            || self.worker.duplicate_sequence != 0
            || self.worker.duplicate_stage != 0
            || self.worker.sequence_gap_mask != 0;
    }
    pub(super) fn persist(mut self) {
        self.drain();
        self.parent_return_unix_ns = wall();
        self.times.return_boundary_ns = Some(ns(self.origin, Instant::now()));
        let mut bytes = [0u8; FILE_CAP];
        let mut out = &mut bytes[..];
        if serde_json::to_writer(&mut out, &self).is_err() {
            return;
        }
        let count = FILE_CAP - out.len();
        let name = format!("parent-{}-{}.json", self.server_pid, self.request_seq);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.dir.join(name))
        {
            let _ = file.write_all(&bytes[..count]).and_then(|_| file.flush());
        }
    }
}
