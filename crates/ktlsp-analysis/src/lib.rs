//! Snapshot-owned Kotlin analysis through a trusted, isolated Rust worker.
//! Compiler state never enters the server. The existing parser supervisor owns
//! admission, cancellation, deadlines, pipe bounds and same-child cleanup.

use ktlsp_parser::{FileKind, Parser, ParserError, SourceSnapshot};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::atomic::AtomicBool};

pub const MAX_FILES: usize = 16;
pub const MAX_DIAGNOSTICS: usize = 256;
pub const MAX_EXPRESSIONS: usize = 4_096;
pub const MAX_MESSAGE_BYTES: usize = 4_096;
pub const MAX_TYPE_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub message: String,
    pub severity: Severity,
}

/// Original expression coordinates and the frontend's debug type spelling.
/// This is native type evidence, not official Kotlin hover/pretty-print output.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionType {
    pub start: usize,
    pub end: usize,
    pub ty: String,
}

#[derive(Debug)]
pub enum Error {
    Parser(ParserError),
    Resource,
    InvalidResponse,
    Unsupported,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parser(error) => write!(f, "analysis worker: {error}"),
            Self::Resource => f.write_str("analysis resource limit exceeded"),
            Self::InvalidResponse => f.write_str("invalid analysis worker response"),
            Self::Unsupported => f.write_str("analysis requires Kotlin source snapshots"),
        }
    }
}
impl std::error::Error for Error {}
impl From<ParserError> for Error {
    fn from(error: ParserError) -> Self {
        Self::Parser(error)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub sources: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAnalysis {
    pub diagnostics: Vec<Diagnostic>,
    pub expressions: Vec<ExpressionType>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Complete { files: Vec<FileAnalysis> },
    Resource,
}

pub struct Analyzer(Parser);
impl std::fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Analyzer(trusted native analysis worker)")
    }
}
impl Analyzer {
    pub fn new(worker: &Path) -> Result<Self, Error> {
        if !worker.is_absolute() {
            return Err(ParserError::Io.into());
        }
        let metadata = worker
            .metadata()
            .map_err(|_| Error::Parser(ParserError::Io))?;
        if !metadata.is_file() {
            return Err(ParserError::Io.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(ParserError::Io.into());
            }
        }
        Parser::new(worker).map(Self).map_err(Into::into)
    }

    pub fn analyze_snapshot_with_cleanup(
        &self,
        snapshot: &SourceSnapshot,
        cancelled: &AtomicBool,
    ) -> (Result<Analysis, Error>, bool) {
        self.analyze_snapshots_with_cleanup(std::slice::from_ref(snapshot), cancelled)
    }

    pub fn analyze_snapshots_with_cleanup(
        &self,
        snapshots: &[SourceSnapshot],
        cancelled: &AtomicBool,
    ) -> (Result<Analysis, Error>, bool) {
        if snapshots.is_empty() || snapshots.len() > MAX_FILES {
            return (Err(Error::Resource), true);
        }
        if snapshots
            .iter()
            .any(|snapshot| snapshot.kind() != FileKind::Source)
        {
            return (Err(Error::Unsupported), true);
        }
        let total = snapshots.iter().try_fold(0usize, |total, snapshot| {
            total.checked_add(snapshot.source().len())
        });
        if total.is_none_or(|bytes| bytes > ktlsp_parser::MAX_SOURCE_BYTES) {
            return (Err(Error::Resource), true);
        }
        // Borrow source text while encoding; the compiler process alone owns its
        // decoded source copies. JSON escaping counts against the transport cap.
        #[derive(Serialize)]
        struct BorrowedRequest<'a> {
            sources: &'a [&'a str],
        }
        let mut sources = [""; MAX_FILES];
        for (source, snapshot) in sources.iter_mut().zip(snapshots) {
            *source = snapshot.source();
        }
        let request = BorrowedRequest {
            sources: &sources[..snapshots.len()],
        };
        let Ok(request) = serde_json::to_string(&request) else {
            return (Err(Error::Resource), true);
        };
        let (reply, clean) = self.0.exchange_analysis_with_cleanup(&request, cancelled);
        let result = reply.map_err(Into::into).and_then(|bytes| {
            if !clean {
                return Err(Error::Parser(ParserError::Io));
            }
            let reply = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)?;
            let Reply::Complete { files } = reply else {
                return Err(Error::Resource);
            };
            validate(snapshots, &files)?;
            Ok(Analysis {
                snapshots: snapshots.to_vec(),
                files,
            })
        });
        (result, clean)
    }
}

#[derive(Debug)]
pub struct Analysis {
    snapshots: Vec<SourceSnapshot>,
    files: Vec<FileAnalysis>,
}
impl Analysis {
    pub fn files_for(&self, current: &[SourceSnapshot]) -> Result<&[FileAnalysis], Error> {
        if self.snapshots != current {
            return Err(Error::Parser(ParserError::StaleSnapshot));
        }
        Ok(&self.files)
    }
    pub fn diagnostics_for(&self, current: &SourceSnapshot) -> Result<&[Diagnostic], Error> {
        Ok(&self.files_for(std::slice::from_ref(current))?[0].diagnostics)
    }
}

fn validate(snapshots: &[SourceSnapshot], files: &[FileAnalysis]) -> Result<(), Error> {
    if files.len() != snapshots.len() {
        return Err(Error::InvalidResponse);
    }
    let mut diagnostics = 0usize;
    let mut expressions = 0usize;
    for (snapshot, file) in snapshots.iter().zip(files) {
        diagnostics = diagnostics
            .checked_add(file.diagnostics.len())
            .ok_or(Error::Resource)?;
        expressions = expressions
            .checked_add(file.expressions.len())
            .ok_or(Error::Resource)?;
        if diagnostics > MAX_DIAGNOSTICS || expressions > MAX_EXPRESSIONS {
            return Err(Error::Resource);
        }
        let valid_range = |start, end| start <= end && snapshot.source().get(start..end).is_some();
        for diagnostic in &file.diagnostics {
            if !valid_range(diagnostic.start, diagnostic.end)
                || diagnostic.message.len() > MAX_MESSAGE_BYTES
            {
                return Err(Error::InvalidResponse);
            }
        }
        for expression in &file.expressions {
            if !valid_range(expression.start, expression.end)
                || expression.start == expression.end
                || expression.ty.is_empty()
                || expression.ty.len() > MAX_TYPE_BYTES
            {
                return Err(Error::InvalidResponse);
            }
        }
    }
    Ok(())
}
