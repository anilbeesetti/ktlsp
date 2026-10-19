//! Temporary #448 evidence for one named synthetic fixture; never a provider.
use serde::Serialize;
use serde_json::Value;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::PathBuf,
    process::{Command, Output},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const FIXTURE: &str = "declaration_hover_paired_lf_boundaries_negotiation_and_refusals";
const SOURCE: &str = "class Box {}\nfun f(p: Long) { val x = 1; val y = x }\n";
const MAX_FRAMES: usize = 32;
const MAX_BYTES: usize = 16 * 1024;

#[derive(Serialize)]
struct Frame {
    direction: &'static str,
    observed_ns: u128,
    write_confirmed_ns: Option<u128>,
    payload: Vec<u8>,
}

pub struct Capture {
    directory: PathBuf,
    server_pid: u32,
    start: Instant,
    start_unix_ns: Option<u128>,
    selected: bool,
    excluded_source: bool,
    frames: Vec<Frame>,
    bytes: usize,
    seen: usize,
    truncated: bool,
    client_writes_seen: usize,
    client_writes_confirmed: usize,
    stdout_eof: Option<bool>,
    finalized: bool,
}
impl Capture {
    pub fn configure(command: &mut Command) -> Option<PathBuf> {
        command.env_remove("KTLSP_WINDOWS_PHASE_CAPTURE_ENABLE");
        let thread = std::thread::current();
        if thread.name() != Some(FIXTURE) {
            return None;
        }
        let directory = std::env::var_os("KTLSP_WINDOWS_PHASE_CAPTURE_DIR")?;
        if directory.as_encoded_bytes().len() > 4096 {
            return None;
        }
        let directory = PathBuf::from(directory);
        if !directory.is_absolute() || !directory.is_dir() {
            return None;
        }
        command.env("KTLSP_WINDOWS_PHASE_CAPTURE_ENABLE", "1");
        Some(directory)
    }
    pub fn new(directory: PathBuf, server_pid: u32) -> Self {
        Self {
            directory,
            server_pid,
            start: Instant::now(),
            start_unix_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_nanos()),
            selected: false,
            excluded_source: false,
            frames: Vec::with_capacity(MAX_FRAMES),
            bytes: 0,
            seen: 0,
            truncated: false,
            client_writes_seen: 0,
            client_writes_confirmed: 0,
            stdout_eof: None,
            finalized: false,
        }
    }
    pub fn sending(&mut self, message: &Value, payload: Vec<u8>, write: impl FnOnce(&[u8])) {
        if message["method"] == "textDocument/didOpen" {
            if message
                .pointer("/params/textDocument/text")
                .and_then(Value::as_str)
                == Some(SOURCE)
            {
                self.selected = true;
            } else {
                self.excluded_source = true;
            }
        }
        if message["method"] == "textDocument/didChange" {
            self.excluded_source = true;
        }
        self.client_writes_seen = self.client_writes_seen.saturating_add(1);
        match self.retain("client", payload) {
            Ok(index) => {
                write(&self.frames[index].payload);
                self.frames[index].write_confirmed_ns = Some(self.start.elapsed().as_nanos());
            }
            Err(payload) => write(&payload),
        }
        self.client_writes_confirmed = self.client_writes_confirmed.saturating_add(1);
    }
    pub fn received<T>(&mut self, payload: Vec<u8>, parse: impl FnOnce(&[u8]) -> T) -> T {
        match self.retain("server", payload) {
            Ok(index) => parse(&self.frames[index].payload),
            Err(payload) => parse(&payload),
        }
    }
    fn retain(&mut self, direction: &'static str, payload: Vec<u8>) -> Result<usize, Vec<u8>> {
        self.seen = self.seen.saturating_add(1);
        if self.frames.len() == MAX_FRAMES || payload.len() > MAX_BYTES - self.bytes {
            self.truncated = true;
            return Err(payload);
        }
        self.bytes += payload.len();
        let index = self.frames.len();
        self.frames.push(Frame {
            direction,
            observed_ns: self.start.elapsed().as_nanos(),
            write_confirmed_ns: None,
            payload,
        });
        Ok(index)
    }
    pub fn mark_stdout_eof(&mut self) {
        self.stdout_eof = Some(true);
    }
    pub fn finish(mut self, output: &Output) -> std::io::Result<()> {
        self.finalized = true;
        self.persist(Some(output))
    }
    fn persist(&self, output: Option<&Output>) -> std::io::Result<()> {
        if self.excluded_source {
            return Ok(());
        }
        #[derive(Serialize)]
        struct Record<'a> {
            schema_version: u8,
            scope: &'static str,
            fixture: &'static str,
            source_selected: bool,
            server_pid: u32,
            harness_pid: u32,
            start_unix_ns: Option<u128>,
            elapsed_ns: u128,
            frames_seen: usize,
            retained_payload_bytes: usize,
            truncated: bool,
            partial: bool,
            unwinding: bool,
            stdout_eof: Option<bool>,
            client_writes_seen: usize,
            client_writes_confirmed: usize,
            exit_observed: bool,
            exit_code: Option<i32>,
            exit_success: Option<bool>,
            stderr: Option<&'a [u8]>,
            stderr_bytes: Option<usize>,
            stderr_truncated: Option<bool>,
            frames: &'a [Frame],
        }
        let record = Record {
            schema_version: 1,
            scope: "synthetic-only; payloads only, failed frame reads may consume unretained bytes; observation perturbs timing; not original-incident causality",
            fixture: FIXTURE,
            source_selected: self.selected,
            server_pid: self.server_pid,
            harness_pid: std::process::id(),
            start_unix_ns: self.start_unix_ns,
            elapsed_ns: self.start.elapsed().as_nanos(),
            frames_seen: self.seen,
            retained_payload_bytes: self.bytes,
            truncated: self.truncated,
            partial: output.is_none(),
            unwinding: std::thread::panicking(),
            stdout_eof: self.stdout_eof,
            client_writes_seen: self.client_writes_seen,
            client_writes_confirmed: self.client_writes_confirmed,
            exit_observed: output.is_some(),
            exit_code: output.and_then(|output| output.status.code()),
            exit_success: output.map(|output| output.status.success()),
            stderr: output.map(|output| &output.stderr[..output.stderr.len().min(4096)]),
            stderr_bytes: output.map(|output| output.stderr.len()),
            stderr_truncated: output.map(|output| output.stderr.len() > 4096),
            frames: &self.frames,
        };
        // Byte arrays cap the serialized record below128KiB. create_new never
        // replaces a prior PID's capture; collision/error means missing evidence.
        let file = OpenOptions::new().write(true).create_new(true).open(
            self.directory
                .join(format!("session-{}.json", self.server_pid)),
        )?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &record).map_err(std::io::Error::other)?;
        writer.flush()
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        if !self.finalized {
            self.finalized = true;
            let _ = self.persist(None);
        }
    }
}
