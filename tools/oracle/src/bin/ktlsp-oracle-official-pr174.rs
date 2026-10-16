//! Fixed protected-host entrypoint; source publication is not runtime admission.
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match ktlsp_oracle_replay::recorder::record_pr174(&args) {
        Ok(report) => {
            println!("{report}");
            if report["complete"] != true {
                std::process::exit(2);
            }
        }
        Err(error) => {
            eprintln!("official PR174 adapter refused: {error}");
            std::process::exit(2);
        }
    }
}
