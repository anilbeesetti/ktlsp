//! Isolated, bounded parser adoption gate. No grammar or semantic parity claim.
pub mod ancestor_candidates;
pub mod declaration_name_index;
pub mod lowering;
pub mod position_regions;
pub mod structural_regions;

use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(any(not(unix), test))]
use std::sync::mpsc;

#[cfg(test)]
#[path = "../tests/support/functional.rs"]
mod functional_test;

pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ELEMENTS: usize = 16_384;
pub const MAX_DIAGNOSTICS: usize = 256;
pub const MAX_NESTING: usize = 128;
pub const MAX_ALLOCATION_BYTES: usize = 64 * 1024 * 1024;
#[doc(hidden)]
pub const WORKER_ALLOCATION_LIMIT_EXIT_CODE: i32 = 86;
pub const TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Source,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserError {
    InputLimit,
    NestingLimit,
    OutputLimit,
    /// The worker observed a cumulative-cap refusal before its outer exit boundary.
    AllocationLimit,
    Coverage,
    InvalidTree,
    Cancelled,
    Timeout,
    WorkerFailed,
    Io,
    StaleSnapshot,
}
impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parser rejected request: {self:?}")
    }
}
impl std::error::Error for ParserError {}

/// Flat preorder CST element. Offsets refer to the original source bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    pub kind: String,
    pub start: usize,
    pub end: usize,
    pub parent: Option<usize>,
    pub subtree_end: usize,
    pub token: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub message: String,
}

#[doc(hidden)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Reply {
    Tree {
        #[serde(deserialize_with = "read_elements")]
        elements: Vec<Element>,
        #[serde(deserialize_with = "read_diagnostics")]
        diagnostics: Vec<Diagnostic>,
    },
    Coverage,
    OutputLimit,
    NestingLimit,
}

fn read_elements<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Element>, D::Error> {
    read_bounded::<Element, D, MAX_ELEMENTS>(d)
}
fn read_diagnostics<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Diagnostic>, D::Error> {
    read_bounded::<Diagnostic, D, MAX_DIAGNOSTICS>(d)
}
fn read_bounded<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>, const MAX: usize>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    struct Bounded<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> serde::de::Visitor<'de> for Bounded<T, MAX> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("bounded parser sequence")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == MAX {
                    return Err(serde::de::Error::custom("parser sequence limit"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Bounded::<T, MAX>(std::marker::PhantomData))
}

/// Authoritative immutable input. The caller assigns a document identity and revision.
/// Reopening a document must use a new identity or revision; byte offsets are not symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    document: Arc<str>,
    revision: i64,
    kind: FileKind,
    source: Arc<str>,
}
impl SourceSnapshot {
    pub fn new(
        document: impl Into<Arc<str>>,
        revision: i64,
        kind: FileKind,
        source: impl Into<Arc<str>>,
    ) -> Result<Self, ParserError> {
        let source = source.into();
        if source.len() > MAX_SOURCE_BYTES {
            return Err(ParserError::InputLimit);
        }
        Ok(Self {
            document: document.into(),
            revision,
            kind,
            source,
        })
    }
    pub fn document(&self) -> &str {
        &self.document
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub fn kind(&self) -> FileKind {
        self.kind
    }
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// A validated tree can only be retrieved against its exact authoritative snapshot.
#[derive(Debug)]
pub struct SnapshotResult {
    snapshot: SourceSnapshot,
    parsed: Parsed,
}
impl SnapshotResult {
    pub fn snapshot(&self) -> &SourceSnapshot {
        &self.snapshot
    }
    pub fn tree_for(&self, current: &SourceSnapshot) -> Result<&Parsed, ParserError> {
        if &self.snapshot != current {
            return Err(ParserError::StaleSnapshot);
        }
        Ok(&self.parsed)
    }
}

#[derive(Debug)]
pub struct Parsed {
    source: Arc<str>,
    elements: Vec<Element>,
    diagnostics: Vec<Diagnostic>,
}
impl Parsed {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn position(
        &self,
        original_offset: usize,
    ) -> Result<ktlsp_text::Position, ktlsp_text::TextError> {
        ktlsp_text::offset_to_position(&self.source, original_offset)
    }
}

#[cfg(unix)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DeadlineReceipt {
    #[default]
    Missing,
    Invalid,
    CheckpointAfterDecision,
}

#[cfg(unix)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeadlineEvidence {
    pub deadline_selected: bool,
    pub decision_send: Option<Result<usize, std::io::ErrorKind>>,
    pub receipt: DeadlineReceipt,
}

#[cfg(unix)]
struct DeadlineObserver {
    socket: std::os::unix::net::UnixDatagram,
    evidence: DeadlineEvidence,
}
#[cfg(unix)]
impl DeadlineObserver {
    fn new(command: &mut Command) -> std::io::Result<Self> {
        use std::os::{fd::OwnedFd, unix::net::UnixDatagram};
        let (socket, worker) = UnixDatagram::pair()?;
        socket.set_nonblocking(true)?;
        worker.set_nonblocking(true)?;
        command
            .arg("--observe-deadline-eof-v1")
            .stderr(Stdio::from(OwnedFd::from(worker)));
        Ok(Self {
            socket,
            evidence: DeadlineEvidence::default(),
        })
    }
    fn select_deadline(&mut self) {
        self.evidence.deadline_selected = true;
        self.evidence.decision_send =
            Some(self.socket.send(b"KTLSP-DUE-v1!!!!").map_err(|e| e.kind()));
    }
    fn without_receipt(self) -> DeadlineEvidence {
        let Self {
            socket,
            mut evidence,
        } = self;
        drop(socket);
        evidence.receipt = DeadlineReceipt::Missing;
        evidence
    }
    fn finish(mut self) -> DeadlineEvidence {
        let mut bytes = [0; 17];
        self.evidence.receipt = match self.socket.recv(&mut bytes) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => DeadlineReceipt::Missing,
            Ok(16)
                if bytes[..16] == *b"KTLSP-EOF-v1!!!!"
                    && self.evidence.deadline_selected
                    && self.evidence.decision_send == Some(Ok(16)) =>
            {
                match self.socket.recv(&mut bytes) {
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        DeadlineReceipt::CheckpointAfterDecision
                    }
                    _ => DeadlineReceipt::Invalid,
                }
            }
            _ => DeadlineReceipt::Invalid,
        };
        if self.evidence.deadline_selected && self.evidence.decision_send != Some(Ok(16)) {
            self.evidence.receipt = DeadlineReceipt::Invalid;
        }
        self.evidence
    }
}

#[cfg(unix)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericCopyRecord {
    pub status: u32,
    pub source_len: u32,
    pub active_events: u32,
    pub active_layers: u32,
    pub production_bound: i32,
    pub independent_min: i32,
    pub window_start: u32,
    pub actual_copy_end: u32,
    pub next_start: u32,
    pub next_end: u32,
    pub next_fits_other_caps: u32,
    pub reserved: u32,
}
#[cfg(unix)]
impl Default for GenericCopyRecord {
    fn default() -> Self {
        Self {
            status: 0,
            source_len: 0,
            active_events: 0,
            active_layers: 0,
            production_bound: -1,
            independent_min: -1,
            window_start: 0,
            actual_copy_end: 0,
            next_start: 0,
            next_end: 0,
            next_fits_other_caps: 0,
            reserved: 0,
        }
    }
}
#[cfg(unix)]
impl GenericCopyRecord {
    #[doc(hidden)]
    pub fn to_bytes(self) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[..16].copy_from_slice(b"KTLSP-B11COPY-v1");
        let words = [
            self.status,
            self.source_len,
            self.active_events,
            self.active_layers,
            self.production_bound as u32,
            self.independent_min as u32,
            self.window_start,
            self.actual_copy_end,
            self.next_start,
            self.next_end,
            self.next_fits_other_caps,
            self.reserved,
        ];
        for (chunk, word) in bytes[16..].as_chunks_mut::<4>().0.iter_mut().zip(words) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    fn from_bytes(bytes: &[u8], source: &str) -> Option<Self> {
        if bytes.len() != 64 || bytes[..16] != *b"KTLSP-B11COPY-v1" {
            return None;
        }
        let mut words = [0; 12];
        for (word, chunk) in words.iter_mut().zip(bytes[16..].as_chunks::<4>().0) {
            *word = u32::from_le_bytes(*chunk);
        }
        let record = Self {
            status: words[0],
            source_len: words[1],
            active_events: words[2],
            active_layers: words[3],
            production_bound: words[4] as i32,
            independent_min: words[5] as i32,
            window_start: words[6],
            actual_copy_end: words[7],
            next_start: words[8],
            next_end: words[9],
            next_fits_other_caps: words[10],
            reserved: words[11],
        };
        if source.len() > MAX_SOURCE_BYTES
            || record.source_len as usize != source.len()
            || record.reserved != 0
        {
            return None;
        }
        if record.status == 0 {
            return (record
                == Self {
                    source_len: record.source_len,
                    ..Self::default()
                })
            .then_some(record);
        }
        let boundary =
            |offset: u32| offset <= record.source_len && source.is_char_boundary(offset as usize);
        let next = (record.next_start == record.actual_copy_end
            && record.next_start < record.next_end
            && boundary(record.next_end))
            || (record.next_start == 0
                && record.next_end == 0
                && record.next_fits_other_caps == 0
                && record.actual_copy_end == record.source_len);
        (record.status == 1
            && record.active_events == 1
            && record.active_layers > 0
            && record.production_bound >= -1
            && (record.production_bound == -1 || boundary(record.production_bound as u32))
            && record.independent_min >= 0
            && boundary(record.independent_min as u32)
            && record.window_start < record.actual_copy_end
            && boundary(record.window_start)
            && boundary(record.actual_copy_end)
            && record.next_fits_other_caps <= 1
            && next)
            .then_some(record)
    }
}
#[cfg(unix)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GenericCopyEvidence {
    #[default]
    Missing,
    Invalid,
    Complete(GenericCopyRecord),
}
#[cfg(unix)]
struct GenericCopyObserver(std::os::unix::net::UnixDatagram);
#[cfg(unix)]
impl GenericCopyObserver {
    fn new(command: &mut Command) -> std::io::Result<Self> {
        use std::os::{fd::OwnedFd, unix::net::UnixDatagram};
        let (socket, worker) = UnixDatagram::pair()?;
        socket.set_nonblocking(true)?;
        worker.set_nonblocking(true)?;
        command
            .arg("--observe-generic-copy-v1")
            .stderr(Stdio::from(OwnedFd::from(worker)));
        Ok(Self(socket))
    }
    fn finish(self, source: &str) -> GenericCopyEvidence {
        let mut bytes = [0; 65];
        let record = match self.0.recv(&mut bytes) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return GenericCopyEvidence::Missing;
            }
            Ok(64) => GenericCopyRecord::from_bytes(&bytes[..64], source),
            _ => None,
        };
        match (record, self.0.recv(&mut bytes)) {
            (Some(record), Err(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                GenericCopyEvidence::Complete(record)
            }
            _ => GenericCopyEvidence::Invalid,
        }
    }
}

/// Caller supplies the trusted installed Rust worker's absolute path.
/// Unix calls share eight reservations, including pending or failed cleanup.
pub struct Parser {
    worker: PathBuf,
}
impl Parser {
    pub fn new(worker: impl AsRef<Path>) -> Result<Self, ParserError> {
        if !worker.as_ref().is_absolute() {
            return Err(ParserError::Io);
        }
        Ok(Self {
            worker: worker.as_ref().to_owned(),
        })
    }
    pub fn parse_snapshot(
        &self,
        snapshot: &SourceSnapshot,
        cancelled: &AtomicBool,
    ) -> Result<SnapshotResult, ParserError> {
        self.parse_snapshot_with_cleanup(snapshot, cancelled).0
    }
    /// Preserve the normal parse result and report this invocation's cleanup.
    /// True means no child ownership/spawn attempt, or this attempt's same child
    /// was confirmed reaped (and non-Unix scoped I/O has exited) before return.
    /// False means unproved closure, not proof that a child remains alive.
    /// It is never revised from later/global state. Joining the caller is not
    /// child-closure proof. Limits, selected errors and the Unix 20 ms cleanup
    /// observation remain unchanged.
    pub fn parse_snapshot_with_cleanup(
        &self,
        snapshot: &SourceSnapshot,
        cancelled: &AtomicBool,
    ) -> (Result<SnapshotResult, ParserError>, bool) {
        let mut cleanup = true;
        let result = self
            .parse_inner(
                snapshot.source(),
                snapshot.kind,
                cancelled,
                &mut cleanup,
                #[cfg(unix)]
                None,
                #[cfg(unix)]
                None,
            )
            .map(|mut parsed| {
                parsed.source = Arc::clone(&snapshot.source);
                SnapshotResult {
                    snapshot: snapshot.clone(),
                    parsed,
                }
            });
        (result, cleanup)
    }
    /// Finite Unix observation; overhead is unmeasured and missing receipts are inconclusive.
    #[cfg(unix)]
    #[doc(hidden)]
    pub fn parse_snapshot_observing_deadline(
        &self,
        snapshot: &SourceSnapshot,
        cancelled: &AtomicBool,
    ) -> (Result<SnapshotResult, ParserError>, DeadlineEvidence) {
        let mut evidence = DeadlineEvidence::default();
        let mut cleanup = true;
        let result = self
            .parse_inner(
                snapshot.source(),
                snapshot.kind,
                cancelled,
                &mut cleanup,
                Some(&mut evidence),
                None,
            )
            .map(|mut parsed| {
                parsed.source = Arc::clone(&snapshot.source);
                SnapshotResult {
                    snapshot: snapshot.clone(),
                    parsed,
                }
            });
        (result, evidence)
    }
    /// Finite actual-copy observation; effects on allocation/deadline outcomes are unmeasured.
    #[cfg(unix)]
    #[doc(hidden)]
    pub fn parse_snapshot_observing_generic_copy(
        &self,
        snapshot: &SourceSnapshot,
        cancelled: &AtomicBool,
    ) -> (Result<SnapshotResult, ParserError>, GenericCopyEvidence) {
        let mut evidence = GenericCopyEvidence::Missing;
        let mut cleanup = true;
        let result = self
            .parse_inner(
                snapshot.source(),
                snapshot.kind,
                cancelled,
                &mut cleanup,
                None,
                Some(&mut evidence),
            )
            .map(|mut parsed| {
                parsed.source = Arc::clone(&snapshot.source);
                SnapshotResult {
                    snapshot: snapshot.clone(),
                    parsed,
                }
            });
        (result, evidence)
    }
    /// Unversioned parser gate for standalone callers; editor consumers use parse_snapshot.
    pub fn parse(
        &self,
        source: &str,
        kind: FileKind,
        cancelled: &AtomicBool,
    ) -> Result<Parsed, ParserError> {
        let mut cleanup = true;
        self.parse_inner(
            source,
            kind,
            cancelled,
            &mut cleanup,
            #[cfg(unix)]
            None,
            #[cfg(unix)]
            None,
        )
    }
    fn parse_inner(
        &self,
        source: &str,
        kind: FileKind,
        cancelled: &AtomicBool,
        cleanup_status: &mut bool,
        #[cfg(unix)] evidence: Option<&mut DeadlineEvidence>,
        #[cfg(unix)] copy_evidence: Option<&mut GenericCopyEvidence>,
    ) -> Result<Parsed, ParserError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(ParserError::InputLimit);
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(ParserError::Cancelled);
        }
        preflight(source)?;
        #[cfg(unix)]
        let mut lease = admission::reserve()?;
        // Once initialization is outside the exchange clock and can block.
        #[cfg(unix)]
        if cancelled.load(Ordering::Relaxed) {
            return Err(ParserError::Cancelled);
        }
        {
            let (normalized, map) = normalize(source);
            #[cfg(unix)]
            if normalized.capacity() > MAX_SOURCE_BYTES || map.capacity() > MAX_SOURCE_BYTES + 1 {
                return Err(ParserError::Io);
            }
            let mut command = Command::new(&self.worker);
            command.arg(match kind {
                FileKind::Source => "--source",
                FileKind::Script => "--script",
            });
            command.stderr(Stdio::null());
            #[cfg(unix)]
            let mut observer = if evidence.is_some() {
                Some(DeadlineObserver::new(&mut command).map_err(|_| ParserError::Io)?)
            } else {
                None
            };
            #[cfg(unix)]
            let copy_observer = if copy_evidence.is_some() {
                Some(GenericCopyObserver::new(&mut command).map_err(|_| ParserError::Io)?)
            } else {
                None
            };
            #[cfg(unix)]
            let mut cleanup = (admission::Cleanup::Failed, true);
            let bytes = supervise(
                command,
                &normalized,
                cancelled,
                #[cfg(not(unix))]
                cleanup_status,
                #[cfg(unix)]
                observer.as_mut(),
                #[cfg(unix)]
                copy_observer.is_some(),
                #[cfg(unix)]
                &mut lease,
                #[cfg(unix)]
                &mut cleanup,
            );
            #[cfg(unix)]
            {
                *cleanup_status = cleanup.1;
            }
            #[cfg(unix)]
            if let (Some(observer), Some(evidence)) = (observer, evidence) {
                *evidence = if cleanup.0 == admission::Cleanup::Confirmed {
                    observer.finish()
                } else {
                    observer.without_receipt()
                };
            }
            #[cfg(unix)]
            if let (Some(observer), Some(evidence)) = (copy_observer, copy_evidence) {
                *evidence = if cleanup.0 == admission::Cleanup::Confirmed {
                    observer.finish(&normalized)
                } else {
                    drop(observer);
                    GenericCopyEvidence::Missing
                };
            }
            let bytes = bytes?;
            if cancelled.load(Ordering::Relaxed) {
                return Err(ParserError::Cancelled);
            }
            let reply: Reply =
                serde_json::from_slice(&bytes).map_err(|_| ParserError::InvalidTree)?;
            validate(source, &normalized, &map, reply)
        }
    }
}
fn supervise(
    mut command: Command,
    normalized: &str,
    cancelled: &AtomicBool,
    #[cfg(not(unix))] cleanup_status: &mut bool,
    #[cfg(unix)] observer: Option<&mut DeadlineObserver>,
    #[cfg(unix)] preserve_copy_stderr: bool,
    #[cfg(unix)] lease: &mut admission::Lease<'_>,
    #[cfg(unix)] cleanup: &mut (admission::Cleanup, bool),
) -> Result<Vec<u8>, ParserError> {
    #[cfg(unix)]
    if observer.is_none() && !preserve_copy_stderr {
        command.stderr(Stdio::null());
    }
    #[cfg(not(unix))]
    command.stderr(Stdio::null());
    let deadline = Instant::now() + TIMEOUT;
    #[cfg(unix)]
    {
        unix_io::supervise(
            command, normalized, cancelled, deadline, observer, lease, cleanup,
        )
    }
    #[cfg(not(unix))]
    {
        supervise_piped(command, normalized, cancelled, deadline, cleanup_status)
    }
}

#[cfg(unix)]
mod admission {
    use super::*;
    use std::{
        panic::catch_unwind,
        sync::{
            Mutex, MutexGuard, Once, TryLockError,
            atomic::{AtomicU8, Ordering::SeqCst},
        },
        thread::JoinHandle,
    };
    const N: usize = 8;
    const B: Duration = Duration::from_millis(20);
    const POLL: Duration = Duration::from_millis(2);
    const FREE: u8 = 0;
    const ACTIVE: u8 = 1;
    const WAITING: u8 = 2;
    const ABANDONED: u8 = 3;
    const CONFIRMED: u8 = 4;
    const FAILED: u8 = 5;
    const NOT_STARTED: u8 = 0;
    const STARTING: u8 = 1;
    const READY: u8 = 2;
    const DEAD: u8 = 3;

    struct Data {
        child: Option<Child>,
        attempted: bool,
        reaped: bool,
    }
    fn valid_confirmation(attempted: bool, has_child: bool, reaped: bool) -> bool {
        attempted && has_child && reaped
    }
    struct Slot {
        state: AtomicU8,
        data: Mutex<Data>,
    }
    impl Slot {
        const fn new() -> Self {
            Self {
                state: AtomicU8::new(FREE),
                data: Mutex::new(Data {
                    child: None,
                    attempted: false,
                    reaped: false,
                }),
            }
        }
    }
    static SLOTS: [Slot; N] = [const { Slot::new() }; N];
    static INIT: Once = Once::new();
    static SINGLETON: AtomicU8 = AtomicU8::new(NOT_STARTED);
    static HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

    pub(super) struct Lease<'a> {
        slot: &'a Slot,
        guard: Option<MutexGuard<'a, Data>>,
        armed: bool,
    }
    // Production entry is always the one process-global SLOTS array. The private
    // reserve_on seam lets isolated capacity tests reuse the exact slot algorithm.
    pub(super) fn reserve() -> Result<Lease<'static>, ParserError> {
        let lease = Lease::reserve_on(&SLOTS)?;
        start_reaper()?; // Empty reservation held during potentially blocking Once.
        Ok(lease)
    }
    impl<'a> Lease<'a> {
        // Call only after existing input-size/cancellation/preflight checks; all
        // normalization/Command/socket allocations follow the returned reservation.
        fn reserve_on(slots: &'a [Slot; N]) -> Result<Self, ParserError> {
            for slot in slots {
                if slot
                    .state
                    .compare_exchange(FREE, ACTIVE, SeqCst, SeqCst)
                    .is_err()
                {
                    continue;
                }
                let guard = match slot.data.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        slot.state.store(FAILED, SeqCst);
                        return Err(ParserError::Io);
                    }
                };
                let lease = Self {
                    slot,
                    guard: Some(guard),
                    armed: true,
                };
                return Ok(lease);
            }
            Err(ParserError::Io)
        }

        // Command must be declared AFTER Lease so its transient child-end aliases
        // drop first during unwind, and explicitly dropped immediately after spawn.
        pub(super) fn spawn(&mut self, command: &mut Command) -> Result<(), ParserError> {
            if SINGLETON.load(SeqCst) != READY {
                return Err(ParserError::Io);
            }
            let data = self.guard.as_mut().ok_or(ParserError::Io)?;
            // One attempt only, even if spawn returns Err without a Child handle.
            if data.attempted || data.child.is_some() || data.reaped {
                return Err(ParserError::Io);
            }
            data.attempted = true;
            match command.spawn() {
                Ok(child) => {
                    data.child = Some(child);
                    Ok(())
                }
                Err(_) => Err(ParserError::Io), // Drop pins attempted-without-Child; no retry.
            }
        }

        pub(super) fn status(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
            let data = self
                .guard
                .as_mut()
                .ok_or_else(|| std::io::Error::other("missing reserved child"))?;
            let child = data
                .child
                .as_mut()
                .ok_or_else(|| std::io::Error::other("missing reserved child"))?;
            let status = child.try_wait()?;
            if status.is_some() {
                data.reaped = true;
            }
            Ok(status)
        }

        // E2 separates observed failure from unconfirmed observation expiry.
        // All selected errors remain frozen outside this cleanup-status method.
        pub(super) fn cleanup(&mut self) -> Cleanup {
            let Some(data) = self.guard.as_ref() else {
                self.slot.state.store(FAILED, SeqCst);
                return Cleanup::Failed;
            };
            if let Some(cleanup) = initial_cleanup(
                data.attempted,
                data.child.is_some(),
                data.reaped,
                self.slot.state.load(SeqCst) == FAILED,
                SINGLETON.load(SeqCst) == DEAD,
            ) {
                if cleanup == Cleanup::Failed {
                    self.slot.state.store(FAILED, SeqCst);
                }
                return cleanup;
            }
            let origin = Instant::now(); // E1 origin, before WAITING publication.
            self.slot.state.store(WAITING, SeqCst);
            drop(self.guard.take());
            loop {
                let due_before = origin.elapsed() >= B;
                let state = self.slot.state.load(SeqCst);
                let dead = SINGLETON.load(SeqCst) == DEAD;
                let due_after = origin.elapsed() >= B; // E1 decision clock.
                if let Some(cleanup) = observation(state, dead, due_before, due_after) {
                    return if cleanup == Cleanup::Confirmed {
                        self.accept_confirmation()
                    } else {
                        if cleanup == Cleanup::Failed {
                            self.slot.state.store(FAILED, SeqCst);
                        }
                        cleanup
                    };
                }
                thread::sleep(POLL);
            }
        }
        fn accept_confirmation(&mut self) -> Cleanup {
            let guard = match self.slot.data.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    self.slot.state.store(FAILED, SeqCst);
                    return Cleanup::Failed;
                }
            };
            if !valid_confirmation(guard.attempted, guard.child.is_some(), guard.reaped) {
                drop(guard);
                self.slot.state.store(FAILED, SeqCst);
                return Cleanup::Failed;
            }
            self.guard = Some(guard);
            Cleanup::Confirmed
        }
    }

    fn initial_cleanup(
        attempted: bool,
        has_child: bool,
        reaped: bool,
        failed: bool,
        dead: bool,
    ) -> Option<Cleanup> {
        if failed || !attempted || !has_child {
            Some(Cleanup::Failed)
        } else if reaped {
            Some(Cleanup::Confirmed)
        } else if dead {
            Some(Cleanup::Failed)
        } else {
            None
        }
    }

    fn observation(state: u8, dead: bool, due_before: bool, due_after: bool) -> Option<Cleanup> {
        // Sampled failure wins over both expiry clocks; late confirmation is unconfirmed.
        if dead || !matches!(state, WAITING | CONFIRMED) {
            Some(Cleanup::Failed)
        } else if due_before || due_after {
            Some(Cleanup::Unconfirmed)
        } else if state == CONFIRMED {
            Some(Cleanup::Confirmed)
        } else {
            None
        }
    }

    // At most two CAS attempts: WAITING may race to CONFIRMED; FAILED is pinned.
    fn abandon(state: &AtomicU8) {
        if state
            .compare_exchange(WAITING, ABANDONED, SeqCst, SeqCst)
            .is_err()
        {
            let _ = state.compare_exchange(CONFIRMED, ABANDONED, SeqCst, SeqCst);
        }
    }
    impl Drop for Lease<'_> {
        fn drop(&mut self) {
            if !self.armed {
                return;
            }
            // Caller lexical resources have already dropped before this lease.
            self.armed = false; // No destructor can touch a reused slot after publish.
            if self.slot.state.load(SeqCst) == FAILED {
                drop(self.guard.take()); // Preserve the pin and static Child/metadata.
                return;
            }
            if let Some(mut data) = self.guard.take() {
                let releasable =
                    valid_confirmation(data.attempted, data.child.is_some(), data.reaped)
                        || (!data.attempted && data.child.is_none() && !data.reaped);
                if releasable {
                    data.child = None; // Child is either absent or confirmed reaped.
                    data.reaped = false;
                    data.attempted = false;
                    drop(data); // Mutex unlock precedes FREE publication.
                    self.slot.state.store(FREE, SeqCst);
                } else {
                    let state = if data.attempted && data.child.is_some() && !data.reaped {
                        ABANDONED
                    } else {
                        FAILED
                    };
                    drop(data);
                    self.slot.state.store(state, SeqCst);
                }
            } else {
                abandon(&self.slot.state); // Final invocation access; never FREE.
            }
        }
    }

    struct StartupGuard(bool);
    impl Drop for StartupGuard {
        fn drop(&mut self) {
            if self.0 {
                SINGLETON.store(DEAD, SeqCst);
            }
        }
    }
    struct ReaperDeath;
    impl Drop for ReaperDeath {
        fn drop(&mut self) {
            SINGLETON.store(DEAD, SeqCst);
        }
    }
    fn start_reaper() -> Result<(), ParserError> {
        initialize(&INIT, &SINGLETON, start_reaper_once)
    }
    fn initialize(
        init: &Once,
        singleton: &AtomicU8,
        start: impl FnOnce() -> Result<(), ParserError> + std::panic::UnwindSafe,
    ) -> Result<(), ParserError> {
        if singleton.load(SeqCst) == DEAD {
            return Err(ParserError::Io);
        }
        // Once may block before the existing exchange clock. Catch INSIDE the
        // initializer, so ordinary startup panic completes Once with terminal DEAD.
        // Outer catch also contains poisoned-Once panic for already waiting callers.
        let once_result = catch_unwind(|| {
            init.call_once(|| {
                let result = catch_unwind(start);
                if !matches!(&result, Ok(Ok(()))) {
                    singleton.store(DEAD, SeqCst);
                }
                drop(result); // A panicking payload Drop can poison Once; DEAD is already set.
            })
        });
        if once_result.is_err() {
            singleton.store(DEAD, SeqCst);
        }
        drop(once_result);
        if singleton.load(SeqCst) == READY {
            Ok(())
        } else {
            Err(ParserError::Io)
        }
    }
    fn start_reaper_once() -> Result<(), ParserError> {
        let mut startup = StartupGuard(true);
        SINGLETON
            .compare_exchange(NOT_STARTED, STARTING, SeqCst, SeqCst)
            .map_err(|_| ParserError::Io)?;
        let mut retained = HANDLE.try_lock().map_err(|_| ParserError::Io)?;
        if retained.is_some() {
            return Err(ParserError::Io);
        }
        let handle = thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(reaper)
            .map_err(|_| ParserError::Io)?;
        *retained = Some(handle); // Only this Once initializer can create/retain one.
        drop(retained);
        SINGLETON
            .compare_exchange(STARTING, READY, SeqCst, SeqCst)
            .map_err(|_| ParserError::Io)?;
        startup.0 = false;
        Ok(())
    }

    fn reaper() {
        let _death = ReaperDeath; // First nonpanicking entry action.
        loop {
            if SINGLETON.load(SeqCst) == DEAD {
                return;
            }
            for slot in &SLOTS {
                reap_slot(slot);
            }
            thread::sleep(POLL);
        }
    }
    fn reap_slot(slot: &Slot) {
        let state = slot.state.load(SeqCst);
        if !matches!(state, WAITING | ABANDONED) {
            return;
        }
        let mut data = match slot.data.try_lock() {
            Ok(data) => data,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(_)) => {
                slot.state.store(FAILED, SeqCst);
                return;
            }
        };
        if !data.attempted || data.child.is_none() {
            slot.state.store(FAILED, SeqCst);
            return;
        }
        if !data.reaped {
            let Some(child) = data.child.as_mut() else {
                slot.state.store(FAILED, SeqCst);
                return;
            };
            let _ = child.kill(); // One call; this can block the only reaper forever.
            if child.wait().is_err() {
                slot.state.store(FAILED, SeqCst);
                return;
            }
            data.reaped = true;
        }
        if state == ABANDONED {
            data.child = None; // Confirmed reaped AND caller resources already gone.
            data.reaped = false;
            data.attempted = false;
            drop(data);
            slot.state.store(FREE, SeqCst); // Last access for that old invocation.
        } else {
            drop(data);
            // If caller won abandonment, the next sweep reclaims this confirmed Child.
            let _ = slot
                .state
                .compare_exchange(WAITING, CONFIRMED, SeqCst, SeqCst);
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Cleanup {
        Confirmed,
        Unconfirmed,
        Failed,
    }
    pub(super) fn select_result(
        selected: Result<Vec<u8>, ParserError>,
        cleanup: Cleanup,
    ) -> Result<Vec<u8>, ParserError> {
        match cleanup {
            Cleanup::Confirmed => selected,
            Cleanup::Failed => Err(ParserError::Io),
            Cleanup::Unconfirmed => match selected {
                Err(error) => Err(error),
                Ok(_) => Err(ParserError::Io), // Never return unconfirmed success.
            },
        }
    }

    #[cfg(test)]
    mod finite_state_checks {
        use super::*;
        #[test]
        fn valid_same_child_reap_confirms_after_unrelated_reaper_death() {
            for dead in [false, true] {
                assert_eq!(
                    initial_cleanup(true, true, true, false, dead),
                    Some(Cleanup::Confirmed)
                );
                assert_eq!(
                    initial_cleanup(true, true, true, true, dead),
                    Some(Cleanup::Failed)
                );
                assert_eq!(
                    initial_cleanup(true, false, true, false, dead),
                    Some(Cleanup::Failed)
                );
                assert_eq!(
                    initial_cleanup(false, true, true, false, dead),
                    Some(Cleanup::Failed)
                );
            }
            assert_eq!(
                initial_cleanup(true, true, false, false, true),
                Some(Cleanup::Failed)
            );
            assert_eq!(initial_cleanup(true, true, false, false, false), None);
        }
        #[test]
        fn cleanup_observation_failure_precedes_expiry_and_late_confirmation() {
            for (before, after) in [(false, false), (false, true), (true, false), (true, true)] {
                for state in [FREE, ACTIVE, ABANDONED, FAILED, u8::MAX] {
                    assert_eq!(
                        observation(state, false, before, after),
                        Some(Cleanup::Failed)
                    );
                }
                for state in [WAITING, CONFIRMED] {
                    assert_eq!(
                        observation(state, true, before, after),
                        Some(Cleanup::Failed)
                    );
                    if before || after {
                        assert_eq!(
                            observation(state, false, before, after),
                            Some(Cleanup::Unconfirmed)
                        );
                    }
                }
            }
            assert_eq!(observation(WAITING, false, false, false), None);
            assert_eq!(
                observation(CONFIRMED, false, false, false),
                Some(Cleanup::Confirmed)
            );
        }

        #[test]
        fn contending_once_callers_share_one_successful_initialization() {
            use std::sync::{atomic::AtomicUsize, mpsc};
            let init = Once::new();
            let singleton = AtomicU8::new(NOT_STARTED);
            let attempts = AtomicUsize::new(0);
            thread::scope(|scope| {
                // Disconnect the initializer gate before scope joins on unwind.
                let (release, gate) = mpsc::channel::<()>();
                let (started, entering) = mpsc::channel();
                let (init, singleton, attempts) = (&init, &singleton, &attempts);
                let first = scope.spawn(move || {
                    initialize(init, singleton, || {
                        attempts.fetch_add(1, SeqCst);
                        singleton.store(STARTING, SeqCst);
                        started.send(()).unwrap();
                        gate.recv().unwrap();
                        singleton.store(READY, SeqCst);
                        Ok(())
                    })
                });
                entering.recv().unwrap();
                let (entered, second_entering) = mpsc::channel();
                let (done, completed) = mpsc::channel();
                let second = scope.spawn(move || {
                    entered.send(()).unwrap();
                    let result =
                        initialize(init, singleton, || panic!("must not initialize twice"));
                    done.send(result).unwrap();
                    result
                });
                second_entering.recv().unwrap();
                let premature = completed.try_recv();
                let phase = singleton.load(SeqCst);
                let released = release.send(());
                drop(release);
                assert!(matches!(premature, Err(mpsc::TryRecvError::Empty)));
                assert_eq!(phase, STARTING);
                released.unwrap();
                assert_eq!(first.join().unwrap(), Ok(()));
                assert_eq!(second.join().unwrap(), Ok(()));
                assert_eq!(completed.recv().unwrap(), Ok(()));
            });
            assert_eq!(attempts.load(SeqCst), 1);
            assert_eq!(singleton.load(SeqCst), READY);
        }

        #[test]
        fn poisoned_or_busy_slot_is_permanently_pinned() {
            for poison in [false, true] {
                let slots = [const { Slot::new() }; N];
                if poison {
                    thread::scope(|scope| {
                        let join = scope.spawn(|| {
                            let _guard = slots[0].data.lock().unwrap();
                            panic!("isolated slot poison");
                        });
                        assert!(join.join().is_err());
                    });
                }
                let held = if poison {
                    None
                } else {
                    Some(slots[0].data.lock().unwrap())
                };
                assert!(Lease::reserve_on(&slots).is_err());
                drop(held);
                assert_eq!(slots[0].state.load(SeqCst), FAILED);
                let others: Vec<_> = (1..N).map(|_| Lease::reserve_on(&slots).unwrap()).collect();
                assert!(Lease::reserve_on(&slots).is_err());
                drop(others);
                assert_eq!(slots[0].state.load(SeqCst), FAILED);
                assert!(
                    slots[1..]
                        .iter()
                        .all(|slot| slot.state.load(SeqCst) == FREE)
                );
            }
        }

        #[test]
        fn abandonment_is_bounded_and_never_releases_capacity() {
            for initial in [WAITING, CONFIRMED, FAILED] {
                let state = AtomicU8::new(initial);
                abandon(&state);
                assert_eq!(
                    state.load(SeqCst),
                    if initial == FAILED { FAILED } else { ABANDONED }
                );
                assert_ne!(state.load(SeqCst), FREE);
            }
        }
        #[test]
        fn once_failure_and_panic_are_terminal_for_concurrent_callers() {
            use std::sync::atomic::AtomicUsize;
            for panic in [false, true] {
                let init = Once::new();
                let singleton = AtomicU8::new(NOT_STARTED);
                let attempts = AtomicUsize::new(0);
                thread::scope(|scope| {
                    let joins: Vec<_> = (0..N + 1)
                        .map(|_| {
                            scope.spawn(|| {
                                initialize(&init, &singleton, || {
                                    attempts.fetch_add(1, SeqCst);
                                    if panic {
                                        panic!("isolated startup failure");
                                    }
                                    Err(ParserError::Io)
                                })
                            })
                        })
                        .collect();
                    for join in joins {
                        assert_eq!(join.join().unwrap(), Err(ParserError::Io));
                    }
                });
                assert_eq!(attempts.load(SeqCst), 1);
                assert_eq!(singleton.load(SeqCst), DEAD);
                assert_eq!(
                    initialize(&init, &singleton, || panic!("must not retry")),
                    Err(ParserError::Io)
                );
            }
        }
        #[test]
        fn ready_cannot_overwrite_early_death() {
            let singleton = AtomicU8::new(STARTING);
            singleton.store(DEAD, SeqCst);
            assert!(
                singleton
                    .compare_exchange(STARTING, READY, SeqCst, SeqCst)
                    .is_err()
            );
            assert_eq!(singleton.load(SeqCst), DEAD);
        }
        #[test]
        fn fixed_slot_and_observation_budget() {
            assert_eq!(N, 8);
            assert_eq!(B, Duration::from_millis(20));
            assert_eq!(8 * (65_536 + 65_537 * 8 + 8_388_610 + 8_192), 71_893_072);
        }
        #[test]
        fn eight_local_reservations_reject_ninth_until_confirmed_empty_release() {
            let slots = [const { Slot::new() }; N];
            let held: Vec<_> = (0..N).map(|_| Lease::reserve_on(&slots).unwrap()).collect();
            assert!(Lease::reserve_on(&slots).is_err());
            assert!(slots.iter().all(|s| s.state.load(SeqCst) == ACTIVE));
            drop(held);
            assert!(slots.iter().all(|s| s.state.load(SeqCst) == FREE));
        }
        #[test]
        fn isolated_concurrent_admission_holds_eight_before_ninth_rejection() {
            use std::sync::mpsc;
            let slots = [const { Slot::new() }; N];
            thread::scope(|scope| {
                // Closure-local senders disconnect on unwind before scope joins.
                let mut releases = Vec::new();
                let (ready, results) = mpsc::channel();
                for _ in 0..N + 1 {
                    let (release, gate) = mpsc::channel::<()>();
                    releases.push(release);
                    let ready = ready.clone();
                    let slots = &slots;
                    scope.spawn(move || {
                        let held = Lease::reserve_on(slots);
                        let _ = ready.send(held.is_ok());
                        drop(ready); // Missing contender now disconnects results instead of stranding main.
                        let _ = gate.recv();
                        drop(held);
                    });
                }
                drop(ready);
                let outcomes: Vec<_> = (0..N + 1)
                    .map(|_| {
                        results
                            .recv()
                            .expect("each admitted contender reports once")
                    })
                    .collect();
                let accepted = outcomes.iter().filter(|ok| **ok).count();
                // Snapshot all outcomes while reservations remain held, then always
                // release workers BEFORE an assertion can unwind into scope joins.
                drop(releases);
                assert_eq!((outcomes.len(), accepted), (9, 8));
            });
        }
        #[test]
        fn eight_sequential_unconfirmed_spawn_attempts_permanently_exhaust_local_gate() {
            let slots = [const { Slot::new() }; N];
            for _ in 0..N {
                let mut lease = Lease::reserve_on(&slots).unwrap();
                lease.guard.as_mut().unwrap().attempted = true;
                drop(lease); // Exact production Drop branch for no returned Child.
            }
            assert!(slots.iter().all(|s| s.state.load(SeqCst) == FAILED));
            assert!(Lease::reserve_on(&slots).is_err());
        }
        #[test]
        fn unconfirmed_preserves_errors_but_never_success() {
            for error in [
                ParserError::Timeout,
                ParserError::Cancelled,
                ParserError::OutputLimit,
                ParserError::WorkerFailed,
                ParserError::AllocationLimit,
                ParserError::Io,
            ] {
                assert_eq!(select_result(Err(error), Cleanup::Unconfirmed), Err(error));
                assert_eq!(
                    select_result(Err(error), Cleanup::Failed),
                    Err(ParserError::Io)
                );
                assert_eq!(select_result(Err(error), Cleanup::Confirmed), Err(error));
            }
            assert_eq!(
                select_result(Ok(Vec::new()), Cleanup::Unconfirmed),
                Err(ParserError::Io)
            );
            assert_eq!(
                select_result(Ok(Vec::new()), Cleanup::Failed),
                Err(ParserError::Io)
            );
            assert_eq!(
                select_result(Ok(Vec::new()), Cleanup::Confirmed),
                Ok(Vec::new())
            );
        }

        #[test]
        fn invalid_confirmation_metadata_pins_instead_of_freeing() {
            // Missing-Child/reaped uses real local slot metadata, no fake Child.
            let slots = [const { Slot::new() }; N];
            let mut lease = Lease::reserve_on(&slots).unwrap();
            let slot = lease.slot;
            let data = lease.guard.as_mut().unwrap();
            data.attempted = true;
            data.reaped = true;
            assert_eq!(lease.cleanup(), Cleanup::Failed);
            drop(lease);
            assert_eq!(slot.state.load(SeqCst), FAILED);
            assert!(slot.data.try_lock().unwrap().reaped); // Pin did not erase metadata.

            let slots = [const { Slot::new() }; N];
            let mut lease = Lease::reserve_on(&slots).unwrap();
            let slot = lease.slot;
            lease.guard.as_mut().unwrap().attempted = true;
            drop(lease.guard.take());
            slot.state.store(CONFIRMED, SeqCst); // Unreaped ownership cannot confirm.
            assert_eq!(lease.accept_confirmation(), Cleanup::Failed);
            drop(lease);
            assert_eq!(slot.state.load(SeqCst), FAILED);
            // Actual shared predicate additionally rejects an unreaped present Child.
            assert!(!valid_confirmation(true, true, false));
            assert!(!valid_confirmation(false, true, true));
            assert!(!valid_confirmation(true, false, true));
            assert!(valid_confirmation(true, true, true));
        }
    }
}

#[cfg(unix)]
mod unix_io {
    use super::*;
    use std::{
        io,
        net::Shutdown,
        os::{fd::OwnedFd, unix::net::UnixStream},
    };

    const CHUNK: usize = 8192;

    // The extra operation lets tests check half-close failures independently of writes.
    trait Exchange: Read + Write {
        fn close_write(&mut self) -> io::Result<()>;
    }
    impl Exchange for UnixStream {
        fn close_write(&mut self) -> io::Result<()> {
            self.shutdown(Shutdown::Write)
        }
    }

    #[derive(Default)]
    struct Pump {
        offset: usize,
        output: Vec<u8>,
        reader: Option<Result<(), ParserError>>,
        writer: Option<Result<(), ParserError>>,
        input_close: Option<Result<(), ParserError>>,
        exited: bool,
    }
    enum Step {
        Pending(bool),
        Complete(Vec<u8>),
    }
    fn pending(error: &io::Error) -> bool {
        matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        )
    }
    fn control(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ParserError> {
        if cancelled.load(Ordering::Relaxed) {
            Err(ParserError::Cancelled)
        } else if Instant::now() >= deadline {
            Err(ParserError::Timeout)
        } else {
            Ok(())
        }
    }
    impl Pump {
        fn reserve_output_room(&mut self, additional: usize) -> Result<(), ParserError> {
            let needed = self
                .output
                .len()
                .checked_add(additional)
                .ok_or(ParserError::Io)?;
            if needed > MAX_OUTPUT_BYTES + 1 {
                return Err(ParserError::Io);
            }
            if needed > self.output.capacity() {
                let target = self
                    .output
                    .capacity()
                    .max(CHUNK)
                    .checked_mul(2)
                    .ok_or(ParserError::Io)?
                    .max(needed)
                    .min(8_388_610);
                self.output
                    .try_reserve_exact(target - self.output.len())
                    .map_err(|_| ParserError::Io)?;
            }
            if self.output.capacity() > 8_388_610 {
                drop(std::mem::take(&mut self.output));
                return Err(ParserError::Io);
            }
            Ok(())
        }
        fn append(&mut self, bytes: &[u8]) {
            if self.reserve_output_room(bytes.len()).is_err() {
                self.reader = Some(Err(ParserError::Io));
                return;
            }
            self.output.extend_from_slice(bytes);
            if self.output.len() > MAX_OUTPUT_BYTES {
                self.reader = Some(Err(ParserError::OutputLimit));
            }
        }
        fn read(&mut self, stream: &mut impl Read) -> bool {
            if self.reader.is_some() {
                return false;
            }
            let mut buffer = [0; CHUNK];
            let len = CHUNK.min(MAX_OUTPUT_BYTES + 1 - self.output.len());
            match stream.read(&mut buffer[..len]) {
                Ok(0) => self.reader = Some(Ok(())),
                Ok(count) => {
                    self.append(&buffer[..count]);
                }
                Err(error) if pending(&error) => return false,
                Err(_) => self.reader = Some(Err(ParserError::Io)),
            }
            true
        }
        fn write(&mut self, stream: &mut impl Write, input: &[u8]) -> bool {
            if self.writer.is_some() || self.offset == input.len() {
                return false;
            }
            let end = input.len().min(self.offset + CHUNK);
            match stream.write(&input[self.offset..end]) {
                Ok(0) => self.writer = Some(Err(ParserError::Io)),
                Ok(count) => self.offset += count,
                Err(error) if pending(&error) => return false,
                Err(_) => self.writer = Some(Err(ParserError::Io)),
            }
            true
        }
        fn close_input(&mut self, stream: &mut impl Exchange, input_len: usize) -> bool {
            if self.input_close.is_some()
                || (self.offset != input_len && self.writer != Some(Err(ParserError::Io)))
            {
                return false;
            }
            match stream.close_write() {
                Ok(()) => {
                    self.input_close = Some(Ok(()));
                    if self.writer.is_none() {
                        self.writer = Some(Ok(()));
                    }
                }
                Err(error) if pending(&error) => return false,
                Err(_) => {
                    self.input_close = Some(Err(ParserError::Io));
                    self.writer = Some(Err(ParserError::Io));
                }
            }
            true
        }
        fn step(
            &mut self,
            stream: &mut impl Exchange,
            input: &[u8],
            deadline: Instant,
            cancelled: &AtomicBool,
            observer: Option<&mut DeadlineObserver>,
            status: impl FnOnce() -> io::Result<Option<std::process::ExitStatus>>,
        ) -> Result<Step, ParserError> {
            if let Err(error) = control(deadline, cancelled) {
                if error == ParserError::Timeout
                    && !self.exited
                    && let Some(observer) = observer
                {
                    observer.select_deadline();
                }
                return Err(error);
            }
            let mut progress = self.read(stream);
            if !self.exited {
                if let Some(result) = self.reader {
                    result?;
                }
                if let Some(status) = status().map_err(|_| ParserError::Io)? {
                    if status.code() == Some(WORKER_ALLOCATION_LIMIT_EXIT_CODE) {
                        return Err(ParserError::AllocationLimit);
                    }
                    if !status.success() {
                        return Err(ParserError::WorkerFailed);
                    }
                    self.exited = true;
                    // A phase transition must not consume a second I/O budget this tick.
                    return Ok(Step::Pending(true));
                }
            }
            progress |= self.write(stream, input);
            // Writer failure still closes input, without turning the error into success.
            progress |= self.close_input(stream, input.len());
            if self.exited {
                control(deadline, cancelled)?;
                let Some(writer) = self.writer else {
                    return Ok(Step::Pending(progress));
                };
                writer?;
                control(deadline, cancelled)?;
                if let Some(reader) = self.reader {
                    reader?;
                    return Ok(Step::Complete(std::mem::take(&mut self.output)));
                }
            }
            Ok(Step::Pending(progress))
        }
    }

    pub(super) fn supervise(
        mut command: Command,
        normalized: &str,
        cancelled: &AtomicBool,
        deadline: Instant,
        mut observer: Option<&mut DeadlineObserver>,
        lease: &mut admission::Lease<'_>,
        cleanup: &mut (admission::Cleanup, bool),
    ) -> Result<Vec<u8>, ParserError> {
        let (parent, child_stream) = UnixStream::pair().map_err(|_| ParserError::Io)?;
        parent.set_nonblocking(true).map_err(|_| ParserError::Io)?;
        let child_output = child_stream.try_clone().map_err(|_| ParserError::Io)?;
        command
            .stdin(Stdio::from(OwnedFd::from(child_stream)))
            .stdout(Stdio::from(OwnedFd::from(child_output)))
            .env_clear();
        cleanup.1 = false;
        lease.spawn(&mut command)?;
        drop(command);
        let result = {
            // Close the parent endpoint before cleanup on both return and unwind.
            let mut stream = parent;
            let mut pump = Pump::default();
            (|| loop {
                let step = pump.step(
                    &mut stream,
                    normalized.as_bytes(),
                    deadline,
                    cancelled,
                    observer.as_deref_mut(),
                    || lease.status(),
                )?;
                match step {
                    Step::Complete(bytes) => return Ok(bytes),
                    Step::Pending(false) => thread::sleep(Duration::from_millis(2)),
                    Step::Pending(true) => {}
                }
            })()
        };
        cleanup.0 = lease.cleanup();
        cleanup.1 = cleanup.0 == admission::Cleanup::Confirmed;
        admission::select_result(result, cleanup.0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{collections::VecDeque, os::unix::process::ExitStatusExt};

        #[derive(Default)]
        struct Script {
            reads: VecDeque<io::Result<Vec<u8>>>,
            writes: VecDeque<io::Result<usize>>,
            closes: VecDeque<io::Result<()>>,
            sent: Vec<u8>,
            calls: [usize; 3],
            cancel_on: Option<(usize, Arc<AtomicBool>)>,
        }
        impl Script {
            fn signal_cancellation(&self, operation: usize) {
                if let Some((selected, cancelled)) = &self.cancel_on
                    && *selected == operation
                {
                    cancelled.store(true, Ordering::Relaxed);
                }
            }
        }
        impl Read for Script {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                assert!(!buffer.is_empty() && buffer.len() <= CHUNK);
                self.calls[0] += 1;
                self.signal_cancellation(0);
                let bytes = self
                    .reads
                    .pop_front()
                    .unwrap_or_else(|| Err(io::ErrorKind::WouldBlock.into()))?;
                assert!(bytes.len() <= buffer.len());
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
        }
        impl Write for Script {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                assert!(!bytes.is_empty() && bytes.len() <= CHUNK);
                self.calls[1] += 1;
                self.signal_cancellation(1);
                let count = self.writes.pop_front().unwrap_or(Ok(bytes.len()))?;
                assert!(count <= bytes.len());
                self.sent.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            fn flush(&mut self) -> io::Result<()> {
                unreachable!()
            }
        }
        impl Exchange for Script {
            fn close_write(&mut self) -> io::Result<()> {
                self.calls[2] += 1;
                self.signal_cancellation(2);
                self.closes.pop_front().unwrap_or(Ok(()))
            }
        }
        fn exit(code: i32) -> std::process::ExitStatus {
            std::process::ExitStatus::from_raw(code << 8)
        }
        fn step(
            pump: &mut Pump,
            script: &mut Script,
            input: &[u8],
            deadline: Instant,
            status: Option<i32>,
        ) -> Result<Step, ParserError> {
            pump.step(
                script,
                input,
                deadline,
                &AtomicBool::new(false),
                None,
                || Ok(status.map(exit)),
            )
        }
        fn error(result: Result<Step, ParserError>) -> ParserError {
            match result {
                Err(error) => error,
                _ => panic!("expected rejected step"),
            }
        }

        #[test]
        fn post_success_capacity_failure_defers_to_writer_and_controls() {
            let deadline = Instant::now() + TIMEOUT;
            for cancelled_on_write in [false, true] {
                let cancelled = Arc::new(AtomicBool::new(false));
                let mut pump = Pump {
                    exited: true,
                    ..Pump::default()
                };
                assert_eq!(pump.reserve_output_room(usize::MAX), Err(ParserError::Io));
                assert_eq!(pump.output.capacity(), 0);
                pump.output = Vec::with_capacity(8_388_611);
                pump.append(b"x");
                assert_eq!(pump.reader, Some(Err(ParserError::Io)));
                assert_eq!(pump.output.capacity(), 0);
                let mut script = Script {
                    writes: [Err(io::ErrorKind::WouldBlock.into())].into(),
                    ..Script::default()
                };
                assert!(matches!(
                    pump.step(&mut script, b"x", deadline, &cancelled, None, || panic!(
                        "post-success never checks status"
                    )),
                    Ok(Step::Pending(false))
                ));
                assert_eq!(script.calls, [0, 1, 0]);
                assert_eq!(pump.writer, None);
                assert_eq!(pump.reader, Some(Err(ParserError::Io)));
                script
                    .writes
                    .push_back(Err(io::ErrorKind::BrokenPipe.into()));
                if cancelled_on_write {
                    script.cancel_on = Some((1, Arc::clone(&cancelled)));
                }
                let selected = pump.step(&mut script, b"x", deadline, &cancelled, None, || {
                    panic!("post-success never checks status")
                });
                assert_eq!(
                    error(selected),
                    if cancelled_on_write {
                        ParserError::Cancelled
                    } else {
                        ParserError::Io
                    }
                );
                assert_eq!(script.calls, [0, 2, 1]);
                assert_eq!(pump.writer, Some(Err(ParserError::Io)));
                assert_eq!(pump.input_close, Some(Ok(())));
                assert_eq!(pump.reader, Some(Err(ParserError::Io)));
            }
        }

        #[test]
        fn productive_ticks_bound_both_directions_and_success_starts_a_new_tick() {
            let deadline = Instant::now() + TIMEOUT;
            let input = vec![b'x'; MAX_SOURCE_BYTES];
            let mut script = Script::default();
            script.reads.push_back(Ok(vec![b'x'; CHUNK]));
            let mut pump = Pump::default();
            assert!(matches!(
                step(&mut pump, &mut script, &input, deadline, None),
                Ok(Step::Pending(true))
            ));
            assert_eq!(script.calls, [1, 1, 0]);
            assert_eq!((pump.offset, pump.output.len()), (CHUNK, CHUNK));
            assert!(matches!(
                step(&mut pump, &mut script, &input, deadline, Some(0)),
                Ok(Step::Pending(true))
            ));
            assert_eq!(script.calls, [2, 1, 0]);
            assert!(pump.exited);
        }

        #[test]
        fn partial_utf8_and_pending_operations_preserve_bytes_and_tick_budgets() {
            let deadline = Instant::now() + TIMEOUT;
            let input = "utf8-😀".as_bytes();
            let mut script = Script::default();
            script
                .reads
                .push_back(Err(io::ErrorKind::Interrupted.into()));
            script
                .writes
                .extend([Err(io::ErrorKind::WouldBlock.into()), Ok(6), Ok(3)]);
            script.closes.extend([
                Err(io::ErrorKind::Interrupted.into()),
                Err(io::ErrorKind::WouldBlock.into()),
                Ok(()),
            ]);
            let mut pump = Pump::default();
            assert!(matches!(
                step(&mut pump, &mut script, input, deadline, None),
                Ok(Step::Pending(false))
            ));
            assert_eq!((pump.offset, script.calls), (0, [1, 1, 0]));
            for _ in 0..4 {
                step(&mut pump, &mut script, input, deadline, None).unwrap();
            }
            assert_eq!(script.sent, input);
            assert_eq!(script.calls, [5, 3, 3]);
            assert_eq!(pump.writer, Some(Ok(())));
            assert_eq!(pump.input_close, Some(Ok(())));
            step(&mut pump, &mut script, input, deadline, None).unwrap();
            assert_eq!((script.calls[1], script.calls[2]), (3, 3));
        }

        #[test]
        fn failed_writer_half_closes_without_becoming_success() {
            for failure in [Ok(0), Err(io::ErrorKind::BrokenPipe.into())] {
                let deadline = Instant::now() + TIMEOUT;
                let mut script = Script::default();
                script.writes.push_back(failure);
                script
                    .closes
                    .extend([Err(io::ErrorKind::Interrupted.into()), Ok(())]);
                let mut pump = Pump::default();
                step(&mut pump, &mut script, b"writer-error", deadline, None).unwrap();
                assert_eq!(pump.writer, Some(Err(ParserError::Io)));
                assert_eq!(pump.input_close, None);
                step(&mut pump, &mut script, b"writer-error", deadline, None).unwrap();
                assert_eq!(pump.input_close, Some(Ok(())));
                // Only input EOF makes this scripted child's successful exit ready.
                assert_eq!(script.calls, [2, 1, 2]);
                step(&mut pump, &mut script, b"writer-error", deadline, Some(0)).unwrap();
                assert_eq!(
                    error(step(
                        &mut pump,
                        &mut script,
                        b"writer-error",
                        deadline,
                        None
                    )),
                    ParserError::Io
                );
                assert_eq!((script.calls[1], script.calls[2]), (1, 2));
            }
        }

        #[test]
        fn output_cap_requires_an_extra_byte_or_eof_and_stops_reading() {
            for overflow in [false, true] {
                let mut pump = Pump {
                    output: vec![b'x'; MAX_OUTPUT_BYTES],
                    ..Pump::default()
                };
                let mut script = Script::default();
                script
                    .reads
                    .push_back(Ok(if overflow { vec![b'x'] } else { vec![] }));
                assert!(pump.read(&mut script));
                assert_eq!(
                    pump.reader,
                    Some(if overflow {
                        Err(ParserError::OutputLimit)
                    } else {
                        Ok(())
                    })
                );
                assert_eq!(pump.output.len(), MAX_OUTPUT_BYTES + usize::from(overflow));
                assert!(!pump.read(&mut script));
                assert_eq!(script.calls[0], 1);
            }
        }

        #[test]
        fn read_status_and_deferred_writer_error_priorities_are_preserved() {
            let deadline = Instant::now() + TIMEOUT;
            for code in [0, 1, 86, 87] {
                let mut pump = Pump {
                    output: vec![b'x'; MAX_OUTPUT_BYTES],
                    ..Pump::default()
                };
                let mut script = Script::default();
                script.reads.push_back(Ok(vec![b'x']));
                assert_eq!(
                    error(step(&mut pump, &mut script, b"", deadline, Some(code))),
                    ParserError::OutputLimit
                );
                assert_eq!(script.calls, [1, 0, 0]);
            }
            for code in [1, 86, 87] {
                let mut pump = Pump {
                    writer: Some(Err(ParserError::Io)),
                    ..Pump::default()
                };
                let mut script = Script::default();
                script.reads.push_back(Ok(vec![]));
                assert_eq!(
                    error(step(&mut pump, &mut script, b"", deadline, Some(code))),
                    if code == 86 {
                        ParserError::AllocationLimit
                    } else {
                        ParserError::WorkerFailed
                    }
                );
            }
            for writer in [Ok(()), Err(ParserError::Io)] {
                let mut pump = Pump {
                    exited: true,
                    writer: Some(writer),
                    input_close: Some(Ok(())),
                    output: vec![b'x'; MAX_OUTPUT_BYTES],
                    ..Pump::default()
                };
                let mut script = Script::default();
                script.reads.push_back(Ok(vec![b'x']));
                assert_eq!(
                    error(step(&mut pump, &mut script, b"", deadline, None)),
                    if writer.is_err() {
                        ParserError::Io
                    } else {
                        ParserError::OutputLimit
                    }
                );
            }
        }

        #[test]
        fn controls_precede_readiness_and_only_running_deadline_sends() {
            for exited in [false, true] {
                for cancel in [false, true] {
                    let (socket, peer) = std::os::unix::net::UnixDatagram::pair().unwrap();
                    socket.set_nonblocking(true).unwrap();
                    peer.set_nonblocking(true).unwrap();
                    let mut observer = DeadlineObserver {
                        socket,
                        evidence: DeadlineEvidence::default(),
                    };
                    let mut pump = Pump {
                        exited,
                        ..Pump::default()
                    };
                    let mut script = Script::default();
                    let result = pump.step(
                        &mut script,
                        b"",
                        Instant::now(),
                        &AtomicBool::new(cancel),
                        Some(&mut observer),
                        || panic!("control must precede status"),
                    );
                    assert_eq!(
                        error(result),
                        if cancel {
                            ParserError::Cancelled
                        } else {
                            ParserError::Timeout
                        }
                    );
                    assert_eq!(script.calls, [0, 0, 0]);
                    let selected = !exited && !cancel;
                    assert_eq!(observer.evidence.deadline_selected, selected);
                    let mut bytes = [0; 17];
                    if selected {
                        assert_eq!(peer.recv(&mut bytes).unwrap(), 16);
                        assert_eq!(&bytes[..16], b"KTLSP-DUE-v1!!!!");
                    }
                    assert_eq!(
                        peer.recv(&mut bytes).unwrap_err().kind(),
                        io::ErrorKind::WouldBlock
                    );
                }
            }
        }

        #[test]
        fn terminal_read_and_status_errors_stop_before_write_or_retry() {
            let deadline = Instant::now() + TIMEOUT;
            let mut pump = Pump::default();
            let mut script = Script::default();
            script.reads.push_back(Err(io::ErrorKind::Other.into()));
            let mut polls = 0;
            assert_eq!(
                error(pump.step(
                    &mut script,
                    b"writer-error",
                    deadline,
                    &AtomicBool::new(false),
                    None,
                    || {
                        polls += 1;
                        Ok(Some(exit(86)))
                    }
                )),
                ParserError::Io
            );
            assert_eq!(polls, 0);
            assert_eq!(script.calls, [1, 0, 0]);
            let mut pump = Pump::default();
            let mut script = Script::default();
            assert_eq!(
                error(pump.step(
                    &mut script,
                    b"writer-error",
                    deadline,
                    &AtomicBool::new(false),
                    None,
                    || {
                        polls += 1;
                        Err(io::ErrorKind::Interrupted.into())
                    }
                )),
                ParserError::Io
            );
            assert_eq!(polls, 1);
            assert_eq!(script.calls, [1, 0, 0]);
        }

        #[test]
        fn post_success_cancellation_during_io_precedes_error_or_bytes() {
            for operation in 0..3 {
                for writer_error in [false, true] {
                    let cancelled = Arc::new(AtomicBool::new(false));
                    let mut script = Script {
                        cancel_on: Some((operation, Arc::clone(&cancelled))),
                        ..Script::default()
                    };
                    script.reads.push_back(Ok(vec![]));
                    if writer_error {
                        script
                            .writes
                            .push_back(Err(io::ErrorKind::BrokenPipe.into()));
                    }
                    let (socket, peer) = std::os::unix::net::UnixDatagram::pair().unwrap();
                    socket.set_nonblocking(true).unwrap();
                    peer.set_nonblocking(true).unwrap();
                    let mut observer = DeadlineObserver {
                        socket,
                        evidence: DeadlineEvidence::default(),
                    };
                    let mut pump = Pump {
                        exited: true,
                        ..Pump::default()
                    };
                    assert_eq!(
                        error(pump.step(
                            &mut script,
                            b"healthy",
                            Instant::now() + TIMEOUT,
                            &cancelled,
                            Some(&mut observer),
                            || panic!("already reaped")
                        )),
                        ParserError::Cancelled
                    );
                    assert_eq!(script.calls, [1, 1, 1]);
                    assert!(!observer.evidence.deadline_selected);
                    assert_eq!(
                        peer.recv(&mut [0; 17]).unwrap_err().kind(),
                        io::ErrorKind::WouldBlock
                    );
                }
            }
        }

        #[test]
        fn success_requires_input_half_close_and_preserves_collected_output() {
            let deadline = Instant::now() + TIMEOUT;
            let mut script = Script::default();
            script.reads.extend([Ok(b"ok".to_vec()), Ok(vec![])]);
            script
                .closes
                .extend([Err(io::ErrorKind::WouldBlock.into()), Ok(())]);
            let mut pump = Pump::default();
            step(&mut pump, &mut script, b"", deadline, None).unwrap();
            assert_eq!(pump.writer, None);
            step(&mut pump, &mut script, b"", deadline, Some(0)).unwrap();
            assert_eq!(script.calls, [2, 0, 1]);
            match step(&mut pump, &mut script, b"", deadline, None).unwrap() {
                Step::Complete(bytes) => assert_eq!(bytes, b"ok"),
                _ => panic!("expected complete exchange"),
            }
            assert_eq!(script.calls, [2, 0, 2]);
            let mut pump = Pump::default();
            let mut script = Script::default();
            script
                .closes
                .push_back(Err(io::ErrorKind::NotConnected.into()));
            step(&mut pump, &mut script, b"", deadline, None).unwrap();
            assert_eq!(pump.writer, Some(Err(ParserError::Io)));
            assert_eq!(pump.input_close, Some(Err(ParserError::Io)));
            step(&mut pump, &mut script, b"", deadline, Some(0)).unwrap();
            assert_eq!(
                error(step(&mut pump, &mut script, b"", deadline, None)),
                ParserError::Io
            );
            assert_eq!(script.calls[2], 1);
        }
    }
}

#[cfg(not(unix))]
fn supervise_piped(
    mut command: Command,
    normalized: &str,
    cancelled: &AtomicBool,
    deadline: Instant,
    cleanup_status: &mut bool,
) -> Result<Vec<u8>, ParserError> {
    *cleanup_status = false;
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env_clear()
        .spawn()
        .map_err(|_| ParserError::Io)?;
    drop(command);
    thread::scope(|scope| {
        let mut guard = ChildGuard {
            child,
            reaped: false,
        };
        let result = (|| {
            let mut stdin = guard.child.stdin.take().ok_or(ParserError::Io)?;
            let stdout = guard.child.stdout.take().ok_or(ParserError::Io)?;
            let input = normalized;
            let writer = thread::Builder::new()
                .spawn_scoped(scope, move || stdin.write_all(input.as_bytes()))
                .map_err(|_| ParserError::Io)?;
            let (sender, receiver) = mpsc::sync_channel(1);
            let reader = thread::Builder::new()
                .spawn_scoped(scope, move || {
                    let mut bytes = Vec::new();
                    let result = stdout
                        .take(MAX_OUTPUT_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| ParserError::Io)
                        .and({
                            if bytes.len() > MAX_OUTPUT_BYTES {
                                Err(ParserError::OutputLimit)
                            } else {
                                Ok(bytes)
                            }
                        });
                    let _ = sender.send(result);
                })
                .map_err(|_| ParserError::Io)?;
            let mut output = None;
            let status = loop {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(ParserError::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(ParserError::Timeout);
                }
                if output.is_none() {
                    match receiver.try_recv() {
                        Ok(result) => output = Some(result?),
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => return Err(ParserError::Io),
                    }
                }
                if let Some(status) = guard.child.try_wait().map_err(|_| ParserError::Io)? {
                    guard.reaped = true;
                    break status;
                }
                thread::sleep(Duration::from_millis(2));
            };
            if status.code() == Some(WORKER_ALLOCATION_LIMIT_EXIT_CODE) {
                return Err(ParserError::AllocationLimit);
            }
            if !status.success() {
                return Err(ParserError::WorkerFailed);
            }
            finish_io(writer, reader, receiver, output, deadline, cancelled)
        })();
        // Check cleanup before the scope joins potentially blocked I/O threads.
        guard.finish()?;
        *cleanup_status = true;
        result
    })
}
#[cfg(any(not(unix), test))]
fn join_io<T>(
    handle: thread::ScopedJoinHandle<'_, T>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<T, ParserError> {
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(ParserError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(ParserError::Timeout);
        }
        if handle.is_finished() {
            return handle.join().map_err(|_| ParserError::Io);
        }
        thread::sleep(Duration::from_millis(2));
    }
}
#[cfg(any(not(unix), test))]
fn finish_io(
    writer: thread::ScopedJoinHandle<'_, std::io::Result<()>>,
    reader: thread::ScopedJoinHandle<'_, ()>,
    receiver: mpsc::Receiver<Result<Vec<u8>, ParserError>>,
    output: Option<Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, ParserError> {
    join_io(writer, deadline, cancelled)?.map_err(|_| ParserError::Io)?;
    join_io(reader, deadline, cancelled)?;
    output
        .map(Ok)
        .unwrap_or_else(|| receiver.try_recv().map_err(|_| ParserError::Io)?)
}
#[cfg(not(unix))]
struct ChildGuard {
    child: Child,
    reaped: bool,
}
#[cfg(not(unix))]
impl ChildGuard {
    fn finish(&mut self) -> Result<(), ParserError> {
        if !self.reaped {
            let _ = self.child.kill();
            self.child.wait().map_err(|_| ParserError::Io)?;
            self.reaped = true;
        }
        Ok(())
    }
}
#[cfg(not(unix))]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Conservative raw delimiter depth, including strings/comments and ambiguous `<`.
/// shortcut: can reject valid text, replace with accepted grammar-aware preflight in T007 follow-up.
#[doc(hidden)]
pub fn preflight(source: &str) -> Result<(), ParserError> {
    let mut depth = 0usize;
    for b in source.bytes() {
        match b {
            b'(' | b'[' | b'{' | b'<' => {
                depth += 1;
                if depth > MAX_NESTING {
                    return Err(ParserError::NestingLimit);
                }
            }
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}
fn normalize(source: &str) -> (String, Vec<usize>) {
    let mut bytes = Vec::with_capacity(source.len());
    let mut map = Vec::with_capacity(source.len() + 1);
    let mut offset = 0;
    while offset < source.len() {
        map.push(offset);
        let b = source.as_bytes()[offset];
        if b == b'\r' {
            bytes.push(b'\n');
            offset += 1;
            if source.as_bytes().get(offset) == Some(&b'\n') {
                offset += 1;
            }
        } else {
            bytes.push(b);
            offset += 1;
        }
    }
    map.push(source.len());
    (
        String::from_utf8(bytes).expect("newline replacement preserves UTF-8"),
        map,
    )
}

fn validate(
    source: &str,
    normalized: &str,
    map: &[usize],
    reply: Reply,
) -> Result<Parsed, ParserError> {
    let (mut elements, mut diagnostics) = match reply {
        Reply::Coverage => return Err(ParserError::Coverage),
        Reply::OutputLimit => return Err(ParserError::OutputLimit),
        Reply::NestingLimit => return Err(ParserError::NestingLimit),
        Reply::Tree {
            elements,
            diagnostics,
        } => (elements, diagnostics),
    };
    if elements.is_empty() || elements.len() > MAX_ELEMENTS || diagnostics.len() > MAX_DIAGNOSTICS {
        return Err(ParserError::InvalidTree);
    }
    // Enumerate only the trusted pinned enum range, never an unchecked wire discriminant.
    static KINDS: OnceLock<std::collections::HashMap<String, bool>> = OnceLock::new();
    let kinds = KINDS.get_or_init(|| {
        (0..=ktrs_syntax::SyntaxKind::DUMMY_HOLDER as u16)
            .map(|raw| {
                let kind = ktrs_syntax::SyntaxKind::from_raw(raw);
                (format!("{kind:?}"), kind.is_token())
            })
            .collect()
    });
    let mut child_ends: Vec<usize> = elements.iter().map(|e| e.start).collect();
    let mut error_spans = Vec::new();
    let mut ancestors: Vec<usize> = Vec::new();
    let mut cursor = 0;
    for (i, e) in elements.iter().enumerate() {
        while ancestors
            .last()
            .is_some_and(|&p| elements[p].subtree_end == i)
        {
            ancestors.pop();
        }
        if e.start > e.end
            || e.end > normalized.len()
            || !normalized.is_char_boundary(e.start)
            || !normalized.is_char_boundary(e.end)
            || kinds.get(&e.kind) != Some(&e.token)
            || e.subtree_end <= i
            || e.subtree_end > elements.len()
            || e.parent != ancestors.last().copied()
            || (i == 0
                && (e.start != 0
                    || e.end != normalized.len()
                    || e.subtree_end != elements.len()
                    || e.kind != "KT_FILE"
                    || e.token))
        {
            return Err(ParserError::InvalidTree);
        }
        if let Some(&p) = ancestors.last() {
            let parent = &elements[p];
            if e.start != child_ends[p] || e.end > parent.end || e.subtree_end > parent.subtree_end
            {
                return Err(ParserError::InvalidTree);
            }
            child_ends[p] = e.end;
        }
        if ancestors.len() + 1 > MAX_NESTING {
            return Err(ParserError::NestingLimit);
        }
        if e.kind == "ERROR_ELEMENT" {
            error_spans.push((e.start, e.end));
        }
        if e.token {
            if e.subtree_end != i + 1 || e.start != cursor || e.start == e.end {
                return Err(ParserError::Coverage);
            }
            cursor = e.end;
        } else if e.subtree_end > i + 1 {
            ancestors.push(i);
        }
    }
    if cursor != normalized.len() {
        return Err(ParserError::Coverage);
    }
    if elements
        .iter()
        .enumerate()
        .any(|(i, e)| !e.token && child_ends[i] != e.end)
        || error_spans.len() != diagnostics.len()
    {
        return Err(ParserError::InvalidTree);
    }
    for (d, span) in diagnostics.iter_mut().zip(error_spans) {
        if d.start > d.end
            || d.end > normalized.len()
            || !normalized.is_char_boundary(d.start)
            || !normalized.is_char_boundary(d.end)
            || d.message.is_empty()
            || d.message.len() > 4096
            || (d.start, d.end) != span
        {
            return Err(ParserError::InvalidTree);
        }
        d.start = map[d.start];
        d.end = map[d.end];
    }
    for e in &mut elements {
        e.start = map[e.start];
        e.end = map[e.end];
    }
    Ok(Parsed {
        source: Arc::from(source),
        elements,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending_io<T: Send>(
        work: impl FnOnce() -> T + Send,
        deadline_after: Duration,
        cancel: bool,
    ) -> Result<(), ParserError> {
        let (release, gate) = mpsc::channel::<()>();
        let (started, ready) = mpsc::channel();
        let (selected, result) = mpsc::channel();
        let cancelled = AtomicBool::new(false);
        thread::scope(|controller| {
            let task = controller.spawn(move || {
                thread::scope(|scope| {
                    let handle = scope.spawn(move || {
                        let value = work();
                        started.send(()).unwrap();
                        gate.recv().unwrap();
                        value
                    });
                    ready.recv().unwrap();
                    cancelled.store(cancel, Ordering::Relaxed);
                    let outcome = join_io(handle, Instant::now() + deadline_after, &cancelled);
                    let _ = selected.send(outcome.map(|_| ()));
                });
            });
            let outcome = result.recv_timeout(Duration::from_secs(3));
            // Release even when a broken join blocks result selection.
            let _ = release.send(());
            task.join().unwrap();
            outcome.expect("I/O result must be selected before releasing the pending thread")
        })
    }
    #[cfg(unix)]
    #[test]
    fn generic_copy_wire_is_bounded_and_rejects_invalid_or_duplicate_records() {
        use std::os::unix::net::UnixDatagram;
        let source = "class C<T fun f() = 1";
        let record = GenericCopyRecord {
            status: 1,
            source_len: source.len() as u32,
            active_events: 1,
            active_layers: 1,
            production_bound: 10,
            independent_min: 10,
            window_start: 10,
            actual_copy_end: 13,
            next_start: 13,
            next_end: 14,
            next_fits_other_caps: 1,
            reserved: 0,
        };
        let bytes = record.to_bytes();
        assert_eq!(GenericCopyRecord::from_bytes(&bytes, source), Some(record));
        let zero = GenericCopyRecord {
            source_len: source.len() as u32,
            ..GenericCopyRecord::default()
        };
        assert_eq!(
            GenericCopyRecord::from_bytes(&zero.to_bytes(), source),
            Some(zero)
        );
        let missing_next = GenericCopyRecord {
            next_start: 0,
            next_end: 0,
            next_fits_other_caps: 0,
            ..record
        };
        assert_eq!(
            GenericCopyRecord::from_bytes(&missing_next.to_bytes(), source),
            None
        );
        // Machinery only: numeric wire validation does not prove the live bound was obeyed.
        for (word, value) in [
            (0, 2),
            (0, 3),
            (1, 0),
            (2, 0),
            (2, 2),
            (3, 0),
            (4, (-2_i32) as u32),
            (5, u32::MAX),
            (6, 13),
            (7, 100),
            (8, 12),
            (9, 100),
            (10, 2),
            (11, 1),
        ] {
            let mut invalid = bytes;
            invalid[16 + word * 4..20 + word * 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(GenericCopyRecord::from_bytes(&invalid, source), None);
        }
        for packet in [b"".as_slice(), b"unknown", &bytes[..63], &[0; 65]] {
            assert_eq!(GenericCopyRecord::from_bytes(packet, source), None);
        }
        for duplicate in [false, true] {
            let (parent, worker) = UnixDatagram::pair().unwrap();
            parent.set_nonblocking(true).unwrap();
            worker.set_nonblocking(true).unwrap();
            worker.send(&bytes).unwrap();
            if duplicate {
                worker.send(b"").unwrap();
            }
            let evidence = GenericCopyObserver(parent).finish(source);
            assert_eq!(
                evidence,
                if duplicate {
                    GenericCopyEvidence::Invalid
                } else {
                    GenericCopyEvidence::Complete(record)
                }
            );
        }
        let (parent, _worker) = UnixDatagram::pair().unwrap();
        parent.set_nonblocking(true).unwrap();
        assert_eq!(
            GenericCopyObserver(parent).finish(source),
            GenericCopyEvidence::Missing
        );
        for packet in [b"".as_slice(), &bytes[..63], &[0; 65]] {
            let (parent, worker) = UnixDatagram::pair().unwrap();
            parent.set_nonblocking(true).unwrap();
            worker.send(packet).unwrap();
            assert_eq!(
                GenericCopyObserver(parent).finish(source),
                GenericCopyEvidence::Invalid
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn generic_copy_precancellation_does_not_launch_or_emit() {
        let parser = Parser::new("/no-such-b11-worker").unwrap();
        let snapshot = SourceSnapshot::new("b11-cancel", 1, FileKind::Source, "fun f()=0").unwrap();
        let (result, evidence) =
            parser.parse_snapshot_observing_generic_copy(&snapshot, &AtomicBool::new(true));
        assert_eq!(result.unwrap_err(), ParserError::Cancelled);
        assert_eq!(evidence, GenericCopyEvidence::Missing);
    }

    #[test]
    fn pending_io_checks_cancellation_and_deadline_before_join() {
        for (deadline, cancel, expected) in [
            (Duration::ZERO, false, ParserError::Timeout),
            (TIMEOUT, true, ParserError::Cancelled),
            (TIMEOUT, false, ParserError::Timeout),
        ] {
            assert_eq!(
                pending_io(|| Ok::<(), std::io::Error>(()), deadline, cancel),
                Err(expected)
            );
            assert_eq!(pending_io(|| (), deadline, cancel), Err(expected));
        }
    }
    #[test]
    fn output_receipt_does_not_mean_reader_thread_finished() {
        for (cancel, expected) in [
            (false, ParserError::Timeout),
            (true, ParserError::Cancelled),
        ] {
            let (sender, receiver) = mpsc::sync_channel(1);
            assert_eq!(
                pending_io(
                    move || sender.send(Ok::<_, ParserError>(b"ok".to_vec())).unwrap(),
                    if cancel { TIMEOUT } else { Duration::ZERO },
                    cancel,
                ),
                Err(expected)
            );
            assert_eq!(receiver.try_recv().unwrap(), Ok(b"ok".to_vec()));
        }
    }
    #[test]
    fn completed_io_preserves_error_order_and_output() {
        for (case, expected) in [
            ("writer-error", Err(ParserError::Io)),
            ("writer-panic", Err(ParserError::Io)),
            ("reader-panic", Err(ParserError::Io)),
            ("reader-error-panic", Err(ParserError::Io)),
            ("reader-error", Err(ParserError::OutputLimit)),
            ("missing-result", Err(ParserError::Io)),
            ("success", Ok(b"ok".to_vec())),
            ("buffered-success", Ok(b"ok".to_vec())),
            ("cancel-ready", Err(ParserError::Cancelled)),
            ("timeout-ready", Err(ParserError::Timeout)),
        ] {
            let actual = thread::scope(|scope| {
                let writer = scope.spawn(move || match case {
                    "writer-error" => Err(std::io::Error::other("test writer failure")),
                    "writer-panic" => panic!("test writer panic"),
                    _ => Ok(()),
                });
                let (sender, receiver) = mpsc::sync_channel(1);
                let reader = scope.spawn(move || {
                    if case == "reader-panic" {
                        panic!("test reader panic");
                    }
                    if case != "missing-result" && case != "buffered-success" {
                        let output = if matches!(
                            case,
                            "writer-error" | "reader-error" | "reader-error-panic"
                        ) {
                            Err(ParserError::OutputLimit)
                        } else {
                            Ok(b"ok".to_vec())
                        };
                        sender.send(output).unwrap();
                    }
                    if case == "reader-error-panic" {
                        panic!("test reader panic after output error");
                    }
                });
                while !writer.is_finished() || !reader.is_finished() {
                    thread::yield_now();
                }
                finish_io(
                    writer,
                    reader,
                    receiver,
                    (case == "buffered-success").then(|| b"ok".to_vec()),
                    Instant::now()
                        + if matches!(case, "cancel-ready" | "timeout-ready") {
                            Duration::ZERO
                        } else {
                            TIMEOUT
                        },
                    &AtomicBool::new(case == "cancel-ready"),
                )
            });
            assert_eq!(actual, expected, "{case}");
        }
    }
    #[test]
    fn normalization_preserves_original_boundaries() {
        let source = "😀\r\n\r\nx\ry\n";
        let (normalized, map) = normalize(source);
        assert_eq!(normalized, "😀\n\nx\ny\n");
        assert_eq!(map[4], 4);
        assert_eq!(map[5], 6);
        assert_eq!(map[6], 8);
        assert_eq!(map[normalized.len()], source.len());
        assert!(!normalized.is_char_boundary(1));
    }
    #[test]
    fn rejects_untrusted_tree_shapes_and_spans() {
        let root = Element {
            kind: "KT_FILE".into(),
            start: 0,
            end: 1,
            parent: None,
            subtree_end: 2,
            token: false,
        };
        let token = Element {
            kind: "IDENTIFIER".into(),
            start: 0,
            end: 1,
            parent: Some(0),
            subtree_end: 2,
            token: true,
        };
        let reply = |elements| Reply::Tree {
            elements,
            diagnostics: vec![],
        };
        assert!(validate("a", "a", &[0, 1], reply(vec![root.clone(), token.clone()])).is_ok());
        for invalid in [
            Element {
                parent: Some(1),
                ..token.clone()
            },
            Element {
                end: 2,
                ..token.clone()
            },
            Element {
                subtree_end: usize::MAX,
                ..token.clone()
            },
            Element {
                start: 1,
                ..token.clone()
            },
        ] {
            assert!(validate("a", "a", &[0, 1], reply(vec![root.clone(), invalid])).is_err());
        }
        let invalid = Element {
            start: 1,
            end: 4,
            ..token
        };
        assert!(
            validate(
                "😀",
                "😀",
                &[0, 1, 2, 3, 4],
                reply(vec![Element { end: 4, ..root }, invalid])
            )
            .is_err()
        );
    }
    #[test]
    fn bounded_decode_rejects_truncation_unknown_variants_and_counts() {
        for bytes in [
            br#"{"Tree":{"elements":["#.as_slice(),
            br#""Unsupported""#.as_slice(),
            br#"{"Tree":{"elements":0,"diagnostics":[]}}"#.as_slice(),
        ] {
            assert!(serde_json::from_slice::<Reply>(bytes).is_err());
        }
        let element = Element {
            kind: "X".into(),
            start: 0,
            end: 0,
            parent: None,
            subtree_end: 1,
            token: false,
        };
        let too_many_elements = Reply::Tree {
            elements: vec![element; MAX_ELEMENTS + 1],
            diagnostics: vec![],
        };
        let too_many_diagnostics = Reply::Tree {
            elements: vec![],
            diagnostics: vec![
                Diagnostic {
                    start: 0,
                    end: 0,
                    message: String::new()
                };
                MAX_DIAGNOSTICS + 1
            ],
        };
        for reply in [too_many_elements, too_many_diagnostics] {
            let bytes = serde_json::to_vec(&reply).unwrap();
            assert!(bytes.len() < MAX_OUTPUT_BYTES);
            assert!(serde_json::from_slice::<Reply>(&bytes).is_err());
        }
    }
    #[test]
    fn immediate_parent_and_subtree_containment_are_required() {
        let elements = vec![
            Element {
                kind: "KT_FILE".into(),
                start: 0,
                end: 2,
                parent: None,
                subtree_end: 4,
                token: false,
            },
            Element {
                kind: "REFERENCE_EXPRESSION".into(),
                start: 0,
                end: 1,
                parent: Some(0),
                subtree_end: 3,
                token: false,
            },
            Element {
                kind: "IDENTIFIER".into(),
                start: 0,
                end: 1,
                parent: Some(1),
                subtree_end: 3,
                token: true,
            },
            Element {
                kind: "IDENTIFIER".into(),
                start: 1,
                end: 2,
                parent: Some(0),
                subtree_end: 4,
                token: true,
            },
        ];
        let check = |elements| {
            validate(
                "ab",
                "ab",
                &[0, 1, 2],
                Reply::Tree {
                    elements,
                    diagnostics: vec![],
                },
            )
        };
        assert!(check(elements.clone()).is_ok());
        for mutation in 0..3 {
            let mut invalid = elements.clone();
            match mutation {
                0 => invalid[2].end = 2,
                1 => invalid[2].subtree_end = 4,
                _ => invalid[2].parent = Some(0),
            }
            assert_eq!(check(invalid).unwrap_err(), ParserError::InvalidTree);
        }
    }
    #[test]
    fn request_cleanup_reports_no_spawn_for_cancel_and_preflight_rejection() {
        let parser = Parser::new(std::env::current_exe().unwrap()).unwrap();
        let snapshot =
            SourceSnapshot::new("cancel-before-spawn", 1, FileKind::Source, "class C").unwrap();
        let (result, closed) =
            parser.parse_snapshot_with_cleanup(&snapshot, &AtomicBool::new(true));
        assert_eq!(result.unwrap_err(), ParserError::Cancelled);
        assert!(closed);
        let snapshot = SourceSnapshot::new(
            "preflight-before-spawn",
            1,
            FileKind::Source,
            "{".repeat(MAX_NESTING + 1),
        )
        .unwrap();
        let (result, closed) =
            parser.parse_snapshot_with_cleanup(&snapshot, &AtomicBool::new(false));
        assert_eq!(result.unwrap_err(), ParserError::NestingLimit);
        assert!(closed);
    }
    #[test]
    fn supervised_faults_terminate_and_allow_next_request() {
        let _functional = crate::functional_test::serial();
        for (mode, expected) in [
            ("hang", ParserError::Timeout),
            ("no-read", ParserError::Timeout),
            ("oversize", ParserError::OutputLimit),
            ("crash", ParserError::WorkerFailed),
            ("exit-failure", ParserError::WorkerFailed),
            ("exit-allocation", ParserError::AllocationLimit),
            ("exit-neighbor", ParserError::WorkerFailed),
        ] {
            #[cfg(unix)]
            let mut lease = admission::reserve().unwrap();
            #[cfg(unix)]
            let mut cleanup = (admission::Cleanup::Failed, true);
            #[cfg(not(unix))]
            let mut cleanup = true;
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "tests::fault_worker", "--ignored", "--nocapture"]);
            let source = if mode == "no-read" {
                "x".repeat(MAX_SOURCE_BYTES)
            } else {
                mode.into()
            };
            let start = Instant::now();
            let result = supervise(
                command,
                &source,
                &AtomicBool::new(false),
                #[cfg(not(unix))]
                &mut cleanup,
                #[cfg(unix)]
                None,
                #[cfg(unix)]
                false,
                #[cfg(unix)]
                &mut lease,
                #[cfg(unix)]
                &mut cleanup,
            );
            if mode == "crash" {
                // Abort completion can cross the deadline checked before exit status.
                assert!(matches!(
                    result,
                    Err(ParserError::WorkerFailed | ParserError::Timeout)
                ));
            } else {
                assert_eq!(result, Err(expected));
            }
            assert!(start.elapsed() < Duration::from_secs(3));
        }
        let cancel = AtomicBool::new(false);
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(30));
                cancel.store(true, Ordering::Relaxed);
            });
            #[cfg(unix)]
            let mut lease = admission::reserve().unwrap();
            #[cfg(unix)]
            let mut cleanup = (admission::Cleanup::Failed, true);
            #[cfg(not(unix))]
            let mut cleanup = true;
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "tests::fault_worker", "--ignored", "--nocapture"]);
            assert_eq!(
                supervise(
                    command,
                    "hang",
                    &cancel,
                    #[cfg(not(unix))]
                    &mut cleanup,
                    #[cfg(unix)]
                    None,
                    #[cfg(unix)]
                    false,
                    #[cfg(unix)]
                    &mut lease,
                    #[cfg(unix)]
                    &mut cleanup,
                ),
                Err(ParserError::Cancelled)
            );
        });
        #[cfg(unix)]
        let mut lease = admission::reserve().unwrap();
        #[cfg(unix)]
        let mut cleanup = (admission::Cleanup::Failed, true);
        #[cfg(not(unix))]
        let mut cleanup = true;
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "tests::fault_worker", "--ignored", "--nocapture"]);
        assert!(
            supervise(
                command,
                "healthy",
                &AtomicBool::new(false),
                #[cfg(not(unix))]
                &mut cleanup,
                #[cfg(unix)]
                None,
                #[cfg(unix)]
                false,
                #[cfg(unix)]
                &mut lease,
                #[cfg(unix)]
                &mut cleanup,
            )
            .is_ok()
        );
    }
    #[test]
    #[ignore = "subprocess fault fixture"]
    fn fault_worker() {
        let mut source = String::new();
        // The no-read fixture consumes one byte, then leaves the remaining input unread.
        let mut first = [0u8; 1];
        std::io::stdin().read_exact(&mut first).unwrap();
        if first[0] == b'x' {
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        source.push(first[0] as char);
        std::io::stdin().read_to_string(&mut source).unwrap();
        match source.as_str() {
            "hang" => loop {
                thread::sleep(Duration::from_secs(1));
            },
            "oversize" => {
                std::io::stdout()
                    .write_all(&vec![b'x'; MAX_OUTPUT_BYTES + 1])
                    .unwrap();
            }
            "crash" => std::process::abort(),
            "exit-failure" => std::process::exit(1),
            "exit-allocation" => std::process::exit(86),
            "exit-neighbor" => std::process::exit(87),
            _ => {}
        }
    }
}

#[cfg(all(test, unix))]
mod deadline_tests {
    use super::*;
    use std::os::unix::net::UnixDatagram;

    #[test]
    fn deadline_receipts_require_exact_challenge_ack_and_empty_sentinel() {
        for (packets, selected, sent, expected) in [
            (vec![], true, Some(Ok(16)), DeadlineReceipt::Missing),
            (
                vec![b"KTLSP-EOF-v1!!!!".to_vec()],
                true,
                Some(Ok(16)),
                DeadlineReceipt::CheckpointAfterDecision,
            ),
            (
                vec![b"KTLSP-EOF-v1!!!!".to_vec()],
                false,
                None,
                DeadlineReceipt::Invalid,
            ),
            (
                vec![],
                true,
                Some(Err(std::io::ErrorKind::WouldBlock)),
                DeadlineReceipt::Invalid,
            ),
            (vec![], true, Some(Ok(15)), DeadlineReceipt::Invalid),
            (vec![vec![]], true, Some(Ok(16)), DeadlineReceipt::Invalid),
            (
                vec![b"KTLSP-EOF-v1!!!".to_vec()],
                true,
                Some(Ok(16)),
                DeadlineReceipt::Invalid,
            ),
            (
                vec![b"KTLSP-EOF-v1!!!!!".to_vec()],
                true,
                Some(Ok(16)),
                DeadlineReceipt::Invalid,
            ),
            (
                vec![vec![b'X'; 4096]],
                true,
                Some(Ok(16)),
                DeadlineReceipt::Invalid,
            ),
            (
                vec![b"KTLSP-EOF-v1!!!!".to_vec(), vec![]],
                true,
                Some(Ok(16)),
                DeadlineReceipt::Invalid,
            ),
            (
                vec![b"KTLSP-EOF-v1!!!!".to_vec(), b"noise".to_vec()],
                true,
                Some(Ok(16)),
                DeadlineReceipt::Invalid,
            ),
        ] {
            let (socket, worker) = UnixDatagram::pair().unwrap();
            socket.set_nonblocking(true).unwrap();
            worker.set_nonblocking(true).unwrap();
            for packet in packets {
                worker.send(&packet).unwrap();
            }
            let observer = DeadlineObserver {
                socket,
                evidence: DeadlineEvidence {
                    deadline_selected: selected,
                    decision_send: sent,
                    receipt: DeadlineReceipt::Missing,
                },
            };
            assert_eq!(observer.finish().receipt, expected);
        }
    }

    #[test]
    fn failed_decision_send_is_not_a_witness() {
        let (socket, worker) = UnixDatagram::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        drop(worker);
        let mut observer = DeadlineObserver {
            socket,
            evidence: DeadlineEvidence::default(),
        };
        observer.select_deadline();
        assert!(observer.evidence.deadline_selected);
        assert!(matches!(observer.evidence.decision_send, Some(Err(_))));
        assert_eq!(observer.finish().receipt, DeadlineReceipt::Invalid);
    }
}
