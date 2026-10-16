use std::io;

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let code = match arguments.as_slice() {
        [] | [_] if arguments.is_empty() || arguments[0] == "--stdio" => {
            match ktlsp_server::StdinInput::new() {
                Ok(input) => ktlsp_server::run(
                    &mut io::BufReader::new(input),
                    &mut io::stdout().lock(),
                    &mut io::stderr().lock(),
                ),
                Err(error) => {
                    eprintln!("ktlsp: {error}");
                    1
                }
            }
        }
        [argument] if argument == "--help" || argument == "-h" => {
            println!(
                "ktlsp — native Kotlin language server\n\nUsage: ktlsp [--stdio | --help | --version]\n\nWith no arguments or --stdio, runs LSP over stdio using UTF-16 incremental document synchronization.\nThe runtime and build require no JVM or JDK. Protocol messages use stdout; errors use stderr."
            );
            0
        }
        [argument] if argument == "--version" || argument == "-V" => {
            println!("ktlsp {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("ktlsp: unsupported arguments; use --help");
            2
        }
    };
    std::process::exit(code);
}
