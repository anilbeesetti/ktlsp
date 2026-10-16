//! The peer is a real standalone Rust executable, not a shell/echo command stub.
#[test]
fn synthetic_cleanup_peer_refuses_wrong_container_identity() {
    let root = std::env::temp_dir().join(format!(
        "ktlsp-cleanup-peer-consumer-{}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let docker = root.join(if cfg!(windows) {
        "docker.exe"
    } else {
        "docker"
    });
    std::fs::copy(env!("CARGO_BIN_EXE_host-cleanup-test-peer"), &docker).unwrap();
    std::fs::write(root.join("fault"), b"reload").unwrap();
    std::fs::write(root.join("cid"), "b".repeat(64)).unwrap();
    std::fs::write(root.join("container"), b"created").unwrap();
    let output = std::process::Command::new(&docker)
        .args(["rm", &"c".repeat(64)])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(root.join("container").is_file());
    std::fs::remove_dir_all(root).unwrap();
}
