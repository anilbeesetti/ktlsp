//! Optional native replay tooling; never part of the server/default workspace.
pub mod receiver;
use ktlsp_protocol::{read_frame, write_frame};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_MESSAGES: usize = 1024;
const MAX_STDERR: usize = 64 * 1024;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub operation_ms: u64,
    pub session_ms: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            operation_ms: 5_000,
            session_ms: 30_000,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    direction: String,
    message: Value,
}

pub struct Evidence {
    clients: Vec<Value>,
    responses: BTreeMap<String, Value>,
    notifications: Vec<Value>,
    metadata: Value,
}

fn bounded_file(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("fixture exceeds byte limit".into());
    }
    Ok(bytes)
}

fn id(message: &Value) -> Result<Option<String>, String> {
    match message.get("id") {
        None => Ok(None),
        Some(value) if value.is_string() || value.is_i64() || value.is_u64() => {
            Ok(Some(value.to_string()))
        }
        _ => Err("ID must be a string or integer".into()),
    }
}

fn envelope(message: &Value) -> Result<(), String> {
    if !message.is_object() || message["jsonrpc"] != "2.0" {
        return Err("invalid JSON-RPC envelope".into());
    }
    if let Some(method) = message.get("method") {
        if !method.is_string() || message.get("result").is_some() || message.get("error").is_some()
        {
            return Err("invalid request/notification".into());
        }
        if let Some(params) = message.get("params")
            && !params.is_null()
            && !params.is_object()
            && !params.is_array()
        {
            return Err("params must be structured or null".into());
        }
    } else if id(message)?.is_none()
        || (message.get("result").is_some() == message.get("error").is_some())
    {
        return Err("invalid response".into());
    }
    if let Some(error) = message.get("error")
        && (!error.is_object() || error["code"].as_i64().is_none() || !error["message"].is_string())
    {
        return Err("invalid JSON-RPC error".into());
    }
    Ok(())
}

impl Evidence {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let transcript = bounded_file(&directory.join("transcript.json"))?;
        let metadata = bounded_file(&directory.join("metadata.json"))?;
        Self::parse(&transcript, &metadata)
    }

    pub fn parse(transcript: &[u8], metadata: &[u8]) -> Result<Self, String> {
        if transcript.len() > MAX_BYTES || metadata.len() > MAX_BYTES {
            return Err("fixture exceeds byte limit".into());
        }
        let records: Vec<Record> = serde_json::from_slice(transcript).map_err(|e| e.to_string())?;
        let metadata: Value = serde_json::from_slice(metadata).map_err(|e| e.to_string())?;
        if metadata["release"].as_str().is_none_or(str::is_empty)
            || metadata["sha256"]
                .as_str()
                .is_none_or(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
            || metadata["returncode"].as_i64().is_none()
        {
            return Err("metadata requires release, archive sha256 and recorded returncode".into());
        }
        if records.is_empty() || records.len() > MAX_MESSAGES {
            return Err("invalid fixture message count".into());
        }
        let mut evidence = Self {
            clients: Vec::new(),
            responses: BTreeMap::new(),
            notifications: Vec::new(),
            metadata,
        };
        let mut seen = BTreeSet::new();
        let mut pending = None;
        let mut exited = false;
        for record in records {
            envelope(&record.message)?;
            let key = id(&record.message)?;
            match record.direction.as_str() {
                "client" => {
                    if exited || record.message.get("method").is_none() {
                        return Err("unsupported client response or messages after exit".into());
                    }
                    if pending.is_some() {
                        return Err(
                            "interleaved client messages require a live interaction contract"
                                .into(),
                        );
                    }
                    if let Some(key) = key {
                        if !seen.insert(key.clone()) {
                            return Err(
                                "requests must have unique IDs and be sequentially answered".into(),
                            );
                        }
                        pending = Some(key);
                    }
                    exited = record.message["method"] == "exit";
                    if exited && pending.is_some() {
                        return Err("exit before recorded response".into());
                    }
                    evidence.clients.push(record.message);
                }
                "server" if record.message.get("method").is_some() => {
                    if key.is_some() {
                        return Err("server requests need a live interaction contract".into());
                    }
                    evidence.notifications.push(record.message);
                }
                "server" => {
                    if key != pending || key.is_none() {
                        return Err("unexpected recorded response ID".into());
                    }
                    evidence.responses.insert(key.unwrap(), record.message);
                    pending = None;
                }
                _ => return Err("unknown transcript direction".into()),
            }
        }
        if pending.is_some() || !exited {
            return Err("fixture needs answered requests and recorded exit".into());
        }
        Ok(evidence)
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

enum Event {
    Written,
    Message(Value),
    StdoutEnd,
    Stderr(Vec<u8>),
    Failed(String),
}

#[derive(Default)]
struct Capture {
    responses: BTreeMap<String, Value>,
    notifications: Vec<Value>,
    pending: Option<String>,
    written: bool,
    stdout_end: bool,
    stderr: Option<Vec<u8>>,
}

impl Capture {
    fn event(&mut self, event: Event) -> Result<(), String> {
        match event {
            Event::Failed(error) => return Err(error),
            Event::Written => self.written = true,
            Event::StdoutEnd => self.stdout_end = true,
            Event::Stderr(bytes) => self.stderr = Some(bytes),
            Event::Message(message) => {
                envelope(&message)?;
                let key = id(&message)?;
                if message.get("method").is_some() {
                    if key.is_some() {
                        return Err(
                            "unexpected server request; live interaction unsupported".into()
                        );
                    }
                    self.notifications.push(message);
                } else {
                    if key != self.pending || key.is_none() {
                        return Err("unexpected or duplicate response ID".into());
                    }
                    self.responses.insert(key.unwrap(), message);
                    self.pending = None;
                }
            }
        }
        Ok(())
    }

    fn receive(&mut self, events: &Receiver<Event>, deadline: Instant) -> Result<(), String> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("subprocess deadline exceeded".into());
        }
        let event = events
            .recv_timeout(remaining)
            .map_err(|e| format!("subprocess deadline/channel: {e}"))?;
        self.event(event)
    }
}

pub fn replay(evidence: &Evidence, command: &[String], limits: Limits) -> Result<Value, String> {
    if command.is_empty() || limits.operation_ms == 0 || limits.session_ms == 0 {
        return Err("executable and positive deadlines required".into());
    }
    let started = Instant::now();
    let session = started
        .checked_add(Duration::from_millis(limits.session_ms))
        .ok_or("deadline overflow")?;
    let mut child = Process(
        Command::new(&command[0])
            .args(&command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn failed: {e}"))?,
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let stdout = child.0.stdout.take().unwrap();
    let stderr = child.0.stderr.take().unwrap();
    let (send, events) = mpsc::channel();
    let (writes, inputs) = mpsc::channel::<Vec<u8>>();
    let writer_send = send.clone();
    thread::spawn(move || {
        for bytes in inputs {
            let event = match write_frame(&mut stdin, &bytes)
                .and_then(|()| stdin.flush().map_err(Into::into))
            {
                Ok(()) => Event::Written,
                Err(error) => Event::Failed(format!("stdin: {error}")),
            };
            if writer_send.send(event).is_err() {
                break;
            }
        }
    });
    let reader_send = send.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout.take((MAX_BYTES + 1) as u64));
        let mut bytes = 0;
        for index in 0..=MAX_MESSAGES {
            let event = match read_frame(&mut reader) {
                Ok(Some(payload)) => {
                    bytes += payload.len();
                    if bytes > MAX_BYTES || index == MAX_MESSAGES {
                        Event::Failed("stdout exceeds aggregate byte/message limit".into())
                    } else {
                        match serde_json::from_slice(&payload) {
                            Ok(value) => Event::Message(value),
                            Err(error) => Event::Failed(format!("stdout JSON: {error}")),
                        }
                    }
                }
                Ok(None) if reader.get_ref().limit() == 0 => {
                    Event::Failed("stdout exceeds aggregate byte limit".into())
                }
                Ok(None) => Event::StdoutEnd,
                Err(error) => Event::Failed(format!("stdout frame/byte limit: {error}")),
            };
            let end = !matches!(event, Event::Message(_));
            if reader_send.send(event).is_err() || end {
                break;
            }
        }
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let event = match stderr.take((MAX_STDERR + 1) as u64).read_to_end(&mut bytes) {
            Ok(_) if bytes.len() <= MAX_STDERR => Event::Stderr(bytes),
            Ok(_) => Event::Failed("stderr exceeds byte limit".into()),
            Err(error) => Event::Failed(format!("stderr: {error}")),
        };
        let _ = send.send(event);
    });
    let mut capture = Capture::default();
    for message in &evidence.clients {
        capture.pending = id(message)?;
        capture.written = false;
        writes
            .send(serde_json::to_vec(message).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let deadline = session.min(
            Instant::now()
                .checked_add(Duration::from_millis(limits.operation_ms))
                .ok_or("deadline overflow")?,
        );
        while !capture.written || capture.pending.is_some() {
            capture.receive(&events, deadline)?;
            if capture.stdout_end && capture.pending.is_some() {
                return Err("EOF before response".into());
            }
        }
    }
    drop(writes);
    let deadline = session.min(
        Instant::now()
            .checked_add(Duration::from_millis(limits.operation_ms))
            .ok_or("deadline overflow")?,
    );
    while !capture.stdout_end || capture.stderr.is_none() {
        capture.receive(&events, deadline)?;
    }
    let status = loop {
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("process exit deadline exceeded".into());
        }
        thread::sleep(Duration::from_millis(5));
    };
    let mut responses = Vec::new();
    for (key, expected) in &evidence.responses {
        let actual = capture.responses.get(key);
        responses.push(
            json!({"id":expected["id"], "matches":actual == Some(expected), "expected":expected,"actual":actual,
            "differences":differences(Some(expected), actual)}),
        );
    }
    let notifications_match = evidence.notifications == capture.notifications;
    let exit_matches = status.code().map(i64::from) == evidence.metadata["returncode"].as_i64();
    let matches =
        responses.iter().all(|r| r["matches"] == true) && notifications_match && exit_matches;
    Ok(
        json!({"outcome":if matches {"match"} else {"mismatch"}, "matches":matches,
        "command":command, "officialMetadata":evidence.metadata, "normalization":"none; original IDs, URIs and array order retained",
        "limits":{"deadlines":limits,"stdoutBytes":MAX_BYTES,"stdoutMessages":MAX_MESSAGES,"stderrBytes":MAX_STDERR},
        "responses":responses, "notifications":{"matchesCapturedSequence":notifications_match,
            "expectedCaptured":evidence.notifications,"actual":capture.notifications,
            "coverage":"decoded observations only; original shutdown tail may be incomplete"},
        "exit":{"expected":evidence.metadata["returncode"],"actual":status.code(),"matches":exit_matches},
        "stderrUtf8Lossy":String::from_utf8_lossy(&capture.stderr.unwrap())}),
    )
}

pub fn differences(expected: Option<&Value>, actual: Option<&Value>) -> Vec<Value> {
    fn visit(
        path: String,
        expected: Option<&Value>,
        actual: Option<&Value>,
        output: &mut Vec<Value>,
    ) {
        if expected == actual {
            return;
        }
        match (expected, actual) {
            (Some(Value::Object(left)), Some(Value::Object(right))) => {
                for key in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
                    visit(
                        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                        left.get(key),
                        right.get(key),
                        output,
                    );
                }
            }
            _ => output.push(json!({"pointer":path,"expectedPresent":expected.is_some(),
                "actualPresent":actual.is_some(),"expected":expected,"actual":actual})),
        }
    }
    let mut result = Vec::new();
    visit(String::new(), expected, actual, &mut result);
    result
}

pub mod recorder;
