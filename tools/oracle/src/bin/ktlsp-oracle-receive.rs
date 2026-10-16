//! Optional authenticated receiver. It never launches the official server.
use ktlsp_oracle_replay::{authority::{self, RootGrant}, receiver::{self, retrieval, verdict}};
use serde::Deserialize;
use serde_json::json;
use std::{fs::File, io::Read, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CalibrationInput { grant: RootGrant, artifact_id: u64 }
fn bounded(path: &Path, cap: usize) -> Result<Vec<u8>,String> {
    let mut bytes = Vec::new();
    File::open(path).map_err(|e|e.to_string())?.take(cap as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    receiver::require(bytes.len() <= cap,"local input limit")?;
    Ok(bytes)
}
fn run() -> Result<serde_json::Value,String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--synthetic" {
        let directory = Path::new(&args[1]);
        let mut files = receiver::Files::new();
        for name in ["manifest.json","metadata.json","transcript.json","client-intended.raw","stdout.raw","stderr.raw"] {
            let path = directory.join(name);
            for ancestor in path.ancestors() {
                receiver::require(!std::fs::symlink_metadata(ancestor).map_err(|e|e.to_string())?.file_type().is_symlink(),"synthetic symlink refused")?;
            }
            files.insert(format!("capture/{name}"),bounded(&path,receiver::FILE_LIMIT)?);
        }
        let transport = receiver::transport(&files,false)?;
        return Ok(json!({"schemaVersion":1,"scope":"synthetic-mechanics-only; not evidence","transport":transport,"provenance":"unauthenticated","runtime":"unverified","teardown":"unverified","officialSuccess":false}));
    }
    if args.len() != 3 { return Err("usage: ktlsp-oracle-receive ROOT-expected-grant.json ARTIFACT_ID ROOT-calibrations.json | --synthetic CAPTURE_DIRECTORY".into()); }
    let expected: RootGrant = authority::decode(&bounded(Path::new(&args[0]),authority::AUTHORITY_BYTES)?)?;
    expected.validate_shape()?;
    let artifact_id = args[1].to_str().ok_or("artifact ID encoding")?.parse::<u64>().map_err(|_|"artifact ID must be positive integer")?;
    let inputs: Vec<CalibrationInput> = serde_json::from_value(receiver::json_bytes(&bounded(Path::new(&args[2]),256*1024)?)?).map_err(|e|e.to_string())?;
    receiver::require(inputs.len() <= 10,"calibration grant count limit")?;
    let artifact = retrieval::retrieve(&expected,artifact_id)?;
    let mut calibrations = Vec::new();
    for input in inputs {
        input.grant.validate_shape()?;
        let artifact = retrieval::retrieve(&input.grant,input.artifact_id)?;
        calibrations.push((input.grant,artifact));
    }
    serde_json::to_value(verdict::evaluate(&artifact,&expected,&calibrations)).map_err(|e|e.to_string())
}
fn main() {
    match run() {
        Ok(report) => { println!("{report}"); if report["officialSuccess"] != true && report["scope"] != "synthetic-mechanics-only; not evidence" { std::process::exit(1); } }
        Err(error) => { eprintln!("receiver refused: {error}"); std::process::exit(1); }
    }
}
