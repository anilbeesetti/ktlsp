//! Temporary exact-fixture macOS investigation, not a protocol or production fix.
use crate::{ParserError, TIMEOUT, admission::Cleanup};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const SOURCE: &str = include_str!("macos_phase_source.kt");
const RECORD_CAP: usize = 32;
const LINE_CAP: usize = 256;
const WORKER_CAP: usize = RECORD_CAP * LINE_CAP;
const SUMMARY_CAP: usize = 32768;
static REQUESTS: AtomicU64 = AtomicU64::new(0);

fn ns(origin: Instant, now: Instant) -> u64 {
    u64::try_from(now.saturating_duration_since(origin).as_nanos()).unwrap_or(u64::MAX)
}
fn wall() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
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
        ParserError::WorkerFailed => "WorkerFailed",
        ParserError::Io => "Io",
        ParserError::PipedTimeout(_) => "PipedTimeout",
        ParserError::StaleSnapshot => "StaleSnapshot",
    }
}
#[derive(Default, Serialize)]
pub(super) struct Operation {
    count: u64,
    first_begin_ns: Option<u64>,
    first_end_ns: Option<u64>,
    latest_begin_ns: Option<u64>,
    latest_end_ns: Option<u64>,
    max_duration_ns: u64,
    latest_result: Option<&'static str>,
    errors: u64,
    pending: u64,
    latest_raw_os_error: Option<i32>,
}
impl Operation {
    fn record(&mut self, origin: Instant, begin: Instant, result: &'static str) {
        let end = Instant::now();
        let begin_ns = ns(origin, begin);
        let end_ns = ns(origin, end);
        self.count = self.count.saturating_add(1);
        self.first_begin_ns.get_or_insert(begin_ns);
        self.first_end_ns.get_or_insert(end_ns);
        self.latest_begin_ns = Some(begin_ns);
        self.latest_end_ns = Some(end_ns);
        self.max_duration_ns = self.max_duration_ns.max(ns(begin, end));
        self.latest_result = Some(result);
    }
}
pub(super) struct TickState {
    pub(super) offset: usize,
    pub(super) bytes: usize,
    pub(super) reader: Option<Result<(), ParserError>>,
    pub(super) writer: Option<Result<(), ParserError>>,
    pub(super) halfclose: Option<Result<(), ParserError>>,
    pub(super) exited: bool,
}
#[derive(Serialize)]
pub(super) struct Progress {
    #[serde(skip)]
    origin: Instant,
    pub(super) read: Operation,
    pub(super) write: Operation,
    pub(super) halfclose: Operation,
    pub(super) status: Operation,
    // This times the original wait_ready call, not an extra poll or readiness query.
    #[serde(rename = "wait_ready_call")]
    pub(super) poll: Operation,
    tick: Operation,
    control: Operation,
    first_control_site: Option<&'static str>,
    latest_control_site: Option<&'static str>,
    pub(super) selection_site: &'static str,
    selection_ns: Option<u64>,
    last_input_offset: usize,
    last_output_bytes: usize,
    last_reader: &'static str,
    last_writer: &'static str,
    last_halfclose: &'static str,
    last_exited: bool,
    observed_exit_code: Option<i32>,
    observed_exit_success: Option<bool>,
    observed_exit_signal: Option<i32>,
}
impl Progress {
    fn new(origin: Instant) -> Self {
        Self {
            origin,
            read: Operation::default(),
            write: Operation::default(),
            halfclose: Operation::default(),
            status: Operation::default(),
            poll: Operation::default(),
            tick: Operation::default(),
            control: Operation::default(),
            first_control_site: None,
            latest_control_site: None,
            selection_site: "not_selected",
            selection_ns: None,
            last_input_offset: 0,
            last_output_bytes: 0,
            last_reader: "pending",
            last_writer: "pending",
            last_halfclose: "pending",
            last_exited: false,
            observed_exit_code: None,
            observed_exit_success: None,
            observed_exit_signal: None,
        }
    }
    pub(super) fn site(&mut self, site: &'static str) {
        self.selection_site = site;
    }
    pub(super) fn control(
        &mut self,
        site: &'static str,
        begin: Instant,
        result: &Result<(), ParserError>,
    ) {
        self.first_control_site.get_or_insert(site);
        self.latest_control_site = Some(site);
        self.control.record(
            self.origin,
            begin,
            result.as_ref().err().copied().map_or("ok", error_name),
        );
        if result.is_err() {
            self.selection_site = site;
            self.control.errors = self.control.errors.saturating_add(1);
        }
    }
    pub(super) fn io<T>(&mut self, operation: usize, begin: Instant, result: &io::Result<T>) {
        let op = match operation {
            0 => &mut self.read,
            1 => &mut self.write,
            2 => &mut self.halfclose,
            _ => &mut self.poll,
        };
        let name = match result {
            Ok(_) => "ok",
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => "would_block",
            Err(e) if e.kind() == io::ErrorKind::Interrupted => "interrupted",
            Err(_) => "error",
        };
        op.record(self.origin, begin, name);
        op.latest_raw_os_error = result.as_ref().err().and_then(io::Error::raw_os_error);
        if let Err(error) = result {
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) {
                op.pending = op.pending.saturating_add(1);
            } else {
                op.errors = op.errors.saturating_add(1);
            }
        }
    }
    pub(super) fn status(&mut self, begin: Instant, result: &io::Result<Option<ExitStatus>>) {
        use std::os::unix::process::ExitStatusExt;
        self.status.record(
            self.origin,
            begin,
            match result {
                Ok(Some(_)) => "exited",
                Ok(None) => "pending",
                Err(_) => "error",
            },
        );
        self.status.latest_raw_os_error = result.as_ref().err().and_then(io::Error::raw_os_error);
        match result {
            Ok(Some(status)) => {
                self.observed_exit_code = status.code();
                self.observed_exit_success = Some(status.success());
                self.observed_exit_signal = status.signal();
            }
            Ok(None) => self.status.pending = self.status.pending.saturating_add(1),
            Err(error) => {
                self.status.errors = self.status.errors.saturating_add(1);
                self.status.latest_raw_os_error = error.raw_os_error();
            }
        }
    }
    pub(super) fn tick(&mut self, begin: Instant, sample: TickState, decision: &'static str) {
        let TickState {
            offset,
            bytes,
            reader,
            writer,
            halfclose,
            exited,
        } = sample;
        fn state(value: Option<Result<(), ParserError>>) -> &'static str {
            value.map_or("pending", |r| r.err().map_or("ok", error_name))
        }
        self.tick.record(self.origin, begin, decision);
        self.last_input_offset = offset;
        self.last_output_bytes = bytes;
        self.last_reader = state(reader);
        self.last_writer = state(writer);
        self.last_halfclose = state(halfclose);
        self.last_exited = exited;
        if decision.starts_with("pending") {
            self.tick.pending = self.tick.pending.saturating_add(1);
        } else {
            self.selection_ns = Some(ns(self.origin, Instant::now()));
            if decision == "error" {
                self.tick.errors = self.tick.errors.saturating_add(1);
            }
        }
    }
}
#[derive(Default, Serialize)]
struct Times {
    setup_begin_ns: u64,
    socket_end_ns: Option<u64>,
    nonblocking_end_ns: Option<u64>,
    clone_end_ns: Option<u64>,
    env_end_ns: Option<u64>,
    spawn_begin_ns: Option<u64>,
    spawn_end_ns: Option<u64>,
    command_drop_end_ns: Option<u64>,
    stream_drop_begin_ns: Option<u64>,
    stream_drop_end_ns: Option<u64>,
    cleanup_begin_ns: Option<u64>,
    cleanup_end_ns: Option<u64>,
    arbitration_end_ns: Option<u64>,
    worker_read_begin_ns: Option<u64>,
    worker_read_end_ns: Option<u64>,
    summary_begin_ns: Option<u64>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkerLine {
    v: u8,
    capture_id: u64,
    server_pid: u32,
    request_seq: u64,
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
#[derive(Default, Serialize)]
struct WorkerRecords {
    records: [Option<WorkerLine>; RECORD_CAP],
    read_not_attempted: bool,
    missing_file: bool,
    open_error: Option<i32>,
    open_error_kind: Option<&'static str>,
    read_error: bool,
    read_error_raw: Option<i32>,
    read_bytes: usize,
    eof_observed: bool,
    over_limit: bool,
    complete_lines_observed: usize,
    lines_examined: usize,
    line_limit_exceeded: bool,
    partial_tail_bytes: usize,
    oversize_lines: u32,
    retained: usize,
    invalid_lines: u32,
    duplicate_sequence: u32,
    duplicate_stage: u32,
    sequence_gap_mask: u32,
    sequence_out_of_order: u32,
    absent_stage_mask: u32,
    prior_write_failures_reported: Option<u8>,
    failure_count_regressions: u32,
    before_exit_is_ok: Option<bool>,
    unix_clock_missing: u32,
    unix_clock_regressions: u32,
    elapsed_clock_regressions: u32,
    clock_limit_observed: bool,
    complete_successful_stream: bool,
    tail_unknown: bool,
}
#[derive(Serialize)]
pub(super) struct Observation {
    format_version: u8,
    server_pid: u32,
    request_seq: u64,
    capture_id: u64,
    worker_pid: Option<u32>,
    worker_file_relative: String,
    worker_file_configured: bool,
    request_cap: u64,
    requests_after_cap_not_recorded: bool,
    selected_fixture_bytes: usize,
    setup_unix_ns: Option<u128>,
    return_unix_ns: Option<u128>,
    times: Times,
    setup_error: Option<&'static str>,
    spawn_error: Option<&'static str>,
    progress: Option<Progress>,
    precleanup_outcome: Option<&'static str>,
    returned_outcome: Option<&'static str>,
    cleanup: Option<&'static str>,
    worker: WorkerRecords,
    limitations: [&'static str; 9],
    #[serde(skip)]
    origin: Instant,
    #[serde(skip)]
    dir: PathBuf,
    #[serde(skip)]
    worker_path: PathBuf,
}
impl Observation {
    pub(super) fn requested(command: &Command, source: &str, deadline: Instant) -> Option<Self> {
        // Public correlation is not authentication: ROOT explicitly supplies a trusted directory.
        if source != SOURCE
            || Path::new(command.get_program()).file_name()? != "ktlsp-parser-worker"
            || command.get_args().count() != 1
            || command.get_args().next()? != "--source"
            || !std::env::current_exe()
                .ok()?
                .file_name()?
                .to_str()?
                .starts_with("recovery_qualified_generic_receiver_functions-")
        {
            return None;
        }
        let origin = deadline - TIMEOUT;
        let setup_begin_ns = ns(origin, Instant::now());
        let dir = PathBuf::from(std::env::var_os("KTLSP_MACOS_PHASE_CAPTURE_DIR")?);
        if dir.as_os_str().as_encoded_bytes().len() > 4096 || !dir.is_absolute() || !dir.is_dir() {
            return None;
        }
        let mut current = REQUESTS.load(Ordering::Relaxed);
        let request_seq = loop {
            if current >= 4 {
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
        let capture_id = RandomState::new().hash_one((server_pid, request_seq));
        let worker_file_relative =
            format!("worker-{server_pid}-{request_seq}-{capture_id:016x}.jsonl");
        let worker_path = dir.join(&worker_file_relative);
        let worker_file_configured = worker_path.as_os_str().as_encoded_bytes().len() <= 4096;
        Some(Self {
            format_version: 2,
            server_pid,
            request_seq,
            capture_id,
            worker_pid: None,
            worker_file_relative,
            worker_file_configured,
            request_cap: 4,
            requests_after_cap_not_recorded: true,
            selected_fixture_bytes: SOURCE.len(),
            setup_unix_ns: wall(),
            return_unix_ns: None,
            times: Times {
                setup_begin_ns,
                ..Times::default()
            },
            setup_error: (!worker_file_configured).then_some("worker_file_path_limit"),
            spawn_error: None,
            progress: None,
            precleanup_outcome: None,
            returned_outcome: None,
            cleanup: None,
            worker: WorkerRecords {
                read_not_attempted: true,
                absent_stage_mask: (1 << 18) - 1,
                tail_unknown: true,
                ..WorkerRecords::default()
            },
            limitations: [
                "temporary observation only; never ship or merge this probe",
                "file I/O can block inside unchanged 500ms; intervals include observation cost",
                "complete_successful_stream means only ordered successful phase coverage, not allocator/process/semantic success",
                "kill/reap individual timing UNKNOWN except existing status-call observations; wait_ready timing is not raw poll result/revents; absent cleanup means original early-return implicit Drop was not observed",
                "CPU, scheduling, and original c419 failure cause UNKNOWN",
                "public correlation is not authentication; trusted private directory required",
                "worker elapsed_ns is worker-local, not the parent clock; wall ordering requires stable clocks; worker file reads occur after cleanup, not at emission",
                "no per-LSP-ID claim; ROOT correlates fixture assertions, logs and source separately",
                "raw worker JSONL retained separately; missing/partial data and unconfirmed cleanup never imply success",
            ],
            origin,
            dir,
            worker_path,
        })
    }
    pub(super) fn configure(&mut self, command: &mut Command) {
        if self.worker_file_configured {
            command.env("KTLSP_MACOS_PHASE_FILE", &self.worker_path);
        }
        self.mark("env");
    }
    pub(super) fn progress(&self) -> Progress {
        Progress::new(self.origin)
    }
    pub(super) fn mark(&mut self, site: &'static str) {
        let value = Some(ns(self.origin, Instant::now()));
        match site {
            "socket" => self.times.socket_end_ns = value,
            "nonblocking" => self.times.nonblocking_end_ns = value,
            "clone" => self.times.clone_end_ns = value,
            "env" => self.times.env_end_ns = value,
            "spawn.begin" => self.times.spawn_begin_ns = value,
            "spawn.end" => self.times.spawn_end_ns = value,
            "command.drop" => self.times.command_drop_end_ns = value,
            "stream.drop.begin" => self.times.stream_drop_begin_ns = value,
            "stream.drop.end" => self.times.stream_drop_end_ns = value,
            "cleanup.begin" => self.times.cleanup_begin_ns = value,
            "cleanup.end" => self.times.cleanup_end_ns = value,
            "arbitration" => self.times.arbitration_end_ns = value,
            _ => {}
        }
    }
    pub(super) fn spawned(&mut self, result: Result<(), ParserError>, pid: Option<u32>) {
        self.mark("spawn.end");
        self.spawn_error = result.err().map(error_name);
        self.worker_pid = pid;
    }
    pub(super) fn selected<T>(
        &mut self,
        result: &Result<T, ParserError>,
        mut progress: Option<Progress>,
    ) {
        self.precleanup_outcome = Some(result.as_ref().err().copied().map_or("ok", error_name));
        if let Some(progress) = &mut progress {
            progress
                .selection_ns
                .get_or_insert_with(|| ns(self.origin, Instant::now()));
        }
        self.progress = progress;
    }
    pub(super) fn setup_failed(mut self, site: &'static str, error: ParserError) {
        self.setup_error = Some(site);
        self.returned_outcome = Some(error_name(error));
        self.persist();
    }
    pub(super) fn finish<T>(mut self, result: &Result<T, ParserError>, cleanup: Option<Cleanup>) {
        self.returned_outcome = Some(result.as_ref().err().copied().map_or("ok", error_name));
        self.cleanup = cleanup.map(|c| match c {
            Cleanup::Confirmed => "Confirmed",
            Cleanup::Unconfirmed => "Unconfirmed",
            Cleanup::Failed => "Failed",
        });
        if cleanup.is_some() {
            self.mark("arbitration");
            self.read_worker_file();
        }
        self.persist();
    }
    fn read_worker_file(&mut self) {
        self.worker.absent_stage_mask = (1 << 18) - 1;
        self.worker.tail_unknown = true;
        if !self.worker_file_configured || self.worker_pid.is_none() {
            self.worker.read_not_attempted = true;
            return;
        }
        self.worker.read_not_attempted = false;
        self.times.worker_read_begin_ns = Some(ns(self.origin, Instant::now()));
        let result = self.decode_worker();
        self.times.worker_read_end_ns = Some(ns(self.origin, Instant::now()));
        if let Err(error) = result {
            self.worker.missing_file = error.kind() == io::ErrorKind::NotFound;
            self.worker.open_error = error.raw_os_error();
            self.worker.open_error_kind = Some(if self.worker.missing_file {
                "not_found"
            } else {
                "error"
            });
        }
    }
    fn decode_worker(&mut self) -> io::Result<()> {
        let mut file = std::fs::File::open(&self.worker_path)?;
        let mut bytes = [0u8; WORKER_CAP + 1];
        let mut count = 0;
        while count < bytes.len() {
            match file.read(&mut bytes[count..]) {
                Ok(0) => {
                    self.worker.eof_observed = true;
                    break;
                }
                Ok(n) => count += n,
                Err(error) => {
                    self.worker.read_error = true;
                    self.worker.read_error_raw = error.raw_os_error();
                    break;
                }
            }
        }
        self.worker.read_bytes = count;
        self.worker.over_limit = count > WORKER_CAP;
        let bytes = &bytes[..count];
        self.worker.complete_lines_observed = bytes.iter().filter(|&&b| b == b'\n').count();
        self.worker.line_limit_exceeded = self.worker.complete_lines_observed > RECORD_CAP;
        self.worker.partial_tail_bytes = bytes
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(count, |at| count - at - 1);
        let (mut sequences, mut stages) = (0u32, 0u32);
        let mut max_seq = None::<u8>;
        let mut previous_seq = None::<u8>;
        let mut previous_elapsed = None::<u64>;
        let mut previous_wall = None::<u128>;
        let mut previous_failures = None::<u8>;
        let mut ordered_success = true;
        let mut last_stage = None;
        for line in bytes.split_inclusive(|&b| b == b'\n').take(RECORD_CAP) {
            if !line.ends_with(b"\n") {
                break;
            }
            self.worker.lines_examined += 1;
            if line.len() > LINE_CAP {
                self.worker.oversize_lines += 1;
                self.worker.invalid_lines += 1;
                continue;
            }
            let Ok(record) = serde_json::from_slice::<WorkerLine>(line) else {
                self.worker.invalid_lines += 1;
                continue;
            };
            if record.v != 2
                || record.capture_id != self.capture_id
                || record.server_pid != self.server_pid
                || record.request_seq != self.request_seq
                || Some(record.pid) != self.worker_pid
                || record.seq >= RECORD_CAP as u8
                || record.stage > 17
                || record.failed_before > record.seq
                || (matches!(record.stage, 14 | 16 | 17) && record.detail > 1)
            {
                self.worker.invalid_lines += 1;
                continue;
            }
            let seq_bit = 1u32 << record.seq;
            let stage_bit = 1u32 << record.stage;
            if sequences & seq_bit != 0 {
                self.worker.duplicate_sequence += 1;
                continue;
            }
            if stages & stage_bit != 0 {
                self.worker.duplicate_stage += 1;
            }
            if previous_seq.is_some_and(|s| record.seq < s) {
                self.worker.sequence_out_of_order += 1;
            }
            if previous_elapsed.is_some_and(|e| record.elapsed_ns < e) {
                self.worker.elapsed_clock_regressions += 1;
            }
            if previous_wall
                .zip(record.unix_ns)
                .is_some_and(|(a, b)| b < a)
            {
                self.worker.unix_clock_regressions += 1;
            }
            if previous_failures.is_some_and(|f| record.failed_before < f) {
                self.worker.failure_count_regressions += 1;
            }
            previous_seq = Some(record.seq);
            previous_elapsed = Some(record.elapsed_ns);
            if record.unix_ns.is_some() {
                previous_wall = record.unix_ns;
            }
            previous_failures = Some(record.failed_before);
            self.worker.clock_limit_observed |=
                record.elapsed_ns == u64::MAX || record.unix_ns == Some(u128::MAX);
            self.worker.unix_clock_missing += u32::from(record.unix_ns.is_none());
            self.worker.prior_write_failures_reported = Some(
                self.worker
                    .prior_write_failures_reported
                    .unwrap_or(0)
                    .max(record.failed_before),
            );
            ordered_success &= record.seq == record.stage
                && record.seq as usize == self.worker.retained
                && (!matches!(record.stage, 14 | 16 | 17) || record.detail == 1);
            sequences |= seq_bit;
            stages |= stage_bit;
            max_seq = Some(max_seq.map_or(record.seq, |s| s.max(record.seq)));
            last_stage = Some(record.stage);
            if record.stage == 17 {
                self.worker.before_exit_is_ok = Some(record.detail == 1);
            }
            self.worker.records[self.worker.retained] = Some(record);
            self.worker.retained += 1;
        }
        self.worker.sequence_gap_mask = max_seq.map_or(0, |s| (u32::MAX >> (31 - s)) & !sequences);
        self.worker.absent_stage_mask = ((1 << 18) - 1) & !stages;
        self.worker.complete_successful_stream = self.worker.eof_observed
            && !self.worker.over_limit
            && !self.worker.read_error
            && !self.worker.line_limit_exceeded
            && self.worker.partial_tail_bytes == 0
            && self.worker.invalid_lines == 0
            && self.worker.duplicate_sequence == 0
            && self.worker.duplicate_stage == 0
            && self.worker.sequence_gap_mask == 0
            && self.worker.sequence_out_of_order == 0
            && self.worker.absent_stage_mask == 0
            && self.worker.retained == 18
            && self.worker.prior_write_failures_reported == Some(0)
            && self.worker.failure_count_regressions == 0
            && self.worker.before_exit_is_ok == Some(true)
            && self.worker.elapsed_clock_regressions == 0
            && self.worker.unix_clock_regressions == 0
            && !self.worker.clock_limit_observed
            && ordered_success
            && last_stage == Some(17);
        self.worker.tail_unknown = !self.worker.complete_successful_stream;
        Ok(())
    }
    fn persist(mut self) {
        self.return_unix_ns = wall();
        self.times.summary_begin_ns = Some(ns(self.origin, Instant::now()));
        let mut bytes = [0u8; SUMMARY_CAP];
        let mut out = &mut bytes[..];
        if serde_json::to_writer(&mut out, &self).is_err() {
            return;
        }
        let count = SUMMARY_CAP - out.len();
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
