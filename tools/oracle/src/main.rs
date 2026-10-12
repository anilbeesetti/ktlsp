use ktlsp_oracle_replay::{Evidence, Limits, replay};
use std::{env, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 4 || args[2] != "--" {
        eprintln!(
            "usage: ktlsp-oracle-replay FIXTURE_DIR DECLARED_NATIVE_REVISION -- EXECUTABLE [ARGS]"
        );
        return ExitCode::from(2);
    }
    let result = Evidence::load(Path::new(&args[0]))
        .and_then(|evidence| replay(&evidence, &args[3..], Limits::default()))
        .map(|mut report| {
            report["fixtureDirectory"] = args[0].clone().into();
            report["declaredNativeRevision"] = args[1].clone().into();
            report
        });
    match result {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            ExitCode::from(if report["matches"] == true { 0 } else { 1 })
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"outcome":"harnessFailure", "error":error})
            );
            ExitCode::from(2)
        }
    }
}
