//! Rust-only adversarial subprocess for the optional harness tests.
use ktlsp_protocol::{read_frame, write_frame};
use serde_json::{Value, json};
use std::io::{self, Write};
use std::{env, thread, time::Duration};

fn stall() -> ! {
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn main() {
    let mode = env::args().nth(1).unwrap();
    if mode == "no-read" {
        stall();
    }
    if mode == "partial" {
        print!("Content-Length: 20\r\n\r\n{{");
        io::stdout().flush().unwrap();
        stall();
    }
    if mode == "bad-frame" {
        print!("Content-Length: -1\r\n\r\n");
        return;
    }
    if mode == "bad-json" {
        write_frame(&mut io::stdout(), b"{").unwrap();
        return;
    }
    if mode == "stderr-flood" {
        io::stderr().write_all(&vec![b'x'; 70 * 1024]).unwrap();
        stall();
    }
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    while let Some(bytes) = read_frame(&mut input).unwrap() {
        let message: Value = serde_json::from_slice(&bytes).unwrap();
        if message["method"] == "exit" {
            if mode == "ignore-exit" {
                stall();
            }
            if mode == "nonzero" {
                std::process::exit(7);
            }
            return;
        }
        if let Some(id) = message.get("id") {
            if mode == "invalid-params" {
                write_frame(
                    &mut output,
                    br#"{"jsonrpc":"2.0","method":"notice","params":17}"#,
                )
                .unwrap();
                output.flush().unwrap();
                stall();
            }
            if mode == "server-request" {
                write_frame(
                    &mut output,
                    &serde_json::to_vec(
                        &json!({"jsonrpc":"2.0","id":8,"method":"workspace/configuration"}),
                    )
                    .unwrap(),
                )
                .unwrap();
                output.flush().unwrap();
                stall();
            }
            if mode == "notifications-flood" {
                for _ in 0..1100 {
                    if write_frame(&mut output, br#"{"jsonrpc":"2.0","method":"notice"}"#).is_err()
                    {
                        return;
                    }
                }
                output.flush().unwrap();
                stall();
            }
            let reply = json!({"jsonrpc":"2.0","id":if mode == "unknown-id" {json!(99)} else {id.clone()},
                "result":if mode == "mismatch" {json!({"severity":2})} else {Value::Null}});
            let bytes = serde_json::to_vec(&reply).unwrap();
            if mode == "fragmented" {
                let mut frame = Vec::new();
                write_frame(&mut frame, &bytes).unwrap();
                for byte in frame {
                    output.write_all(&[byte]).unwrap();
                    output.flush().unwrap();
                }
            } else {
                write_frame(&mut output, &bytes).unwrap();
                if mode == "duplicate" {
                    write_frame(&mut output, &bytes).unwrap();
                }
                output.flush().unwrap();
            }
        }
    }
}
