//! Opt-in fault-fixture evidence only; never compiled into the product.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Sample {
    pub read_bytes: usize,
    pub written_bytes: usize,
    pub reader: Option<Result<(), ParserError>>,
    pub writer: Option<Result<(), ParserError>>,
    pub input_close: Option<Result<(), ParserError>>,
    pub exited: bool,
    pub decision: Decision,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Decision {
    Pending(bool),
    Complete,
    Error(ParserError),
}

pub(super) struct Observation {
    start: Instant,
    spawned: Option<(Duration, Result<(), ParserError>)>,
    socket: Option<ObservationChannel>,
    setup_error: Option<std::io::ErrorKind>,
    samples: [Option<(Duration, Sample)>; 32],
    ticks: u64,
    idle: u64,
    first_read: Option<Duration>,
    first_write: Option<Duration>,
    input_closed: Option<Duration>,
    output_eof: Option<Duration>,
    child_exited: Option<Duration>,
    stages: [Option<(Duration, u8)>; 8],
    stage_count: usize,
    invalid_stages: u64,
}

fn eligible(command: &Command) -> bool {
    command.get_args().any(|arg| arg == "tests::fault_worker")
        && !command
            .get_args()
            .any(|arg| arg == "--observe-deadline-eof-v1" || arg == "--observe-generic-copy-v1")
}
impl Observation {
    fn new(start: Instant) -> Self {
        Self {
            start,
            spawned: None,
            socket: None,
            setup_error: None,
            samples: [None; 32],
            ticks: 0,
            idle: 0,
            first_read: None,
            first_write: None,
            input_closed: None,
            output_eof: None,
            child_exited: None,
            stages: [None; 8],
            stage_count: 0,
            invalid_stages: 0,
        }
    }
    pub(super) fn requested(command: &mut Command, deadline: Instant) -> Option<Self> {
        // Scope the opt-in to this known fault fixture, not real parser/source traffic.
        if std::env::var_os("KTLSP_NATIVE_IO_OBSERVE").as_deref() != Some(std::ffi::OsStr::new("1"))
            || !eligible(command)
        {
            return None;
        }
        let mut observation = Self::new(deadline - TIMEOUT);
        match ObservationChannel::new(command) {
            Ok(socket) => {
                command.env("KTLSP_FAULT_IO_OBSERVE", "1");
                observation.socket = Some(socket);
            }
            Err(error) => observation.setup_error = Some(error.kind()),
        }
        Some(observation)
    }
    pub(super) fn spawned(&mut self, result: Result<(), ParserError>) {
        self.spawned = Some((self.start.elapsed(), result));
    }
    fn drain_stages(&mut self) {
        let Some(socket) = &self.socket else {
            return;
        };
        // Fixed receive budget, fixed storage; unknown bytes are counted, never logged.
        for _ in 0..8 {
            let mut bytes = [0; 2];
            match socket.recv(&mut bytes) {
                Ok(1) if (1..=5).contains(&bytes[0]) && self.stage_count < self.stages.len() => {
                    self.stages[self.stage_count] = Some((self.start.elapsed(), bytes[0]));
                    self.stage_count += 1;
                }
                Ok(_) => self.invalid_stages = self.invalid_stages.saturating_add(1),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.invalid_stages = self.invalid_stages.saturating_add(1);
                    break;
                }
            }
        }
    }
    pub(super) fn tick(&mut self, sample: Sample) {
        let elapsed = self.start.elapsed();
        if sample.read_bytes > 0 {
            self.first_read.get_or_insert(elapsed);
        }
        if sample.written_bytes > 0 {
            self.first_write.get_or_insert(elapsed);
        }
        if sample.input_close == Some(Ok(())) {
            self.input_closed.get_or_insert(elapsed);
        }
        if sample.reader == Some(Ok(())) {
            self.output_eof.get_or_insert(elapsed);
        }
        if sample.exited {
            self.child_exited.get_or_insert(elapsed);
        }
        if matches!(sample.decision, Decision::Pending(false)) {
            self.idle = self.idle.saturating_add(1);
        }
        self.samples[(self.ticks % 32) as usize] = Some((elapsed, sample));
        self.ticks = self.ticks.saturating_add(1);
        self.drain_stages();
    }
    pub(super) fn finish(
        mut self,
        result: &Result<Vec<u8>, ParserError>,
        cleanup: admission::Cleanup,
        mut log: impl Write,
    ) {
        self.drain_stages();
        let _ = writeln!(
            log,
            "KTLSP_NATIVE_IO_V1 elapsed={:?} spawn={:?} setup={:?} ticks={} idle={} first_read={:?} first_write={:?} input_closed={:?} output_eof={:?} child_exited={:?} result={:?} cleanup={:?} stages={:?} invalid_stages={}",
            self.start.elapsed(),
            self.spawned,
            self.setup_error,
            self.ticks,
            self.idle,
            self.first_read,
            self.first_write,
            self.input_closed,
            self.output_eof,
            self.child_exited,
            result.as_ref().map(Vec::len),
            cleanup,
            self.stages,
            self.invalid_stages,
        );
        let retained = self.ticks.min(32);
        for index in self.ticks.saturating_sub(retained)..self.ticks {
            if let Some((elapsed, sample)) = self.samples[(index % 32) as usize] {
                // Explicit scalar fields ensure no source, output or error payload is exposed.
                let decision = match sample.decision {
                    Decision::Pending(progress) => ("pending", Some(progress), None),
                    Decision::Complete => ("complete", None, None),
                    Decision::Error(error) => ("error", None, Some(error)),
                };
                let _ = writeln!(
                    log,
                    "KTLSP_NATIVE_IO_V1 tick={index} elapsed={elapsed:?} read={} written={} reader={:?} writer={:?} input_close={:?} exited={} decision={decision:?}",
                    sample.read_bytes,
                    sample.written_bytes,
                    sample.reader,
                    sample.writer,
                    sample.input_close,
                    sample.exited,
                );
            }
        }
    }
}

pub(super) fn child_stage(stage: u8) {
    if std::env::var_os("KTLSP_FAULT_IO_OBSERVE").as_deref() == Some(std::ffi::OsStr::new("1")) {
        // Diagnostic send failure never changes the fixture/product result.
        let _ = std::io::stderr().write(&[stage]);
    }
}

#[test]
fn fixed_history_retains_terminal_decision_and_counts_idle_ticks() {
    let mut observation = Observation::new(Instant::now());
    for index in 0..65 {
        observation.tick(Sample {
            read_bytes: index,
            written_bytes: 8,
            reader: None,
            writer: Some(Ok(())),
            input_close: Some(Ok(())),
            exited: false,
            decision: if index == 64 {
                Decision::Error(ParserError::Timeout)
            } else {
                Decision::Pending(false)
            },
        });
    }
    assert_eq!((observation.ticks, observation.idle), (65, 64));
    assert_eq!(observation.samples.iter().flatten().count(), 32);
    let last = observation.samples[0].unwrap().1;
    assert_eq!(last.read_bytes, 64);
    assert!(matches!(
        last.decision,
        Decision::Error(ParserError::Timeout)
    ));
    assert!(observation.first_read.is_some());
    assert!(observation.first_write.is_some());
    assert!(observation.input_closed.is_some());
    assert!(observation.output_eof.is_none());
    assert!(observation.child_exited.is_none());
}

#[test]
fn child_stage_storage_and_receive_budget_are_bounded() {
    let socket = ObservationChannel::pair().unwrap();
    let worker = socket.peer.try_clone().unwrap();
    let mut observation = Observation::new(Instant::now());
    observation.socket = Some(socket);
    for packet in [&[1][..], &[1, 2], &[6], &[]] {
        worker.send(packet).unwrap();
    }
    observation.drain_stages();
    assert_eq!(observation.stage_count, 1);
    assert_eq!(observation.stages[0].unwrap().1, 1);
    assert_eq!(observation.invalid_stages, 3);
    for _ in 0..8 {
        worker.send(&[5]).unwrap();
    }
    observation.drain_stages();
    assert_eq!(observation.stage_count, 8);
    assert_eq!(observation.invalid_stages, 4);
    assert!(observation.stages.iter().all(Option::is_some));
    drop(worker); // Child endpoint closure must not become invalid stage data.
    observation.drain_stages();
    assert_eq!(observation.invalid_stages, 4);
}

#[test]
fn logging_errors_do_not_replace_the_selected_result_with_a_panic() {
    struct FailedLog(usize);
    impl Write for FailedLog {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.0 += 1;
            Err(std::io::ErrorKind::WouldBlock.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            panic!("diagnostic logging must not flush")
        }
    }
    let mut observation = Observation::new(Instant::now());
    observation.tick(Sample {
        read_bytes: MAX_OUTPUT_BYTES,
        written_bytes: 8,
        reader: None,
        writer: Some(Ok(())),
        input_close: Some(Ok(())),
        exited: false,
        decision: Decision::Error(ParserError::Timeout),
    });
    let mut log = FailedLog(0);
    let selected = Err(ParserError::Timeout);
    observation.finish(&selected, admission::Cleanup::Confirmed, &mut log);
    assert_eq!(selected, Err(ParserError::Timeout));
    assert_eq!(log.0, 2); // One failed summary and one failed snapshot, no retry.
}

#[test]
fn explicit_observer_modes_are_never_replaced_by_fault_tracing() {
    for (args, expected) in [
        (&["tests::fault_worker"][..], true),
        (&["--source"][..], false),
        (
            &["tests::fault_worker", "--observe-deadline-eof-v1"][..],
            false,
        ),
        (
            &["--observe-generic-copy-v1", "tests::fault_worker"][..],
            false,
        ),
    ] {
        let mut command = Command::new("unused-test-fixture");
        command.args(args);
        assert_eq!(eligible(&command), expected);
    }
}
