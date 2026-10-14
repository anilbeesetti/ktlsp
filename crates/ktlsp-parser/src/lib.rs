//! Isolated, bounded parser adoption gate. No grammar or semantic parity claim.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

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

/// Caller supplies the trusted installed Rust worker's absolute path.
/// Each call owns exactly one child; concurrent calls need caller-side admission.
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
        let mut parsed = self.parse(snapshot.source(), snapshot.kind, cancelled)?;
        parsed.source = Arc::clone(&snapshot.source);
        Ok(SnapshotResult {
            snapshot: snapshot.clone(),
            parsed,
        })
    }
    /// Unversioned parser gate for standalone callers; editor consumers use parse_snapshot.
    pub fn parse(
        &self,
        source: &str,
        kind: FileKind,
        cancelled: &AtomicBool,
    ) -> Result<Parsed, ParserError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(ParserError::InputLimit);
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(ParserError::Cancelled);
        }
        preflight(source)?;
        let (normalized, map) = normalize(source);
        let mut command = Command::new(&self.worker);
        command.arg(match kind {
            FileKind::Source => "--source",
            FileKind::Script => "--script",
        });
        let bytes = supervise(command, &normalized, cancelled)?;
        if cancelled.load(Ordering::Relaxed) {
            return Err(ParserError::Cancelled);
        }
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| ParserError::InvalidTree)?;
        validate(source, &normalized, &map, reply)
    }
}
fn supervise(
    mut command: Command,
    normalized: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, ParserError> {
    let deadline = Instant::now() + TIMEOUT;
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear()
        .spawn()
        .map_err(|_| ParserError::Io)?;
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
        result
    })
}
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
struct ChildGuard {
    child: Child,
    reaped: bool,
}
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
        let (release, gate) = mpsc::channel();
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
    fn supervised_faults_terminate_and_allow_next_request() {
        for (mode, expected) in [
            ("hang", ParserError::Timeout),
            ("no-read", ParserError::Timeout),
            ("oversize", ParserError::OutputLimit),
            ("crash", ParserError::WorkerFailed),
            ("exit-failure", ParserError::WorkerFailed),
            ("exit-allocation", ParserError::AllocationLimit),
            ("exit-neighbor", ParserError::WorkerFailed),
        ] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "tests::fault_worker", "--ignored", "--nocapture"]);
            let source = if mode == "no-read" {
                "x".repeat(MAX_SOURCE_BYTES)
            } else {
                mode.into()
            };
            let start = Instant::now();
            let result = supervise(command, &source, &AtomicBool::new(false));
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
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "tests::fault_worker", "--ignored", "--nocapture"]);
            assert_eq!(
                supervise(command, "hang", &cancel),
                Err(ParserError::Cancelled)
            );
        });
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "tests::fault_worker", "--ignored", "--nocapture"]);
        assert!(supervise(command, "healthy", &AtomicBool::new(false)).is_ok());
    }
    #[test]
    #[ignore = "subprocess fault fixture"]
    fn fault_worker() {
        let mut source = String::new();
        // The no-read fixture fills the input pipe before the parent can close it.
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
