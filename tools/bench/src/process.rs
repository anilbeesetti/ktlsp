use ktlsp_protocol::{read_frame, write_frame};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, BufReader, Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: u64 = 1024 * 1024;
const CLEANUP: Duration = Duration::from_secs(2);
type Result<T> = std::result::Result<T, String>;
enum Event {
    Frame(Value),
    Written,
    Eof,
    Stderr(usize),
    Error(String),
}

struct Limited<R> {
    inner: R,
    remaining: u64,
}
impl<R: Read> Read for Limited<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("stdout output limit"));
        }
        let len = buf.len().min(self.remaining as usize);
        let count = self.inner.read(&mut buf[..len])?;
        self.remaining -= count as u64;
        Ok(count)
    }
}

pub struct Process {
    child: Child,
    writes: Option<SyncSender<Vec<u8>>>,
    events: Receiver<Event>,
    frames: VecDeque<Value>,
    written: bool,
    eof: bool,
    stderr_done: bool,
    stderr_bytes: usize,
    reaped: bool,
    deadline: Instant,
}
impl Process {
    pub fn spawn(command: &mut Command, budget: Duration) -> Result<Self> {
        let deadline = Instant::now() + budget;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PATH", "")
            .env_remove("JAVA_HOME")
            .env_remove("JDK_HOME")
            .spawn()
            .map_err(|e| e.to_string())?;
        let (tx, events) = mpsc::sync_channel(16);
        let stdout = child.stdout.take().ok_or("missing stdout")?;
        let out_tx = tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(Limited {
                inner: stdout,
                remaining: OUTPUT_LIMIT,
            });
            loop {
                let event = match read_frame(&mut reader) {
                    Ok(Some(bytes)) => match serde_json::from_slice(&bytes) {
                        Ok(value) => Event::Frame(value),
                        Err(e) => Event::Error(format!("stdout JSON: {e}")),
                    },
                    Ok(None) => Event::Eof,
                    Err(e) => Event::Error(format!("stdout framing: {e}")),
                };
                let terminal = matches!(event, Event::Eof | Event::Error(_));
                if out_tx.send(event).is_err() || terminal {
                    break;
                }
            }
        });
        let stderr = child.stderr.take().ok_or("missing stderr")?;
        let err_tx = tx.clone();
        thread::spawn(move || {
            let mut reader = stderr.take(OUTPUT_LIMIT + 1);
            let mut buffer = [0; 4096];
            let mut total = 0;
            let event = loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break Event::Stderr(total),
                    Ok(n) => {
                        total += n;
                        if total > OUTPUT_LIMIT as usize {
                            break Event::Error("stderr output limit".into());
                        }
                    }
                    Err(e) => break Event::Error(format!("stderr read: {e}")),
                }
            };
            let _ = err_tx.send(event);
        });
        let mut stdin = child.stdin.take().ok_or("missing stdin")?;
        let (writes, input) = mpsc::sync_channel::<Vec<u8>>(1);
        thread::spawn(move || {
            while let Ok(bytes) = input.recv() {
                let event = match stdin.write_all(&bytes).and_then(|()| stdin.flush()) {
                    Ok(()) => Event::Written,
                    Err(e) => Event::Error(format!("stdin write: {e}")),
                };
                let terminal = matches!(event, Event::Error(_));
                if tx.send(event).is_err() || terminal {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            writes: Some(writes),
            events,
            frames: VecDeque::new(),
            written: false,
            eof: false,
            stderr_done: false,
            stderr_bytes: 0,
            reaped: false,
            deadline,
        })
    }
    fn pump(&mut self) -> Result<()> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or("subprocess timeout")?;
        match self
            .events
            .recv_timeout(remaining)
            .map_err(|e| format!("subprocess timeout/disconnected: {e}"))?
        {
            Event::Frame(value) => {
                if self.frames.len() >= 128 {
                    return Err("unsolicited frame limit".into());
                }
                self.frames.push_back(value);
            }
            Event::Written => self.written = true,
            Event::Eof => self.eof = true,
            Event::Stderr(n) => {
                self.stderr_done = true;
                self.stderr_bytes = n;
            }
            Event::Error(e) => return Err(e),
        }
        Ok(())
    }
    pub fn raw(&mut self, bytes: Vec<u8>) -> Result<()> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("input limit".into());
        }
        self.written = false;
        self.writes
            .as_ref()
            .ok_or("stdin closed")?
            .try_send(bytes)
            .map_err(|e| e.to_string())?;
        while !self.written {
            self.pump()?;
        }
        Ok(())
    }
    pub fn send(&mut self, value: &Value) -> Result<()> {
        let mut bytes = Vec::new();
        write_frame(
            &mut bytes,
            &serde_json::to_vec(value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        self.raw(bytes)
    }
    pub fn receive(&mut self) -> Result<Value> {
        loop {
            if let Some(value) = self.frames.pop_front() {
                return Ok(value);
            }
            if self.eof {
                return Err("unexpected stdout EOF".into());
            }
            self.pump()?;
        }
    }
    pub fn finish(&mut self) -> Result<(ExitStatus, usize)> {
        self.writes.take();
        while !self.eof || !self.stderr_done {
            self.pump()?;
        }
        if !self.frames.is_empty() {
            return Err("unexpected extra frames".into());
        }
        loop {
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                self.reaped = true;
                return Ok((status, self.stderr_bytes));
            }
            if Instant::now() >= self.deadline {
                return Err("exit timeout".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.writes.take();
        if !self.reaped {
            let _ = self.child.kill();
            let deadline = Instant::now() + CLEANUP;
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) => {
                        self.reaped = true;
                        break;
                    }
                    _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(1)),
                    _ => {
                        eprintln!("benchmark cleanup failed to reap child {}", self.child.id());
                        break;
                    }
                }
            }
        }
    }
}
