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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod usage {
    use std::{
        ffi::{c_int, c_long},
        io,
        os::unix::process::ExitStatusExt,
        process::ExitStatus,
    };

    // Darwin SDK sys/{resource,wait}.h and sys/_types/_timeval.h on arm64.
    #[repr(C)]
    #[derive(Default)]
    struct Timeval {
        seconds: c_long,
        microseconds: c_int,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Usage {
        user: Timeval,
        system: Timeval,
        max_rss: c_long,
        counters: [c_long; 13],
    }
    const _: () = {
        assert!(std::mem::size_of::<Timeval>() == 16);
        assert!(std::mem::align_of::<Usage>() == 8);
        assert!(std::mem::size_of::<Usage>() == 144);
        assert!(std::mem::offset_of!(Usage, max_rss) == 32);
    };
    unsafe extern "C" {
        fn wait4(pid: c_int, status: *mut c_int, options: c_int, usage: *mut Usage) -> c_int;
    }
    pub fn poll(pid: u32) -> io::Result<Option<(ExitStatus, i64)>> {
        let pid = c_int::try_from(pid).map_err(io::Error::other)?;
        let mut status = 0;
        let mut usage = Usage::default();
        // The PID belongs to our unreaped child; both output pointers have the SDK ABI.
        match unsafe { wait4(pid, &mut status, 1, &mut usage) } {
            0 => Ok(None), // WNOHANG
            -1 => Err(io::Error::last_os_error()),
            reaped if reaped == pid => Ok(Some((ExitStatus::from_raw(status), usage.max_rss))),
            _ => Err(io::Error::other("wait4 returned a different child")),
        }
    }
}

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
    exit_status: Option<ExitStatus>,
    peak_rss: Option<u64>,
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
            exit_status: None,
            peak_rss: None,
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
    pub fn peak_rss_bytes(&self) -> Option<u64> {
        self.peak_rss
    }
    fn try_exit(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.reaped {
            return self
                .exit_status
                .map(Some)
                .ok_or_else(|| io::Error::other("child status unavailable"));
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            match usage::poll(self.child.id()) {
                Ok(Some((status, rss))) => {
                    self.reaped = true;
                    self.exit_status = Some(status);
                    let rss = u64::try_from(rss).map_err(io::Error::other)?;
                    if rss == 0 {
                        return Err(io::Error::other("wait4 peak RSS is zero"));
                    }
                    self.peak_rss = Some(rss); // Darwin reports bytes.
                    Ok(Some(status))
                }
                Ok(None) => Ok(None),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(None),
                Err(e) => {
                    if e.raw_os_error() == Some(10) {
                        self.reaped = true;
                    } // ECHILD: never signal a reused PID.
                    Err(e)
                }
            }
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let status = self.child.try_wait()?;
            if let Some(status) = status {
                self.reaped = true;
                self.exit_status = Some(status);
            }
            Ok(status)
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
            if let Some(status) = self.try_exit().map_err(|e| e.to_string())? {
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
            match self.try_exit() {
                Ok(Some(_)) => return,
                Err(e) if self.reaped => {
                    eprintln!("benchmark cleanup: {e}");
                    return;
                }
                _ => (),
            }
            let _ = self.child.kill();
            let deadline = Instant::now() + CLEANUP;
            loop {
                match self.try_exit() {
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
