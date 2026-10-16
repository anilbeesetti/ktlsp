//! Independently authored Rust-only capture fixtures, never an official oracle.
use ktlsp_protocol::{read_frame, write_frame};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self, Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
fn stall() -> ! {
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
#[allow(clippy::zombie_processes)] // Intentionally orphaned to exercise group cleanup after parent exit.
fn descendant() {
    let child = Command::new(env::current_exe().unwrap())
        .arg(
            if env::args().nth(1).as_deref() == Some("descendant-silent") {
                "heartbeat-silent"
            } else {
                "heartbeat"
            },
        )
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    eprintln!("descendant={}", child.id());
    let heartbeat = std::path::PathBuf::from(env::var_os("TMPDIR").unwrap()).join("heartbeat");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !heartbeat.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(heartbeat.exists(), "descendant readiness deadline");
}
fn main() {
    let mode = env::args().nth(1).unwrap();
    if mode == "echo" || mode == "error" {
        eprintln!("SYNTHETIC / NONOFFICIAL Rust peer: adapter mechanics only");
    }
    if mode == "premature-response" {
        eprintln!("SYNTHETIC / NONOFFICIAL premature response peer");
        io::stdin().read_exact(&mut [0]).unwrap();
        write_frame(
            &mut io::stdout(),
            br#"{"jsonrpc":"2.0","id":1,"result":null}"#,
        )
        .unwrap();
        io::stdout().flush().unwrap();
        stall();
    }
    if mode == "heartbeat" || mode == "heartbeat-silent" {
        if mode == "heartbeat-silent" {
            #[cfg(unix)]
            {
                unsafe extern "C" {
                    fn close(fd: i32) -> i32;
                }
                unsafe {
                    close(0);
                    close(1);
                    close(2);
                }
            }
        }
        let path = std::path::PathBuf::from(env::var_os("TMPDIR").unwrap()).join("heartbeat");
        for i in 0u64..=u64::MAX {
            fs::write(&path, i.to_string()).unwrap();
            thread::sleep(Duration::from_millis(10));
        }
        return;
    }
    if mode == "no-read" {
        stall();
    }
    if mode == "closed-pipes" {
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn close(fd: i32) -> i32;
            }
            unsafe {
                close(1);
                close(2);
            }
        }
        stall();
    }
    if mode == "partial-eof" {
        print!("Content-Length: 20\r\n\r\n{{");
        return;
    }
    if mode == "bad-frame-tail" {
        print!("Content-Length: -1\r\n\r\nmalformed-tail");
        return;
    }
    if mode == "bad-json-tail" {
        write_frame(&mut io::stdout(), b"{").unwrap();
        print!("raw-tail");
        return;
    }
    if mode == "stderr-flood" || mode == "both-flood" {
        let both = mode == "both-flood";
        thread::spawn(move || {
            loop {
                if io::stderr().write_all(&[b'e'; 4096]).is_err() {
                    return;
                }
            }
        });
        if both {
            loop {
                if io::stdout().write_all(&[b'o'; 4096]).is_err() {
                    return;
                }
            }
        }
        stall();
    }
    if mode == "stdout-flood" {
        loop {
            if io::stdout().write_all(&[b'o'; 4096]).is_err() {
                return;
            }
        }
    }
    if mode == "signal" {
        std::process::abort();
    }
    let mut input = io::stdin().lock();
    while let Some(bytes) = read_frame(&mut input).unwrap() {
        let message: Value = serde_json::from_slice(&bytes).unwrap();
        if message["method"] == "exit" {
            if mode.starts_with("exit-burst") {
                let mut bytes = Vec::new();
                for _ in 0..100 {
                    write_frame(&mut bytes, br#"{"jsonrpc":"2.0","method":"notice"}"#).unwrap();
                }
                if mode == "exit-burst-bad" {
                    bytes.extend_from_slice(b"Content-Length: -1\r\n\r\nraw-tail");
                }
                if mode == "exit-burst-request" {
                    write_frame(
                        &mut bytes,
                        br#"{"jsonrpc":"2.0","id":9,"method":"unknown"}"#,
                    )
                    .unwrap();
                }
                io::stdout().write_all(&bytes).unwrap();
                io::stdout().flush().unwrap();
            }
            if mode == "descendant" || mode == "descendant-silent" {
                descendant();
            }
            if mode == "nonzero" {
                std::process::exit(7);
            }
            return;
        }
        if let Some(id) = message.get("id") {
            let reply = if mode == "server-request" {
                json!({"jsonrpc":"2.0","id":9,"method":"workspace/configuration"})
            } else if mode == "echo" {
                json!({"jsonrpc":"2.0","id":id,"result":message["params"]["text"]})
            } else if mode == "error" {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":"synthetic peer error"}})
            } else {
                json!({"jsonrpc":"2.0","id":id,"result":"😀"})
            };
            let mut frame = Vec::new();
            if mode == "notices" || mode == "notices-eof" {
                for _ in 0..100 {
                    write_frame(&mut frame, br#"{"jsonrpc":"2.0","method":"notice"}"#).unwrap();
                }
            }
            write_frame(&mut frame, &serde_json::to_vec(&reply).unwrap()).unwrap();
            if mode == "fragmented" {
                for byte in frame {
                    io::stdout().write_all(&[byte]).unwrap();
                    io::stdout().flush().unwrap();
                    thread::sleep(Duration::from_millis(1));
                }
            } else {
                io::stdout().write_all(&frame).unwrap();
                io::stdout().flush().unwrap();
            }
            if mode == "notices-eof" {
                #[cfg(unix)]
                {
                    unsafe extern "C" {
                        fn close(fd: i32) -> i32;
                    }
                    unsafe {
                        close(1);
                    }
                }
            }
            if mode == "server-request" {
                stall();
            }
        }
    }
}
