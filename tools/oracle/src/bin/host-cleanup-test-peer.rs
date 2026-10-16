//! Synthetic file-backed command peer; never connects to Docker or systemd.
use serde_json::json;
use std::{fs, io::Write};

fn main() {
    if let Err(error) = run() {
        eprintln!("synthetic host peer: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let root = exe.parent().ok_or("peer directory")?;
    let program = exe.file_name().ok_or("peer name")?.to_string_lossy();
    let mut argv: Vec<_> = std::env::args().skip(1).collect();
    if argv.first().is_some_and(|v| v.starts_with("--host=")) {
        argv.remove(0);
    }
    let mut log = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(root.join("commands"))?;
    writeln!(log, "{}", json!({"program":program,"args":argv}))?;
    let fault = fs::read_to_string(root.join("fault"))?;
    match program.as_ref() {
        "systemctl" => match argv.first().map(String::as_str) {
            Some("daemon-reload") if fault == "reload" => {
                return Err("injected reload failure".into());
            }
            Some("daemon-reload") => {}
            Some("show") if fault == "readback-command" => {
                return Err("injected readback failure".into());
            }
            Some("show") => {
                let user = if fault == "readback-policy" {
                    "unrelated"
                } else {
                    "root"
                };
                println!(
                    "User={user}\nKillMode=control-group\nRemainAfterExit=yes\nStartLimitBurst=1\nStartLimitIntervalUSec=10min\nDropInPaths=\nFragmentPath={}",
                    root.join("cleanup.service").display()
                );
            }
            Some("start") => {
                fs::write(root.join("service-requested"), b"requested")?;
            }
            _ => return Err("unexpected synthetic systemctl command".into()),
        },
        "docker" => {
            let cid = fs::read_to_string(root.join("cid"))?;
            if argv.last() != Some(&cid)
                && argv.first().map(String::as_str) != Some("info")
                && argv.get(1).map(String::as_str) != Some("ls")
            {
                return Err("wrong synthetic CID".into());
            }
            match argv.first().map(String::as_str) {
                Some("info") => println!("{}", json!({"ID":"synthetic-daemon"})),
                Some("container") if argv.get(1).map(String::as_str) == Some("inspect") => {
                    if !root.join("container").exists() {
                        return Err("synthetic container absent".into());
                    }
                    println!(
                        "{}",
                        json!([{"Id":cid,"State":{"Running":false,"Restarting":false,"Dead":false,"Pid":0,"ExitCode":0,"StartedAt":"","FinishedAt":""}}])
                    );
                }
                Some("container") if argv.get(1).map(String::as_str) == Some("ls") => {
                    if root.join("container").exists() {
                        println!("{}", json!(cid));
                    }
                }
                Some("wait") => println!("0"),
                Some("rm") => {
                    fs::remove_file(root.join("container"))?;
                    println!("{cid}");
                }
                Some("kill") => {}
                _ => return Err("unexpected synthetic Docker command".into()),
            }
        }
        "curl" => {
            let cid = fs::read_to_string(root.join("cid"))?;
            if root.join("container").exists()
                || !argv
                    .last()
                    .is_some_and(|s| s.ends_with(&format!("/{cid}/json")))
            {
                return Err("synthetic HTTP lookup mismatch".into());
            }
            let body = json!({"message":format!("No such container: {cid}")}).to_string();
            print!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
        }
        _ => return Err("copy this synthetic peer as systemctl, docker or curl".into()),
    }
    Ok(())
}
