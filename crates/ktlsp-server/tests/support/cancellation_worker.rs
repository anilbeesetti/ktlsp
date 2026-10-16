//! A controlled failing Rust peer, never a fabricated successful parser tree.
use std::{
    io::{self, Read},
    time::{Duration, Instant},
};
fn main() {
    assert_eq!(std::env::args().nth(1).as_deref(), Some("--source"));
    let mut source = String::new();
    io::stdin().read_to_string(&mut source).unwrap();
    let token = source
        .lines()
        .next()
        .unwrap()
        .strip_prefix("// hold:")
        .unwrap();
    assert!(!token.is_empty() && token.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    let directory = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    std::fs::write(
        directory.join(format!("started-{token}")),
        std::process::id().to_string(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !directory.join(format!("release-{token}")).exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    // The consumer must expose a real parser failure, never an empty outline.
    std::process::exit(7);
}
