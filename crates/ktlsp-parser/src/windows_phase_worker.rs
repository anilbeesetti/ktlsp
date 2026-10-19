use serde::Serialize;
use std::{
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
struct Packet {
    v: u8,
    token: u64,
    pid: u32,
    seq: u8,
    stage: u8,
    elapsed_ns: u64,
    unix_ns: Option<u128>,
    detail: u64,
    failed_before: u8,
}

pub(super) struct Observer {
    socket: UdpSocket,
    token: u64,
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
        // The main-entry clocks precede config parsing and all socket setup.
        let start = Instant::now();
        let main_wall = wall_ns();
        let address = std::env::var("KTLSP_WINDOWS_PHASE_ADDR").ok()?;
        let token = std::env::var("KTLSP_WINDOWS_PHASE_TOKEN").ok()?;
        if address.is_empty()
            || address.len() > 21
            || token.is_empty()
            || token.len() > 20
            || !token.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let address: SocketAddrV4 = address.parse().ok()?;
        if !address.ip().is_loopback() || address.port() == 0 {
            return None;
        }
        let token = token.parse().ok()?;
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
        socket.set_nonblocking(true).ok()?;
        socket.connect(address).ok()?;
        let ready = Instant::now();
        let ready_wall = wall_ns();
        let mut observer = Self {
            socket,
            token,
            pid: std::process::id(),
            start,
            seq: 0,
            failed: 0,
        };
        observer.send(0, 0, 0, main_wall);
        let setup_ns = elapsed_ns(start, ready);
        observer.send(1, setup_ns, setup_ns, ready_wall);
        Some(observer)
    }

    fn send(&mut self, stage: u8, detail: u64, elapsed_ns: u64, unix_ns: Option<u128>) {
        if self.seq >= 32 {
            return;
        }
        let packet = Packet {
            v: 1,
            token: self.token,
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
        let mut remaining = &mut bytes[..];
        let serialized = serde_json::to_writer(&mut remaining, &packet).is_ok();
        let len = 256 - remaining.len();
        if !serialized || !matches!(self.socket.send(&bytes[..len]), Ok(sent) if sent == len) {
            self.failed = self.failed.saturating_add(1);
        }
    }
}

pub(super) fn emit(observer: &mut Option<Observer>, stage: u8, detail: u64) {
    if let Some(observer) = observer {
        let elapsed = elapsed_ns(observer.start, Instant::now());
        observer.send(stage, detail, elapsed, wall_ns());
    }
}
