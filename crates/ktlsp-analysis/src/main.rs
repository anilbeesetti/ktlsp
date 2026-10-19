//! One analysis per process. Never construct compiler state in the LSP server.
use krusty_frontend::{
    Limits,
    diag::{DiagSink, Severity as CompilerSeverity},
    features::LangFeatures,
    frontend::{self, PlatformProvider},
    language_version::LanguageVersion,
    libraries::EmptySymbolSource,
    source::SourceInput,
    types::Ty,
    with_limits,
};
use ktlsp_analysis::{
    Diagnostic, ExpressionType, FileAnalysis, MAX_DIAGNOSTICS, MAX_EXPRESSIONS, MAX_FILES,
    MAX_MESSAGE_BYTES, MAX_TYPE_BYTES, Reply, Request, Severity,
};
use std::io::{Read, Write};

#[path = "../../ktlsp-parser/src/worker_allocator.rs"]
mod worker_allocator;
#[global_allocator]
static ALLOCATOR: worker_allocator::LimitedAllocator = worker_allocator::LimitedAllocator;

fn main() {
    let success = run().is_ok();
    std::process::exit(worker_allocator::worker_exit_code(success));
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--analyze") || args.next().is_some() {
        return Err("invalid analysis worker mode".into());
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(ktlsp_parser::MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > ktlsp_parser::MAX_SOURCE_BYTES {
        return write_reply(&Reply::Resource);
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    let total = request
        .sources
        .iter()
        .try_fold(0usize, |total, source| total.checked_add(source.len()));
    if request.sources.is_empty()
        || request.sources.len() > MAX_FILES
        || total.is_none_or(|bytes| bytes > ktlsp_parser::MAX_SOURCE_BYTES)
    {
        return write_reply(&Reply::Resource);
    }
    // ponytail: one fresh compiler process per query. The fixed stack supplies
    // capacity, not a sufficiency proof; the parent contains abort/OOM/timeout.
    // Construct and destroy all non-Send compiler state on this one thread.
    let result = std::thread::Builder::new()
        .name("kotlin-analysis".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || analyze(request))?
        .join()
        .map_err(|_| "analysis thread panicked")?;
    write_reply(&result)
}

fn analyze(request: Request) -> Reply {
    let checked = with_limits(
        Limits {
            work: 2_097_152,
            depth: 128,
        },
        || {
            let inputs = request
                .sources
                .iter()
                .map(|source| SourceInput::kotlin(source))
                .collect::<Vec<_>>();
            let features = LangFeatures::for_language_version(LanguageVersion::V2_3);
            let mut diagnostics = DiagSink::new();
            let analysis = frontend::analyze_source_set_with_features(
                &inputs,
                PlatformProvider::jvm(Box::new(EmptySymbolSource)),
                &features,
                &mut diagnostics,
            );
            let mut files: Vec<FileAnalysis> = request
                .sources
                .iter()
                .map(|_| FileAnalysis {
                    diagnostics: Vec::new(),
                    expressions: Vec::new(),
                })
                .collect();
            if diagnostics.diags.len() > MAX_DIAGNOSTICS || analysis.files.len() != files.len() {
                return Reply::Resource;
            }
            let has_errors = diagnostics.has_errors();
            for diagnostic in diagnostics.diags {
                if diagnostic.msg.len() > MAX_MESSAGE_BYTES {
                    return Reply::Resource;
                }
                let Some(file) = files.get_mut(diagnostic.file as usize) else {
                    return Reply::Resource;
                };
                let span = diagnostic.editor_span.unwrap_or(diagnostic.span);
                file.diagnostics.push(Diagnostic {
                    start: span.lo as usize,
                    end: span.hi as usize,
                    message: diagnostic.msg,
                    severity: match diagnostic.severity {
                        CompilerSeverity::Error => Severity::Error,
                        CompilerSeverity::Warning => Severity::Warning,
                    },
                });
            }
            // Ordinary language errors retain their real diagnostics. Do not expose
            // recovered/approximate type tables as successful type evidence.
            if !has_errors {
                let mut count = 0usize;
                for ((file, types), output) in
                    analysis.files.iter().zip(&analysis.types).zip(&mut files)
                {
                    let Some(types) = types else { continue };
                    for (span, ty) in file.expr_spans.iter().zip(&types.expr_types) {
                        if *ty == Ty::Error || span.lo == span.hi {
                            continue;
                        }
                        count += 1;
                        if count > MAX_EXPRESSIONS {
                            return Reply::Resource;
                        }
                        let ty = format!("{ty:?}");
                        if ty.len() > MAX_TYPE_BYTES {
                            return Reply::Resource;
                        }
                        output.expressions.push(ExpressionType {
                            start: span.lo as usize,
                            end: span.hi as usize,
                            ty,
                        });
                    }
                }
            }
            Reply::Complete { files }
        },
    );
    checked.unwrap_or(Reply::Resource)
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > ktlsp_parser::MAX_OUTPUT_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("analysis output limit"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(std::io::Error::other)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn write_reply(reply: &Reply) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = BoundedOutput(Vec::new());
    if serde_json::to_writer(&mut output, reply).is_err() {
        std::io::stdout().write_all(b"{\"status\":\"resource\"}")?;
    } else {
        std::io::stdout().write_all(&output.0)?;
    }
    Ok(())
}
