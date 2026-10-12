//! Hostile peers for the supervisor integration tests; never a product server.
use std::{
    env,
    io::{self, Write},
    thread,
    time::Duration,
};
fn main() {
    match env::args().nth(1).as_deref() {
        Some("hang") => thread::sleep(Duration::from_secs(60)),
        Some("partial") => {
            io::stdout()
                .write_all(b"Content-Length: 10\r\n\r\n{")
                .unwrap();
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(60));
        }
        Some("stderr") => {
            let _ = io::stderr().write_all(&vec![b'x'; 1024 * 1024 + 1]);
            thread::sleep(Duration::from_secs(60));
        }
        Some("stdout") => {
            let mut out = io::stdout().lock();
            for _ in 0..100000 {
                if ktlsp_protocol::write_frame(&mut out, b"{}").is_err() {
                    break;
                }
            }
        }
        Some("exit") => (),
        Some("echo") => {
            let mut input = io::stdin().lock();
            let mut out = io::stdout().lock();
            while let Ok(Some(bytes)) = ktlsp_protocol::read_frame(&mut input) {
                if ktlsp_protocol::write_frame(&mut out, &bytes).is_err() {
                    break;
                }
                if out.flush().is_err() {
                    break;
                }
            }
        }
        _ => std::process::exit(2),
    }
}
