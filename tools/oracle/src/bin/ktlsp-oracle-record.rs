use ktlsp_oracle_replay::recorder::{CaptureLimits, record};
use serde_json::Value;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Read,
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 5 || args[0] != "--synthetic-only" || args[3] != "--" {
        eprintln!(
            "usage: ktlsp-oracle-record --synthetic-only SCRIPT.json FRESH_OUTPUT_DIR -- ABSOLUTE_EXECUTABLE [ARGS]"
        );
        return ExitCode::from(2);
    }
    let result = (|| {
        let mut bytes = Vec::new();
        {
            if !fs::symlink_metadata(&args[1])
                .map_err(|e| e.to_string())?
                .is_file()
            {
                return Err("script must be a regular file".into());
            }
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                #[cfg(target_os = "linux")]
                const NONBLOCK: i32 = 0x800;
                #[cfg(not(target_os = "linux"))]
                const NONBLOCK: i32 = 4;
                options.custom_flags(NONBLOCK);
            }
            let input = options.open(&args[1]).map_err(|e| e.to_string())?;
            if !input.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("script must be a regular file".into());
            }
            input
        }
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("script byte limit".into());
        }
        let script: Vec<Value> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        record(
            &script,
            &args[4..],
            Path::new(&args[2]),
            CaptureLimits::default(),
        )
    })();
    match result {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            ExitCode::from(if report["complete"] == true { 0 } else { 2 })
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"complete":false,"failureReason":error})
            );
            ExitCode::from(2)
        }
    }
}
