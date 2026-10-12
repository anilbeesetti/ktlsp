include!("../src/process.rs");

fn peer(mode: &str) -> Process {
    Process::spawn(
        Command::new(env!("CARGO_BIN_EXE_bench-test-peer")).arg(mode),
        Duration::from_millis(500),
    )
    .unwrap()
}
#[test]
fn writes_reads_and_exit_are_bounded() {
    let begin = Instant::now();
    {
        let mut p = peer("hang");
        assert!(
            p.raw(vec![b'x'; 1024 * 1024])
                .unwrap_err()
                .contains("timeout")
        );
    }
    assert!(begin.elapsed() < Duration::from_secs(4));
    for mode in ["hang", "partial", "stderr", "stdout", "exit"] {
        let begin = Instant::now();
        {
            let mut p = peer(mode);
            // Flooded framed stdout may yield frames before reaching its cap.
            let error = loop {
                if let Err(e) = p.receive() {
                    break e;
                }
            };
            assert!(!error.is_empty());
        }
        assert!(begin.elapsed() < Duration::from_secs(4), "{mode}");
    }
}
#[test]
fn framed_exchange_and_clean_reap() {
    let mut p = peer("echo");
    let value = serde_json::json!({"unicode":"😀\r\n水"});
    p.send(&value).unwrap();
    assert_eq!(p.receive().unwrap(), value);
    let (status, stderr) = p.finish().unwrap();
    assert!(status.success());
    assert_eq!(stderr, 0);
    assert!(p.reaped);
    assert!(p.child.try_wait().unwrap().is_some());
}
#[test]
fn missing_metadata_and_worker_failure_are_not_success() {
    let mut p = Process::spawn(
        Command::new(env!("CARGO_BIN_EXE_ktlsp-baseline")).args(["--worker", "bogus"]),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(p.receive().is_err());
    let (status, stderr) = p.finish().unwrap();
    assert!(!status.success());
    assert!(stderr > 0);
}
