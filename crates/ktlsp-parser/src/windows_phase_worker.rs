use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
struct Record {
    v: u8,
    capture_id: u64,
    server_pid: u32,
    request_seq: u64,
    pid: u32,
    seq: u8,
    stage: u8,
    elapsed_ns: u64,
    unix_ns: Option<u128>,
    detail: u64,
    failed_before: u8,
}

pub(super) struct Observer {
    file: File,
    capture_id: u64,
    server_pid: u32,
    request_seq: u64,
    pid: u32,
    start: Instant,
    seq: u8,
    failed: u8,
}

fn wall_ns() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

fn elapsed_ns(start: Instant, now: Instant) -> u64 {
    u64::try_from(now.saturating_duration_since(start).as_nanos()).unwrap_or(u64::MAX)
}

impl Observer {
    pub(super) fn start() -> Option<Self> {
        // The main-entry clocks precede config parsing and all file setup.
        let start = Instant::now();
        let main_wall = wall_ns();
        let path = PathBuf::from(std::env::var_os("KTLSP_WINDOWS_PHASE_FILE")?);
        if !path.is_absolute() || path.as_os_str().as_encoded_bytes().len() > 4096 {
            return None;
        }
        let filename = path.file_name()?.to_str()?;
        let mut fields = filename
            .strip_prefix("worker-")?
            .strip_suffix(".jsonl")?
            .split('-');
        let server = fields.next()?;
        let request = fields.next()?;
        let capture = fields.next()?;
        if fields.next().is_some()
            || server.is_empty()
            || server.len() > 10
            || server.starts_with('0')
            || !server.bytes().all(|byte| byte.is_ascii_digit())
            || request.is_empty()
            || request.len() > 2
            || request.starts_with('0')
            || !request.bytes().all(|byte| byte.is_ascii_digit())
            || capture.len() != 16
            || !capture.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return None;
        }
        let server_pid: u32 = server.parse().ok()?;
        let request_seq: u64 = request.parse().ok()?;
        if !(1..=16).contains(&request_seq) {
            return None;
        }
        let capture_id = u64::from_str_radix(capture, 16).ok()?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .ok()?;
        let mut observer = Self {
            file,
            capture_id,
            server_pid,
            request_seq,
            pid: std::process::id(),
            start,
            seq: 0,
            failed: 0,
        };
        let ready = Instant::now();
        let ready_wall = wall_ns();
        observer.record(0, 0, 0, main_wall);
        let setup_ns = elapsed_ns(start, ready);
        observer.record(1, setup_ns, setup_ns, ready_wall);
        Some(observer)
    }

    fn record(&mut self, stage: u8, detail: u64, elapsed_ns: u64, unix_ns: Option<u128>) {
        if self.seq >= 32 {
            return;
        }
        let record = Record {
            v: 2,
            capture_id: self.capture_id,
            server_pid: self.server_pid,
            request_seq: self.request_seq,
            pid: self.pid,
            seq: self.seq,
            stage,
            elapsed_ns,
            unix_ns,
            detail,
            failed_before: self.failed,
        };
        self.seq += 1;
        let mut bytes = [0_u8; 256];
        let mut remaining = &mut bytes[..255];
        let serialized = serde_json::to_writer(&mut remaining, &record).is_ok();
        let len = 255 - remaining.len();
        bytes[len] = b'\n';
        // Synchronous trace I/O can block inside the unchanged supervisor allowance.
        // At most 32 attempts of <=256 bytes bound this file to 8192 bytes.
        if !serialized || self.file.write_all(&bytes[..=len]).is_err() {
            self.failed = self.failed.saturating_add(1);
        }
    }
}

pub(super) fn emit(observer: &mut Option<Observer>, stage: u8, detail: u64) {
    if let Some(observer) = observer {
        let elapsed = elapsed_ns(observer.start, Instant::now());
        observer.record(stage, detail, elapsed, wall_ns());
    }
}
