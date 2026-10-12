use std::io;

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let code = match arguments.as_slice() {
        [] => ktlsp_server::run(
            &mut io::stdin().lock(),
            &mut io::stdout().lock(),
            &mut io::stderr().lock(),
        ),
        [argument] if argument == "--help" || argument == "-h" => {
            println!(
                "ktlsp — native Kotlin language server\n\nUsage: ktlsp [--help | --version]\n\nWith no arguments, runs LSP over stdio using UTF-16 incremental document synchronization.\nThe runtime and build require no JVM or JDK. Language providers and project import\nare not implemented in this foundation. Protocol messages use stdout; errors use stderr."
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
