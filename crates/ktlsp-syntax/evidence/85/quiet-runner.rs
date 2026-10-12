use std::{collections::BTreeMap, fs::File, io::Write, path::Path, process::{Command, Stdio}, thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

fn inventory(root: &Path) {
    let result = Command::new("/bin/ps").args(["-A", "-o", "comm="]).output().unwrap();
    assert!(result.status.success());
    let text = std::str::from_utf8(&result.stdout).unwrap();
    let mut counts = BTreeMap::<String, usize>::new();
    for line in text.lines() {
        let name = Path::new(line.trim()).file_name().unwrap_or_default().to_string_lossy();
        if matches!(name.as_ref(), "cargo" | "rustc" | "rustfmt" | "clippy-driver" | "python3" | "Python")
            || name.starts_with("ktlsp") || line.contains("/target/debug/deps/") || line.contains("/target/release/deps/")
        {
            *counts.entry(name.into_owned()).or_default() += 1;
        }
    }
    let mut record = format!("inventory_epoch_ms={}\nmatching_process_count={}\n", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), counts.values().sum::<usize>());
    for (name, count) in counts.iter().take(64) { record.push_str(&format!("process_name={name:?} count={count}\n")); }
    record.push_str(&format!("distinct_names={} displayed_limit=64\n", counts.len()));
    let nano = match std::env::var("MallocNanoZone").as_deref() { Err(_) => "unset", Ok("0") => "0", Ok("1") => "1", _ => "other (value redacted)" };
    record.push_str(&format!("inherited_MallocNanoZone={nano}\n"));
    for (name, _) in std::env::vars_os().filter(|(k, _)| k.to_string_lossy().starts_with("Malloc") || k.to_string_lossy().starts_with("DYLD")) {
        record.push_str(&format!("allocator_override_name={name:?} value_not_recorded\n"));
    }
    std::fs::write(root.join("quiet-preinventory.txt"), record).unwrap();
    assert!(counts.is_empty(), "unexpected competing processes; stopped before the first measured child");
}

fn main() {
    let root = Path::new("/private/tmp/ktlsp-85");
    inventory(root);
    let mut output = File::create(root.join("quiet-primary.jsonl")).unwrap();
    let start_epoch_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let outer_deadline_epoch_ms = start_epoch_ms + 110000;
    std::fs::write(root.join("quiet-window.txt"), format!("{start_epoch_ms} {outer_deadline_epoch_ms}\n")).unwrap();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(90);
    let corpora = ["large", "short_tokens", "empty", "short", "whitespace", "comment", "string", "dense_sparse", "sparse_dense"];
    let revisions = ["historical", "current", "candidate"];
    let mut observations = 0;
    for round in 0..5 {
        for offset in 0..corpora.len() {
            let corpus = corpora[(offset + round * 2) % corpora.len()];
            let passes = match corpus { "large" | "short_tokens" => "3", "empty" | "short" => "100000", _ => "1" };
            for zone in if round % 2 == 0 { ["default", "0"] } else { ["0", "default"] } {
                for step in 0..3 {
                    let index = if round % 2 == 0 { (step + round) % 3 } else { (2 - step + round) % 3 };
                    let revision = revisions[index];
                    if Instant::now() >= deadline { panic!("primary budget expired after {observations} children"); }
                    let binary = root.join(format!("quiet-{revision}/crates/ktlsp-syntax/evidence/85/probe/target/release/ktlsp-allocation-probe"));
                    let mut command = Command::new(binary);
                    command.args([corpus, passes, "lex"]).env_remove("MallocNanoZone").stdout(Stdio::piped()).stderr(Stdio::piped());
                    if zone == "0" { command.env("MallocNanoZone", "0"); }
                    let mut child = command.spawn().unwrap();
                    loop {
                        if child.try_wait().unwrap().is_some() { break; }
                        if Instant::now() >= deadline { child.kill().unwrap(); child.wait().unwrap(); panic!("primary budget expired during child"); }
                        thread::sleep(Duration::from_millis(2));
                    }
                    let result = child.wait_with_output().unwrap();
                    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
                    let text = std::str::from_utf8(&result.stdout).unwrap().trim();
                    assert!(text.starts_with('{') && text.ends_with('}'));
                    let elapsed_ns = start.elapsed().as_nanos();
                    writeln!(output, "{{\"revision\":\"{revision}\",\"malloc_nano_zone\":\"{zone}\",\"round\":{round},\"order\":{observations},\"elapsed_ns\":{elapsed_ns},{}", &text[1..]).unwrap();
                    output.flush().unwrap();
                    observations += 1;
                }
            }
        }
    }
    println!("completed {observations} children in {:.3}s", start.elapsed().as_secs_f64());
}
