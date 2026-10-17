//! Native LSP lifecycle, bounded synchronization, lexical/structural providers
//! and finite authenticated local navigation. JSON source snapshots are opt-in.

mod cache;
mod document_symbols;
mod file_uri;
mod input;
mod semantic_navigation;
mod semantic_tokens;
pub use input::StdinInput;
mod structure;
mod uri;
mod workspace;
use ktlsp_parser::{FileKind, ParserError, SourceSnapshot};
use ktlsp_protocol::{FrameReader, write_frame};
use ktlsp_text::{Document, MAX_CHANGES, TextChange};
use serde_json::{Map, Number, Value, json};
use std::task::Poll;
use std::{
    collections::HashMap,
    io::{BufRead, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const MAX_OPEN_DOCUMENTS: usize = 128;
pub const MAX_OPEN_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_URI_BYTES: usize = 8 * 1024;
pub const MAX_ERROR_RECORDS: usize = 128;
pub const MAX_WORKSPACE_FOLDERS: usize = 128;
/// Includes cancelled work until its thread has actually terminated.
pub const MAX_PENDING_PARSER_REQUESTS: usize = 8;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);
// Parser and native import jobs share the same bounded owned-thread registry.
static PARSER_THREADS: Mutex<[Option<ParserThread>; MAX_PENDING_PARSER_REQUESTS]> =
    Mutex::new([const { None }; MAX_PENDING_PARSER_REQUESTS]);

struct ParserThread {
    handle: JoinHandle<()>,
    // 0: retained unfinished thread, 1: joined successfully, 2: joined panic.
    outcome: Arc<AtomicU8>,
}
fn reap_parser_threads(threads: &mut [Option<ParserThread>; MAX_PENDING_PARSER_REQUESTS]) {
    for slot in threads {
        if slot
            .as_ref()
            .is_some_and(|thread| thread.handle.is_finished())
        {
            let thread = slot.take().expect("finished handle checked");
            let outcome = if thread.handle.join().is_ok() { 1 } else { 2 };
            thread.outcome.store(outcome, Ordering::Relaxed);
        }
    }
}

type ParserResult = Result<Value, ParserFailure>;

#[derive(Debug)]
enum ParserFailure {
    Protocol(i32, &'static str),
    Source(ktlsp_import::source_inventory::Error),
}
impl ParserFailure {
    fn response(self, id: Value) -> Value {
        match self {
            Self::Protocol(code, message) => response_error(id, code, message),
            Self::Source(error) => {
                let code = if matches!(error, ktlsp_import::source_inventory::Error::Cancelled) {
                    -32800
                } else {
                    -32801
                };
                let mut response = response_error(
                    id,
                    code,
                    "workspace source snapshot is stale or unavailable; reload required",
                );
                response["error"]["data"] = json!({"reason":null});
                response["error"]["data"]["reason"] = error.to_string().into();
                response
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ParserAuthority {
    Open {
        incarnation: u64,
    },
    Imported {
        snapshot: Arc<workspace::Snapshot>,
        key: workspace::SourceKey,
        epoch: u64,
    },
}
impl ParserAuthority {
    fn verify(
        &self,
        budget: &mut ktlsp_import::source_inventory::Budget,
        cancelled: &AtomicBool,
    ) -> Result<(), ParserFailure> {
        if let Self::Imported { snapshot, key, .. } = self {
            snapshot
                .verify(*key, budget, cancelled)
                .map_err(ParserFailure::Source)?;
        }
        Ok(())
    }
}
#[derive(Debug)]
struct PendingParser {
    id: Option<Value>,
    snapshot: SourceSnapshot,
    authority: ParserAuthority,
    cancelled: Arc<AtomicBool>,
    result: Receiver<(ParserResult, bool)>,
    outcome: Arc<AtomicU8>,
}
impl PendingParser {
    fn cancel(&mut self, code: i32, message: &str) -> Option<Value> {
        self.cancelled.store(true, Ordering::Relaxed);
        self.id.take().map(|id| response_error(id, code, message))
    }
}

#[derive(Debug)]
struct PendingImport {
    id: Option<Value>,
    attempt: u64,
    cancelled: Arc<AtomicBool>,
    result: Receiver<Result<workspace::Snapshot, String>>,
    outcome: Arc<AtomicU8>,
}
impl PendingImport {
    fn cancel(&mut self, code: i32, message: &str) -> Option<Value> {
        self.cancelled.store(true, Ordering::Relaxed);
        self.id.take().map(|id| response_error(id, code, message))
    }
}

#[derive(Debug)]
struct WaitingSymbol {
    id: Value,
    uri: Arc<str>,
    attempt: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Uninitialized,
    Initializing,
    Running,
    Shutdown,
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
}
/// Stored client configuration, not evidence of a successful workspace import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    pub process_id: Option<Number>,
    pub root_uri: Option<String>,
    pub workspace_folders: Vec<WorkspaceFolder>,
    pub line_folding_only: bool,
    pub folding_range_limit: usize,
    pub diagnostic_version_support: bool,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            process_id: None,
            root_uri: None,
            workspace_folders: Vec::new(),
            line_folding_only: false,
            folding_range_limit: structure::MAX_FOLDS,
            diagnostic_version_support: false,
        }
    }
}

#[derive(Debug)]
pub struct Session {
    state: State,
    documents: HashMap<String, Document>,
    analysis_cache: cache::Cache,
    pull_diagnostics: bool,
    open_bytes: usize,
    errors: Vec<String>,
    configuration: Configuration,
    parser_provider: Option<Arc<document_symbols::Provider>>,
    symbol_client: document_symbols::Client,
    hover_format: Option<semantic_navigation::HoverFormat>,
    rename_client: semantic_navigation::RenameClient,
    semantic_tokens: Option<semantic_tokens::Client>,
    code_actions: bool,
    exit_code: Option<i32>,
    incarnations: HashMap<String, u64>,
    next_incarnation: u64,
    pending_parser: Vec<PendingParser>,
    workspace: workspace::Workspace,
    pending_import: Option<PendingImport>,
    waiting_symbols: Vec<WaitingSymbol>,
    cleanup_confirmed: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn response_error(id: Value, code: i32, message: &str) -> Value {
    let mut response = json!({"jsonrpc":"2.0", "id":null, "error":{"code":code,"message":message}});
    response["id"] = id;
    response
}
fn valid_id(id: &Value) -> bool {
    id.is_string() || id.as_number().is_some_and(|n| n.is_i64() || n.is_u64())
}
fn object(value: &Value) -> Result<&Map<String, Value>, &'static str> {
    value.as_object().ok_or("params must be an object")
}
fn required<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a Value, &'static str> {
    map.get(key).ok_or("required parameter missing")
}
fn uri_value(value: &Value) -> Result<&str, &'static str> {
    let value = value.as_str().ok_or("URI must be a string")?;
    if value.len() > MAX_URI_BYTES || !uri::absolute(value) {
        return Err("invalid or oversized absolute URI");
    }
    Ok(value)
}
fn empty_params(params: Option<&Value>) -> Result<(), &'static str> {
    if params.is_none_or(|p| p.is_null() || p.is_object()) {
        Ok(())
    } else {
        Err("params must be an object or null")
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: State::Uninitialized,
            documents: HashMap::new(),
            analysis_cache: cache::Cache::default(),
            pull_diagnostics: false,
            open_bytes: 0,
            errors: Vec::new(),
            configuration: Configuration::default(),
            parser_provider: None,
            symbol_client: document_symbols::Client::default(),
            hover_format: Some(semantic_navigation::HoverFormat::Plaintext),
            rename_client: semantic_navigation::RenameClient::default(),
            semantic_tokens: None,
            code_actions: false,
            exit_code: None,
            incarnations: HashMap::new(),
            next_incarnation: 0,
            pending_parser: Vec::new(),
            workspace: workspace::Workspace::default(),
            pending_import: None,
            waiting_symbols: Vec::new(),
            cleanup_confirmed: true,
        }
    }
    /// Enable parser-backed symbols and finite local navigation with a trusted worker.
    /// The caller, not document content, chooses this absolute executable path.
    pub fn with_parser(worker: impl AsRef<Path>) -> Result<Self, ParserError> {
        let provider = document_symbols::Provider::new(worker.as_ref())?;
        let mut session = Self::new();
        session.parser_provider = Some(Arc::new(provider));
        Ok(session)
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn document(&self, uri: &str) -> Option<&Document> {
        self.documents.get(uri)
    }
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    pub fn open_document_count(&self) -> usize {
        self.documents.len()
    }
    pub fn open_bytes(&self) -> usize {
        self.open_bytes
    }
    /// Lifecycle exit intent, not cleanup proof. A successful owner must also
    /// require the true cleanup outcome from `close()` before reporting success.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
    /// Notification failures are bounded records, never protocol responses.
    pub fn drain_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }
    fn record(&mut self, message: impl Into<String>) {
        if self.errors.len() < MAX_ERROR_RECORDS {
            self.errors.push(message.into());
        } else if let Some(last) = self.errors.last_mut() {
            *last = "additional notification errors suppressed".into();
        }
    }
    /// Handle one decoded message. JSON-RPC batches are invalid in LSP.
    /// Parser-backed requests are deferred: call `poll` while servicing input.
    /// Notifications never receive errors; valid client responses are ignored.
    pub fn handle(&mut self, message: Value) -> Vec<Value> {
        if self.state == State::Exited {
            return Vec::new();
        }
        let Some(map) = message.as_object() else {
            return vec![response_error(Value::Null, -32600, "Invalid Request")];
        };
        let notification = map.contains_key("method") && !map.contains_key("id");
        let invalid = |session: &mut Self| {
            if notification {
                session.record("invalid JSON-RPC notification");
                Vec::new()
            } else {
                vec![response_error(Value::Null, -32600, "Invalid Request")]
            }
        };
        if map.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return invalid(self);
        }
        // There are no server-originated requests to resolve in this foundation.
        if !map.contains_key("method") {
            let id_ok = map.get("id").is_some_and(|id| valid_id(id) || id.is_null());
            let error_ok = map.get("error").is_some_and(|error| {
                error.as_object().is_some_and(|e| {
                    e.get("code").is_some_and(|n| n.as_i64().is_some())
                        && e.get("message").is_some_and(Value::is_string)
                })
            });
            if id_ok
                && (map.contains_key("result") ^ map.contains_key("error"))
                && (!map.contains_key("error") || error_ok)
            {
                return Vec::new();
            }
            return invalid(self);
        }
        let Some(method) = map.get("method").and_then(Value::as_str) else {
            return invalid(self);
        };
        if map.contains_key("result") || map.contains_key("error") {
            return invalid(self);
        }
        let params = map.get("params");
        if notification {
            return self.notification(method, params);
        }
        let id = map.get("id").expect("request id presence checked");
        if !valid_id(id) {
            return invalid(self);
        }
        if self
            .pending_parser
            .iter()
            .any(|pending| pending.id.as_ref() == Some(id))
            || self
                .pending_import
                .as_ref()
                .is_some_and(|pending| pending.id.as_ref() == Some(id))
            || self.waiting_symbols.iter().any(|pending| &pending.id == id)
        {
            return vec![response_error(
                Value::Null,
                -32600,
                "request id is already active",
            )];
        }
        if method == "intellij/reloadWorkspace" && self.state == State::Running {
            let projects = params
                .ok_or("reload params missing")
                .and_then(object)
                .and_then(|map| required(map, "initializationOptions"))
                .and_then(|options| workspace::projects(Some(options)));
            return match projects {
                Ok(projects) => self.start_import(Some(id.clone()), projects),
                Err(error) => vec![response_error(id.clone(), -32602, error)],
            };
        }
        if matches!(
            method,
            "textDocument/documentSymbol"
                | "textDocument/definition"
                | "textDocument/hover"
                | "textDocument/references"
                | "textDocument/documentHighlight"
                | "textDocument/prepareRename"
                | "textDocument/rename"
                | "textDocument/completion"
        ) && self.state == State::Running
        {
            return match self.start_parser(id.clone(), method, params) {
                Ok(()) => Vec::new(),
                Err((code, message)) => vec![response_error(id.clone(), code, message)],
            };
        }
        let result = self.request(method, params);
        let mut responses = if method == "shutdown" && result.is_ok() {
            self.cancel_all(-32800, "Request cancelled by shutdown")
        } else {
            Vec::new()
        };
        responses.push(match result {
            Ok(result) => json!({"jsonrpc":"2.0", "id":id, "result":result}),
            Err((code, message)) => response_error(id.clone(), code, message),
        });
        responses
    }
    fn request(
        &mut self,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, (i32, &'static str)> {
        if self.state == State::Shutdown {
            return Err((-32600, "server has shut down"));
        }
        if method == "initialize" {
            if self.state != State::Uninitialized {
                return Err((-32600, "initialize may only be called once"));
            }
            let (configuration, pull_diagnostics) =
                parse_initialize(params).map_err(|e| (-32602, e))?;
            let symbol_client = document_symbols::Client::parse(
                params
                    .and_then(|p| p.get("capabilities"))
                    .expect("validated capabilities"),
            )
            .map_err(|e| (-32602, e))?;
            let rename_client = semantic_navigation::RenameClient::parse(
                params
                    .and_then(|p| p.get("capabilities"))
                    .expect("validated capabilities"),
            )
            .map_err(|e| (-32602, e))?;
            let code_actions = parse_code_action_capability(
                params
                    .and_then(|p| p.get("capabilities"))
                    .expect("validated capabilities"),
            )
            .map_err(|e| (-32602, e))?
                && rename_client.document_changes;
            let projects = workspace::projects(params.and_then(|p| p.get("initializationOptions")))
                .map_err(|e| (-32602, e))?;
            self.workspace.initial = projects;
            self.code_actions = code_actions;
            self.rename_client = rename_client;
            self.symbol_client = symbol_client;
            self.hover_format = semantic_navigation::HoverFormat::negotiate(
                params
                    .and_then(|p| p.get("capabilities"))
                    .expect("validated capabilities"),
            );
            self.semantic_tokens = semantic_tokens::Client::negotiate(
                params
                    .and_then(|p| p.get("capabilities"))
                    .expect("validated capabilities"),
            );
            self.configuration = configuration;
            self.pull_diagnostics = pull_diagnostics;
            self.state = State::Initializing;
            let mut result = json!({"capabilities":{"positionEncoding":"utf-16", "foldingRangeProvider":true, "textDocumentSync":{"openClose":true,"change":2,"save":false}}, "serverInfo":{"name":"ktlsp","version":env!("CARGO_PKG_VERSION")}});
            if self.code_actions {
                result["capabilities"]["codeActionProvider"] =
                    json!({"codeActionKinds":["quickfix"],"resolveProvider":false});
            }
            if let Some(client) = &self.semantic_tokens {
                result["capabilities"]["semanticTokensProvider"] = client.options();
            }
            if self.parser_provider.is_some() {
                result["capabilities"]["documentSymbolProvider"] = json!(true);
                result["capabilities"]["definitionProvider"] = json!(true);
                result["capabilities"]["referencesProvider"] = json!(true);
                result["capabilities"]["documentHighlightProvider"] = json!(true);
                result["capabilities"]["completionProvider"] = json!({"resolveProvider":false});
                if self.hover_format.is_some() {
                    result["capabilities"]["hoverProvider"] = json!(true);
                }
                if self.rename_client.document_changes {
                    result["capabilities"]["renameProvider"] = if self.rename_client.prepare {
                        json!({"prepareProvider":true})
                    } else {
                        json!(true)
                    };
                }
            }
            if pull_diagnostics {
                result["capabilities"]["diagnosticProvider"] =
                    json!({"interFileDependencies":false,"workspaceDiagnostics":false});
            }
            return Ok(result);
        }
        if self.state != State::Running {
            return Err((-32002, "Server not initialized"));
        }
        match method {
            "shutdown" => {
                empty_params(params).map_err(|e| (-32602, e))?;
                self.state = State::Shutdown;
                Ok(Value::Null)
            }
            "textDocument/diagnostic" if self.pull_diagnostics => {
                let map = params
                    .ok_or((-32602, "diagnostic params missing"))
                    .and_then(|p| object(p).map_err(|e| (-32602, e)))?;
                for key in ["identifier", "previousResultId"] {
                    if map.get(key).is_some_and(|value| !value.is_string()) {
                        return Err((-32602, "diagnostic identifiers must be strings"));
                    }
                }
                let uri = required(map, "textDocument")
                    .and_then(object)
                    .and_then(|d| required(d, "uri"))
                    .and_then(uri_value)
                    .map_err(|e| (-32602, e))?;
                let (_, diagnostics) = self
                    .document_diagnostics(uri)
                    .ok_or((-32602, "diagnostic refers to unopened document"))?;
                Ok(json!({"kind":"full","items":diagnostics}))
            }
            "textDocument/codeAction" if self.code_actions => {
                let map = params
                    .ok_or((-32602, "code action params missing"))
                    .and_then(|p| object(p).map_err(|e| (-32602, e)))?;
                let uri = required(map, "textDocument")
                    .and_then(object)
                    .and_then(|d| required(d, "uri"))
                    .and_then(uri_value)
                    .map_err(|e| (-32602, e))?;
                let range = required(map, "range")
                    .and_then(range_value)
                    .map_err(|e| (-32602, e))?;
                let permits = required(map, "context")
                    .and_then(code_action_context)
                    .map_err(|e| (-32602, e))?;
                let document = self
                    .documents
                    .get(uri)
                    .ok_or((-32602, "code action refers to unopened document"))?;
                let start = ktlsp_text::position_to_offset(document.text(), range.start)
                    .map_err(|_| (-32602, "invalid UTF-16 code action range"))?;
                let end = ktlsp_text::position_to_offset(document.text(), range.end)
                    .map_err(|_| (-32602, "invalid UTF-16 code action range"))?;
                let analysis = self
                    .analysis_cache
                    .get(uri, document, self.configuration.folding_range_limit)
                    .map_err(|e| (-32803, e))?;
                if permits
                    && document.version() != i32::MAX
                    && let Some((token_start, token_end, title)) =
                        analysis.trailing_closer(document.text())
                    && (if start == end {
                        token_start <= start && start <= token_end
                    } else {
                        start < token_end && token_start < end
                    })
                {
                    let edit_start = ktlsp_text::offset_to_position(document.text(), token_start)
                        .map_err(|_| (-32803, "invalid structural closer range"))?;
                    // The authenticated closer is one ASCII code unit, not a newline.
                    let edit_end = ktlsp_text::Position {
                        line: edit_start.line,
                        character: edit_start.character + 1,
                    };
                    return Ok(json!([{
                        "title":title,
                        "kind":"quickfix",
                        "edit":{"documentChanges":[{
                            "textDocument":{"uri":uri,"version":document.version()},
                            "edits":[{"range":ktlsp_text::Range {start:edit_start,end:edit_end},"newText":""}]
                        }]}
                    }]));
                }
                Ok(json!([]))
            }
            "textDocument/semanticTokens/full" | "textDocument/semanticTokens/range" => {
                let range_request = method == "textDocument/semanticTokens/range";
                let client = self
                    .semantic_tokens
                    .as_ref()
                    .filter(|client| client.supports(range_request))
                    .ok_or((-32601, "Method not found"))?;
                let map = params
                    .ok_or((-32602, "semantic token params missing"))
                    .and_then(|p| object(p).map_err(|e| (-32602, e)))?;
                let uri = required(map, "textDocument")
                    .and_then(object)
                    .and_then(|d| required(d, "uri"))
                    .and_then(uri_value)
                    .map_err(|e| (-32602, e))?;
                let document = self
                    .documents
                    .get(uri)
                    .ok_or((-32602, "semantic tokens refer to unopened document"))?;
                let range = if range_request {
                    Some(
                        serde_json::from_value::<ktlsp_text::Range>(
                            required(map, "range").map_err(|e| (-32602, e))?.clone(),
                        )
                        .map_err(|_| (-32602, "invalid UTF-16 semantic token range"))?,
                    )
                } else {
                    None
                };
                client.render(document.text(), range)
            }
            "textDocument/foldingRange" => {
                let map = params
                    .ok_or((-32602, "folding params missing"))
                    .and_then(|p| object(p).map_err(|e| (-32602, e)))?;
                let uri = required(map, "textDocument")
                    .and_then(object)
                    .and_then(|d| required(d, "uri"))
                    .and_then(uri_value)
                    .map_err(|e| (-32602, e))?;
                let document = self
                    .documents
                    .get(uri)
                    .ok_or((-32602, "folding refers to unopened document"))?;
                let analysis = self
                    .analysis_cache
                    .get(uri, document, self.configuration.folding_range_limit)
                    .map_err(|e| (-32803, e))?;
                let (folds, _) =
                    analysis.render(document.text(), self.configuration.line_folding_only);
                Ok(json!(folds))
            }
            _ => Err((-32601, "Method not found")),
        }
    }
    fn start_parser(
        &mut self,
        id: Value,
        method: &str,
        params: Option<&Value>,
    ) -> Result<(), (i32, &'static str)> {
        if self.parser_provider.is_none() {
            return Err((-32601, "Method not found"));
        }
        if method == "textDocument/hover" && self.hover_format.is_none() {
            return Err((-32601, "Method not found"));
        }
        if matches!(method, "textDocument/rename" | "textDocument/prepareRename")
            && (!self.rename_client.document_changes
                || (method == "textDocument/prepareRename" && !self.rename_client.prepare))
        {
            return Err((-32601, "Method not found"));
        }
        let map = params
            .ok_or((-32602, "parser request params missing"))
            .and_then(|p| object(p).map_err(|e| (-32602, e)))?;
        let uri = required(map, "textDocument")
            .and_then(object)
            .and_then(|d| required(d, "uri"))
            .and_then(uri_value)
            .map_err(|e| (-32602, e))?;
        self.start_parser_uri(id, method, uri, Some(map))
    }

    fn start_parser_uri(
        &mut self,
        id: Value,
        method: &str,
        uri: &str,
        map: Option<&Map<String, Value>>,
    ) -> Result<(), (i32, &'static str)> {
        let document_uri = if method == "textDocument/documentSymbol" {
            self.workspace.overlay_uri(uri).unwrap_or(uri)
        } else {
            uri
        };
        let document = self.documents.get(document_uri);
        if document.is_some_and(|document| document.text().len() > ktlsp_parser::MAX_SOURCE_BYTES) {
            return Err((-32803, "request exceeds the parser's 64 KiB source limit"));
        }
        if document.is_none()
            && method == "textDocument/documentSymbol"
            && self.pending_import.as_ref().is_some_and(|pending| {
                !pending.cancelled.load(Ordering::Relaxed)
                    && pending.attempt == self.workspace.attempt
            })
        {
            if self.pending_requests() >= MAX_PENDING_PARSER_REQUESTS {
                return Err((-32803, "native analysis capacity exhausted"));
            }
            let canonical = workspace::canonical_uri(uri).map_err(|e| (-32803, e))?;
            self.waiting_symbols.push(WaitingSymbol {
                id,
                uri: canonical.into(),
                attempt: self.workspace.attempt,
            });
            return Ok(());
        }
        let query = if method == "textDocument/documentSymbol" {
            None
        } else {
            let map = map.ok_or((-32602, "parser request params missing"))?;
            let document =
                document.ok_or((-32602, "parser request refers to unopened document"))?;
            let position = map
                .get("position")
                .ok_or((-32602, "position missing"))
                .and_then(|value| {
                    serde_json::from_value::<ktlsp_text::Position>(value.clone())
                        .map_err(|_| (-32602, "invalid UTF-16 position"))
                })?;
            let offset = ktlsp_text::position_to_offset(document.text(), position)
                .map_err(|_| (-32602, "invalid UTF-16 position"))?;
            Some(match method {
                "textDocument/definition" => semantic_navigation::Query::Definition { offset },
                "textDocument/references" => {
                    let include_declaration = required(map, "context")
                        .and_then(object)
                        .and_then(|context| required(context, "includeDeclaration"))
                        .and_then(|value| {
                            value
                                .as_bool()
                                .ok_or("includeDeclaration must be a boolean")
                        })
                        .map_err(|e| (-32602, e))?;
                    semantic_navigation::Query::References {
                        offset,
                        include_declaration,
                    }
                }
                "textDocument/documentHighlight" => {
                    semantic_navigation::Query::Highlights { offset }
                }
                "textDocument/prepareRename" => {
                    semantic_navigation::Query::PrepareRename { offset }
                }
                "textDocument/rename" => {
                    let name = required(map, "newName")
                        .and_then(|v| v.as_str().ok_or("newName must be a string"))
                        .map_err(|e| (-32602, e))?;
                    if !ktlsp_parser::local_value_binding::valid_local_rename_name(name) {
                        return Err((-32602, "unsupported ASCII rename spelling"));
                    }
                    semantic_navigation::Query::Rename {
                        offset,
                        new_name: name.to_owned(),
                    }
                }
                "textDocument/completion" => semantic_navigation::Query::Completion { offset },
                "textDocument/hover" => semantic_navigation::Query::Hover {
                    offset,
                    format: self.hover_format.expect("hover format checked"),
                },
                _ => return Err((-32601, "Method not found")),
            })
        };
        if self.pending_requests() >= MAX_PENDING_PARSER_REQUESTS {
            return Err((-32803, "parser worker capacity exhausted"));
        }
        let mut threads = PARSER_THREADS
            .try_lock()
            .map_err(|_| (-32803, "parser worker admission unavailable"))?;
        reap_parser_threads(&mut threads);
        let slot = threads
            .iter()
            .position(Option::is_none)
            .ok_or((-32803, "parser worker capacity exhausted"))?;
        let (snapshot, authority) = if let Some(document) = document {
            (
                SourceSnapshot::new(
                    document_uri,
                    i64::from(document.version()),
                    FileKind::Source,
                    document.text(),
                )
                .map_err(|_| (-32803, "snapshot exceeds parser limits"))?,
                ParserAuthority::Open {
                    incarnation: self.incarnations[document_uri],
                },
            )
        } else {
            if self.workspace.attempt == 0 {
                return Err((-32602, "parser request refers to unopened document"));
            }
            let (catalog, key, canonical, source) =
                self.workspace.capture(uri).map_err(|e| (-32803, e))?;
            let snapshot =
                SourceSnapshot::new(canonical, catalog.generation, FileKind::Source, source)
                    .map_err(|_| (-32803, "snapshot exceeds parser limits"))?;
            (
                snapshot,
                ParserAuthority::Imported {
                    snapshot: catalog,
                    key,
                    epoch: self.workspace.epoch,
                },
            )
        };
        let worker_snapshot = snapshot.clone();
        let worker_authority = authority.clone();
        let provider = Arc::clone(
            self.parser_provider
                .as_ref()
                .ok_or((-32601, "Method not found"))?,
        );
        let client = self.symbol_client.clone();
        // Bound the complete response envelope without copying or serializing
        // the arbitrary client id. A JSON string byte needs at most six bytes.
        let response_id_bound = id
            .as_str()
            .map_or(Some(20), |id| {
                id.len()
                    .checked_mul(6)
                    .and_then(|bytes| bytes.checked_add(2))
            })
            .ok_or((-32803, "hover response id limit exceeded"))?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let (sender, result) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("ktlsp-parser".into())
            .spawn(move || {
                let mut cleanup_confirmed = true;
                let mut import_budget =
                    matches!(&worker_authority, ParserAuthority::Imported { .. })
                        .then(ktlsp_import::source_inventory::Budget::default);
                let result = (|| {
                    if let Some(budget) = import_budget.as_mut() {
                        worker_authority.verify(budget, &worker_cancelled)?;
                    }
                    let (parsed, clean) = provider.parse(&worker_snapshot, &worker_cancelled);
                    cleanup_confirmed &= clean;
                    let parsed = parsed
                        .map_err(|error| match error {
                            ParserError::Cancelled => (-32800, "Request cancelled"),
                            ParserError::InputLimit => (-32803, "parser input limit exceeded"),
                            ParserError::NestingLimit => (-32803, "parser nesting limit exceeded"),
                            ParserError::OutputLimit => (-32803, "parser output limit exceeded"),
                            ParserError::AllocationLimit => {
                                (-32803, "parser worker allocation limit exceeded")
                            }
                            ParserError::Coverage => {
                                (-32803, "parser output coverage validation failed")
                            }
                            ParserError::InvalidTree => {
                                (-32803, "parser output tree validation failed")
                            }
                            ParserError::Timeout => (-32803, "parser deadline exceeded"),
                            ParserError::WorkerFailed => (-32803, "parser worker failed"),
                            ParserError::Io => (-32803, "parser I/O or cleanup failed"),
                            ParserError::StaleSnapshot => {
                                (-32803, "parser output snapshot mismatch")
                            }
                        })
                        .map_err(|(code, message)| ParserFailure::Protocol(code, message))?;
                    if worker_cancelled.load(Ordering::Relaxed) {
                        return Err(ParserFailure::Protocol(-32800, "Request cancelled"));
                    }
                    let rendered = if let Some(query) = query {
                        semantic_navigation::render(
                            &parsed,
                            &worker_snapshot,
                            query,
                            &worker_cancelled,
                            response_id_bound,
                        )
                        .map_err(|error| ParserFailure::Protocol(-32803, error))
                    } else {
                        let tree = parsed.tree_for(&worker_snapshot).map_err(|_| {
                            ParserFailure::Protocol(-32801, "parser snapshot is stale")
                        })?;
                        document_symbols::render(tree, &client, worker_snapshot.document())
                            .map_err(|error| ParserFailure::Protocol(-32803, error))
                    }?;
                    if let Some(budget) = import_budget.as_mut() {
                        worker_authority.verify(budget, &worker_cancelled)?;
                    }
                    Ok(rendered)
                })();
                let _ = sender.send((result, cleanup_confirmed));
            })
            .map_err(|_| (-32803, "parser worker could not start"))?;
        let outcome = Arc::new(AtomicU8::new(0));
        threads[slot] = Some(ParserThread {
            handle: thread,
            outcome: Arc::clone(&outcome),
        });
        self.pending_parser.push(PendingParser {
            id: Some(id),
            snapshot,
            authority,
            cancelled,
            result,
            outcome,
        });
        Ok(())
    }

    fn import_response(&mut self, id: Option<Value>, result: Result<(), String>) -> Option<Value> {
        match (id, result) {
            (Some(id), Ok(())) => {
                let mut response = json!({"jsonrpc":"2.0", "id":null, "result":null});
                response["id"] = id;
                Some(response)
            }
            (Some(id), Err(reason)) => {
                let mut response =
                    response_error(id, -32803, "workspace import failed; reload required");
                response["error"]["data"] = json!({"reason":null});
                response["error"]["data"]["reason"] = reason.into();
                Some(response)
            }
            (None, Err(reason)) => {
                // Native model identifiers are untrusted terminal text.
                self.record(
                    serde_json::to_string(&reason)
                        .expect("String JSON serialization is infallible"),
                );
                None
            }
            (None, Ok(())) => None,
        }
    }

    fn start_import(&mut self, id: Option<Value>, projects: Vec<workspace::Project>) -> Vec<Value> {
        let mut responses = self.cancel_import(-32800, "Workspace import superseded");
        responses.extend(self.cancel_imported(-32801, "workspace snapshot is stale"));
        let admission = (|| {
            let (attempt, generation) = self
                .workspace
                .begin(projects.is_empty())
                .map_err(str::to_owned)?;
            let Some(generation) = generation else {
                return Ok(None);
            };
            if projects
                .iter()
                .any(|p| matches!(p, workspace::Project::Unsupported))
            {
                return Err(
                    "only explicit JSON projects with type/path fields are supported".into(),
                );
            }
            if self.pending_import.is_some() {
                return Err("previous workspace import is still owned; capacity exhausted".into());
            }
            if self.pending_requests() >= MAX_PENDING_PARSER_REQUESTS {
                return Err("native analysis capacity exhausted".into());
            }
            let mut threads = PARSER_THREADS
                .try_lock()
                .map_err(|_| "native worker admission unavailable".to_owned())?;
            reap_parser_threads(&mut threads);
            let slot = threads
                .iter()
                .position(Option::is_none)
                .ok_or_else(|| "native worker capacity exhausted".to_owned())?;
            let cancelled = Arc::new(AtomicBool::new(false));
            let worker_cancelled = Arc::clone(&cancelled);
            let (sender, result) = mpsc::sync_channel(1);
            let thread = thread::Builder::new()
                .name("ktlsp-json-import".into())
                .spawn(move || {
                    let _ = sender.send(workspace::Snapshot::load(
                        projects,
                        generation,
                        &worker_cancelled,
                    ));
                })
                .map_err(|error| format!("workspace import thread could not start: {error}"))?;
            let outcome = Arc::new(AtomicU8::new(0));
            threads[slot] = Some(ParserThread {
                handle: thread,
                outcome: Arc::clone(&outcome),
            });
            Ok(Some(PendingImport {
                id: None,
                attempt,
                cancelled,
                result,
                outcome,
            }))
        })();
        match admission {
            Ok(Some(mut pending)) => {
                pending.id = id;
                self.pending_import = Some(pending);
            }
            result => {
                let result = result.map(|_| ());
                responses.extend(self.import_response(id, result));
            }
        }
        responses
    }

    fn cancel_import(&mut self, code: i32, message: &str) -> Vec<Value> {
        self.pending_import
            .as_mut()
            .and_then(|pending| pending.cancel(code, message))
            .into_iter()
            .collect()
    }

    fn cancel_imported(&mut self, code: i32, message: &str) -> Vec<Value> {
        let mut responses: Vec<_> = self
            .pending_parser
            .iter_mut()
            .filter(|pending| matches!(&pending.authority, ParserAuthority::Imported { .. }))
            .filter_map(|pending| pending.cancel(code, message))
            .collect();
        responses.extend(
            self.waiting_symbols
                .drain(..)
                .map(|pending| response_error(pending.id, code, message)),
        );
        responses
    }

    fn cancel_all(&mut self, code: i32, message: &str) -> Vec<Value> {
        let mut responses = self.cancel_parser(None, code, message);
        responses.extend(self.cancel_import(code, message));
        responses.extend(
            self.waiting_symbols
                .drain(..)
                .map(|pending| response_error(pending.id, code, message)),
        );
        responses
    }

    /// Nonblocking collection after incoming messages commit their mutations.
    /// Results publish only after owned worker joins and authority revalidation.
    pub fn poll(&mut self) -> Vec<Value> {
        if let Ok(mut threads) = PARSER_THREADS.try_lock() {
            reap_parser_threads(&mut threads);
        }
        let mut responses = Vec::new();
        if self
            .pending_import
            .as_ref()
            .is_some_and(|pending| pending.outcome.load(Ordering::Relaxed) != 0)
        {
            let mut pending = self
                .pending_import
                .take()
                .expect("completed import checked");
            let result = if pending.outcome.load(Ordering::Relaxed) != 1 {
                self.cleanup_confirmed = false;
                Err("workspace import worker panicked".to_owned())
            } else {
                pending.result.try_recv().unwrap_or_else(|_| {
                    self.cleanup_confirmed = false;
                    Err("workspace import worker returned no result".into())
                })
            };
            if self.state == State::Running
                && pending.attempt == self.workspace.attempt
                && !pending.cancelled.load(Ordering::Relaxed)
            {
                let result = result.and_then(|snapshot| {
                    self.workspace
                        .commit(
                            snapshot,
                            self.incarnations
                                .iter()
                                .map(|(uri, incarnation)| (uri.as_str(), *incarnation)),
                        )
                        .map_err(str::to_owned)
                });
                let committed = result.is_ok();
                let failure = if self.waiting_symbols.is_empty() {
                    None
                } else {
                    result.as_ref().err().cloned()
                };
                responses.extend(self.import_response(pending.id.take(), result));
                let waiting = std::mem::take(&mut self.waiting_symbols);
                for request in waiting {
                    if committed && request.attempt == pending.attempt {
                        let id = request.id;
                        if let Err((code, message)) = self.start_parser_uri(
                            id.clone(),
                            "textDocument/documentSymbol",
                            &request.uri,
                            None,
                        ) {
                            responses.push(response_error(id, code, message));
                        }
                    } else if request.attempt != pending.attempt {
                        responses.push(response_error(
                            request.id,
                            -32801,
                            "workspace snapshot is stale",
                        ));
                    } else {
                        let mut response = response_error(
                            request.id,
                            -32803,
                            "workspace source inventory unavailable; reload required",
                        );
                        if let Some(reason) = failure.as_deref() {
                            response["error"]["data"] = json!({"reason":reason});
                        }
                        responses.push(response);
                    }
                }
            } else {
                let code = if pending.cancelled.load(Ordering::Relaxed) {
                    -32800
                } else {
                    -32801
                };
                responses.extend(pending.cancel(code, "workspace import cancelled or superseded"));
                responses.extend(self.waiting_symbols.drain(..).map(|request| {
                    response_error(request.id, code, "workspace import cancelled or superseded")
                }));
            }
        }
        let mut index = 0;
        while index < self.pending_parser.len() {
            let outcome = self.pending_parser[index].outcome.load(Ordering::Relaxed);
            if outcome == 0 {
                index += 1;
                continue;
            }
            let mut pending = self.pending_parser.swap_remove(index);
            let result = if outcome != 1 {
                self.cleanup_confirmed = false;
                Err(ParserFailure::Protocol(-32803, "parser worker failed"))
            } else {
                match pending.result.try_recv() {
                    Ok((result, cleanup_confirmed)) => {
                        self.cleanup_confirmed &= cleanup_confirmed;
                        result
                    }
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
                        self.cleanup_confirmed = false;
                        Err(ParserFailure::Protocol(-32803, "parser worker failed"))
                    }
                }
            };
            let Some(id) = pending.id.take() else {
                continue;
            };
            let fresh = match &pending.authority {
                ParserAuthority::Open { incarnation } => {
                    self.incarnations.get(pending.snapshot.document()) == Some(incarnation)
                        && self
                            .documents
                            .get(pending.snapshot.document())
                            .is_some_and(|document| {
                                i64::from(document.version()) == pending.snapshot.revision()
                                    && document.text() == pending.snapshot.source()
                            })
                }
                ParserAuthority::Imported {
                    snapshot, epoch, ..
                } => self.workspace.fresh(snapshot, *epoch),
            };
            let result = if !fresh {
                Err(ParserFailure::Protocol(-32801, "parser snapshot is stale"))
            } else if pending.cancelled.load(Ordering::Relaxed) {
                Err(ParserFailure::Protocol(-32800, "Request cancelled"))
            } else {
                result
            };
            responses.push(match result {
                Ok(result) => {
                    // Move source-sized values without serializing a borrowed Value.
                    let mut response = json!({"jsonrpc":"2.0","id":null,"result":null});
                    response["id"] = id;
                    response["result"] = result;
                    response
                }
                Err(error) => error.response(id),
            });
        }
        responses
    }

    /// Retained jobs, including cancelled jobs awaiting actual thread completion.
    pub fn pending_requests(&self) -> usize {
        self.pending_parser.len()
            + usize::from(self.pending_import.is_some())
            + self.waiting_symbols.len()
    }

    fn cancel_parser(&mut self, uri: Option<&str>, code: i32, message: &str) -> Vec<Value> {
        self.pending_parser
            .iter_mut()
            .filter(|pending| uri.is_none_or(|uri| pending.snapshot.document() == uri))
            .filter_map(|pending| pending.cancel(code, message))
            .collect()
    }

    /// Terminal cleanup: return cancellation responses and whether all owned
    /// threads joined and each request proved child closure/no spawn attempt.
    /// The thread grace is one second excluding OS calls. An unproved parser
    /// cleanup/panic permanently prevents clean success for this Session.
    pub fn close(&mut self) -> (Vec<Value>, bool) {
        let responses = self.cancel_all(-32800, "Request cancelled by transport close");
        let deadline = Instant::now() + CLEANUP_TIMEOUT;
        while self.pending_requests() != 0 && Instant::now() < deadline {
            let _ = self.poll();
            if self.pending_requests() != 0 {
                thread::sleep(Duration::from_millis(2));
            }
        }
        self.state = State::Exited;
        (
            responses,
            self.pending_requests() == 0 && self.cleanup_confirmed,
        )
    }
    fn document_diagnostics(&mut self, uri: &str) -> Option<(i32, Vec<Value>)> {
        let document = self.documents.get(uri)?;
        let diagnostics =
            match self
                .analysis_cache
                .get(uri, document, self.configuration.folding_range_limit)
            {
                Ok(analysis) => analysis.diagnostics(document.text()),
                Err(error) => structure::limit_diagnostic(error),
            };
        Some((document.version(), diagnostics))
    }
    fn notification(&mut self, method: &str, params: Option<&Value>) -> Vec<Value> {
        if method == "exit" {
            if let Err(error) = empty_params(params) {
                self.record(error);
                return Vec::new();
            }
            self.exit_code = Some(if self.state == State::Shutdown { 0 } else { 1 });
            self.state = State::Exited;
            return self.cancel_all(-32800, "Request cancelled by exit");
        }
        if self.state == State::Shutdown {
            return Vec::new();
        }
        if method == "initialized" {
            if self.state != State::Initializing {
                self.record("initialized notification out of order");
                return Vec::new();
            }
            if let Err(error) = params.ok_or("initialized params missing").and_then(object) {
                self.record(error);
                return Vec::new();
            }
            self.state = State::Running;
            let projects = std::mem::take(&mut self.workspace.initial);
            return if projects.is_empty() {
                Vec::new()
            } else {
                self.start_import(None, projects)
            };
        }
        if !matches!(
            method,
            "textDocument/didOpen"
                | "textDocument/didChange"
                | "textDocument/didClose"
                | "textDocument/didSave"
                | "$/cancelRequest"
                | "workspace/didChangeWorkspaceFolders"
        ) {
            return Vec::new();
        }
        if self.state != State::Running {
            self.record("document notification before initialized");
            return Vec::new();
        }
        if method == "workspace/didChangeWorkspaceFolders" {
            return match self.did_change_folders(params) {
                Ok(false) => Vec::new(),
                Ok(true) => {
                    let invalidate =
                        self.workspace.snapshot.is_some() || self.pending_import.is_some();
                    let mut responses =
                        self.cancel_import(-32801, "workspace configuration changed");
                    responses
                        .extend(self.cancel_imported(-32801, "workspace configuration changed"));
                    if invalidate && let Err(error) = self.workspace.invalidate() {
                        self.record(error);
                    }
                    responses
                }
                Err(error) => {
                    self.record(error);
                    Vec::new()
                }
            };
        }
        if method == "$/cancelRequest" {
            let id = params
                .ok_or("cancel params missing")
                .and_then(object)
                .and_then(|map| required(map, "id"))
                .and_then(|id| valid_id(id).then_some(id).ok_or("invalid cancellation id"));
            return match id {
                Ok(id) => {
                    if let Some(response) = self
                        .pending_parser
                        .iter_mut()
                        .find(|pending| pending.id.as_ref() == Some(id))
                        .and_then(|pending| pending.cancel(-32800, "Request cancelled"))
                    {
                        return vec![response];
                    }
                    if self
                        .pending_import
                        .as_ref()
                        .is_some_and(|pending| pending.id.as_ref() == Some(id))
                    {
                        let mut responses = self.cancel_import(-32800, "Request cancelled");
                        responses.extend(self.cancel_imported(-32800, "Request cancelled"));
                        return responses;
                    }
                    self.waiting_symbols
                        .iter()
                        .position(|pending| &pending.id == id)
                        .map(|index| {
                            response_error(
                                self.waiting_symbols.swap_remove(index).id,
                                -32800,
                                "Request cancelled",
                            )
                        })
                        .into_iter()
                        .collect()
                }
                Err(error) => {
                    self.record(error);
                    Vec::new()
                }
            };
        }
        let closing_open = method == "textDocument/didClose"
            && params
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri"))
                .and_then(Value::as_str)
                .is_some_and(|uri| self.documents.contains_key(uri));
        let result = match method {
            "textDocument/didOpen" => self.did_open(params),
            "textDocument/didChange" => self.did_change(params),
            "textDocument/didClose" => self.did_close(params),
            "textDocument/didSave" => self.did_save(params),
            _ => unreachable!(),
        };
        if let Err(error) = result {
            self.record(error);
            return Vec::new();
        }
        let mut responses = if matches!(
            method,
            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose"
        ) {
            let uri = params.expect("validated notification params")["textDocument"]["uri"]
                .as_str()
                .expect("validated URI");
            let mut responses = self.cancel_parser(Some(uri), -32801, "parser snapshot is stale");
            if workspace::is_file_uri(uri) && (method != "textDocument/didClose" || closing_open) {
                responses.extend(self.cancel_imported(-32801, "workspace snapshot is stale"));
            }
            responses
        } else {
            Vec::new()
        };
        if !self.pull_diagnostics
            && matches!(
                method,
                "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose"
            )
        {
            let uri = params.expect("validated notification params")["textDocument"]["uri"]
                .as_str()
                .expect("validated URI");
            let (version, diagnostics) = self
                .document_diagnostics(uri)
                .map_or((None, Vec::new()), |(version, diagnostics)| {
                    (Some(version), diagnostics)
                });
            let mut params = json!({"uri":uri,"diagnostics":diagnostics});
            if self.configuration.diagnostic_version_support
                && let Some(version) = version
            {
                params["version"] = json!(version);
            }
            responses.push(
                json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":params}),
            );
        }
        responses
    }
    fn did_change_folders(&mut self, params: Option<&Value>) -> Result<bool, &'static str> {
        let map = object(params.ok_or("workspace folder change params missing")?)?;
        let event = object(required(map, "event")?)?;
        let added = parse_workspace_folders(required(event, "added")?)?;
        let removed = parse_workspace_folders(required(event, "removed")?)?;
        if added.is_empty() && removed.is_empty() {
            return Ok(false);
        }
        // ponytail: at most 128 folders; use a URI set if this cap grows.
        let retained = self
            .configuration
            .workspace_folders
            .iter()
            .filter(|folder| !removed.iter().any(|removed| removed.uri == folder.uri))
            .count();
        if retained + added.len() > MAX_WORKSPACE_FOLDERS {
            return Err("workspace folder count limit exceeded");
        }
        for (index, folder) in added.iter().enumerate() {
            if added[..index].iter().any(|prior| prior.uri == folder.uri)
                || self.configuration.workspace_folders.iter().any(|prior| {
                    prior.uri == folder.uri
                        && !removed.iter().any(|removed| removed.uri == prior.uri)
                })
            {
                return Err("added workspace folder URI is already configured");
            }
        }
        self.configuration
            .workspace_folders
            .retain(|folder| !removed.iter().any(|removed| removed.uri == folder.uri));
        self.configuration.workspace_folders.extend(added);
        Ok(true)
    }

    fn did_open(&mut self, params: Option<&Value>) -> Result<(), String> {
        let map = object(params.ok_or("open params missing")?)?;
        let doc = object(required(map, "textDocument")?)?;
        let uri = uri_value(required(doc, "uri")?)?;
        if required(doc, "languageId")?.as_str() != Some("kotlin") {
            return Err("only Kotlin documents are supported".into());
        }
        let version = serde_json::from_value::<i32>(required(doc, "version")?.clone())
            .map_err(|_| "version must be i32")?;
        let text = required(doc, "text")?
            .as_str()
            .ok_or("text must be a string")?;
        if self.documents.contains_key(uri) {
            return Err("document is already open".into());
        }
        self.workspace.check_open(uri)?;
        if self.documents.len() >= MAX_OPEN_DOCUMENTS {
            return Err("open document count limit exceeded".into());
        }
        if text.len() > ktlsp_text::MAX_DOCUMENT_BYTES {
            return Err("document size limit exceeded".into());
        }
        let bytes = self.open_bytes + text.len();
        if bytes > MAX_OPEN_BYTES {
            return Err("aggregate open document size limit exceeded".into());
        }
        let document = Document::new(version, text.into()).map_err(|e| e.to_string())?;
        let incarnation = self
            .next_incarnation
            .checked_add(1)
            .ok_or("document incarnation limit exceeded")?;
        self.next_incarnation = incarnation;
        self.incarnations.insert(uri.into(), incarnation);
        self.documents.insert(uri.into(), document);
        self.workspace.opened(uri, incarnation);
        self.analysis_cache.invalidate(uri);
        self.open_bytes = bytes;
        Ok(())
    }
    fn did_change(&mut self, params: Option<&Value>) -> Result<(), String> {
        let map = object(params.ok_or("change params missing")?)?;
        let doc = object(required(map, "textDocument")?)?;
        let uri = uri_value(required(doc, "uri")?)?;
        let version = serde_json::from_value::<i32>(required(doc, "version")?.clone())
            .map_err(|_| "version must be i32")?;
        let raw_changes = required(map, "contentChanges")?
            .as_array()
            .ok_or("contentChanges must be an array")?;
        if raw_changes.len() > MAX_CHANGES {
            return Err("change count limit exceeded".into());
        }
        for change in raw_changes {
            let change = object(change)?;
            if change.get("range").is_some_and(Value::is_null)
                || change.get("rangeLength").is_some_and(Value::is_null)
            {
                return Err("optional change fields cannot be null".into());
            }
        }
        let changes: Vec<TextChange> = raw_changes
            .iter()
            .map(|change| {
                serde_json::from_value(change.clone()).map_err(|_| "invalid text change schema")
            })
            .collect::<Result<_, _>>()?;
        let existing = self
            .documents
            .get(uri)
            .ok_or("change refers to unopened document")?;
        let mut candidate = existing.clone();
        candidate
            .apply_changes(version, &changes)
            .map_err(|e| e.to_string())?;
        let bytes = self.open_bytes - existing.text().len() + candidate.text().len();
        if bytes > MAX_OPEN_BYTES {
            return Err("aggregate open document size limit exceeded".into());
        }
        self.documents.insert(uri.into(), candidate);
        self.workspace.changed(uri);
        self.analysis_cache.invalidate(uri);
        self.open_bytes = bytes;
        Ok(())
    }
    fn did_close(&mut self, params: Option<&Value>) -> Result<(), String> {
        let map = object(params.ok_or("close params missing")?)?;
        let doc = object(required(map, "textDocument")?)?;
        let uri = uri_value(required(doc, "uri")?)?;
        self.analysis_cache.invalidate(uri);
        if let Some(document) = self.documents.remove(uri) {
            self.open_bytes -= document.text().len();
            if let Some(incarnation) = self.incarnations.remove(uri) {
                self.workspace.closed(uri, incarnation);
            }
        }
        Ok(())
    }
    fn did_save(&self, params: Option<&Value>) -> Result<(), String> {
        let map = object(params.ok_or("save params missing")?)?;
        let doc = object(required(map, "textDocument")?)?;
        let uri = uri_value(required(doc, "uri")?)?;
        if !self.documents.contains_key(uri) {
            return Err("save refers to unopened document".into());
        }
        if map.get("text").is_some_and(|text| {
            !text.is_string()
                || text
                    .as_str()
                    .is_some_and(|s| s.len() > ktlsp_text::MAX_DOCUMENT_BYTES)
        }) {
            return Err("invalid save text".into());
        }
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        for pending in &self.pending_parser {
            pending.cancelled.store(true, Ordering::Relaxed);
        }
        if let Some(pending) = &self.pending_import {
            pending.cancelled.store(true, Ordering::Relaxed);
        }
        // Global bounded ownership retains unfinished handles. Admission/poll
        // reap them only after is_finished(); close() reports cleanup outcome.
    }
}

fn parse_code_action_capability(capabilities: &Value) -> Result<bool, &'static str> {
    let Some(text) = capabilities.get("textDocument") else {
        return Ok(false);
    };
    let Some(action) = object(text)?.get("codeAction") else {
        return Ok(false);
    };
    let action = object(action)?;
    for key in [
        "dynamicRegistration",
        "isPreferredSupport",
        "disabledSupport",
        "dataSupport",
        "honorsChangeAnnotations",
    ] {
        if action.get(key).is_some_and(|value| !value.is_boolean()) {
            return Err("code action capability flags must be boolean");
        }
    }
    if let Some(resolve) = action.get("resolveSupport") {
        string_array(required(object(resolve)?, "properties")?)?;
    }
    let Some(literal) = action.get("codeActionLiteralSupport") else {
        return Ok(false);
    };
    let kind = object(required(object(literal)?, "codeActionKind")?)?;
    string_array(required(kind, "valueSet")?)?;
    // LSP clients must gracefully handle kinds outside this advisory value set.
    Ok(true)
}

fn string_array(value: &Value) -> Result<&[Value], &'static str> {
    value
        .as_array()
        .filter(|values| values.iter().all(Value::is_string))
        .map(Vec::as_slice)
        .ok_or("expected an array of strings")
}

fn range_value(value: &Value) -> Result<ktlsp_text::Range, &'static str> {
    fn position(value: &Value) -> Result<ktlsp_text::Position, &'static str> {
        let map = object(value)?;
        let coordinate = |key| {
            required(map, key)?
                .as_u64()
                .filter(|n| *n <= i32::MAX as u64)
                .map(|n| n as u32)
                .ok_or("range coordinates must be LSP uintegers")
        };
        Ok(ktlsp_text::Position {
            line: coordinate("line")?,
            character: coordinate("character")?,
        })
    }
    let map = object(value)?;
    let range = ktlsp_text::Range {
        start: position(required(map, "start")?)?,
        end: position(required(map, "end")?)?,
    };
    if range.start > range.end {
        return Err("reversed range");
    }
    Ok(range)
}

fn code_action_context(value: &Value) -> Result<bool, &'static str> {
    let map = object(value)?;
    let diagnostics = required(map, "diagnostics")?
        .as_array()
        .ok_or("code action diagnostics must be an array")?;
    for diagnostic in diagnostics {
        let diagnostic = object(diagnostic)?;
        range_value(required(diagnostic, "range")?)?;
        required(diagnostic, "message")?
            .as_str()
            .ok_or("diagnostic message must be a string")?;
        if diagnostic
            .get("source")
            .is_some_and(|value| !value.is_string())
        {
            return Err("diagnostic source must be a string");
        }
        if diagnostic
            .get("severity")
            .is_some_and(|value| !matches!(value.as_u64(), Some(1..=4)))
        {
            return Err("invalid diagnostic severity");
        }
        if diagnostic.get("code").is_some_and(|value| {
            !value.is_string() && value.as_i64().and_then(|n| i32::try_from(n).ok()).is_none()
        }) {
            return Err("diagnostic code must be an integer or string");
        }
        if let Some(description) = diagnostic.get("codeDescription") {
            uri_value(required(object(description)?, "href")?)?;
        }
        if let Some(tags) = diagnostic.get("tags")
            && !tags.as_array().is_some_and(|values| {
                values
                    .iter()
                    .all(|value| matches!(value.as_u64(), Some(1 | 2)))
            })
        {
            return Err("invalid diagnostic tags");
        }
        if let Some(related) = diagnostic.get("relatedInformation") {
            for info in related
                .as_array()
                .ok_or("related information must be an array")?
            {
                let info = object(info)?;
                required(info, "message")?
                    .as_str()
                    .ok_or("related message must be a string")?;
                let location = object(required(info, "location")?)?;
                uri_value(required(location, "uri")?)?;
                range_value(required(location, "range")?)?;
            }
        }
    }
    if map
        .get("triggerKind")
        .is_some_and(|value| !matches!(value.as_u64(), Some(1 | 2)))
    {
        return Err("invalid code action trigger kind");
    }
    map.get("only")
        .map(|value| {
            string_array(value).map(|kinds| {
                kinds
                    .iter()
                    .any(|kind| matches!(kind.as_str(), Some("" | "quickfix")))
            })
        })
        .unwrap_or(Ok(true))
}

fn parse_workspace_folders(value: &Value) -> Result<Vec<WorkspaceFolder>, &'static str> {
    let folders = value
        .as_array()
        .ok_or("workspaceFolders must be an array")?;
    if folders.len() > MAX_WORKSPACE_FOLDERS {
        return Err("workspace folder count limit exceeded");
    }
    let mut parsed = Vec::with_capacity(folders.len());
    for folder in folders {
        let folder = object(folder)?;
        let uri = uri_value(required(folder, "uri")?)?;
        let name = required(folder, "name")?
            .as_str()
            .ok_or("workspace folder name must be a string")?;
        if name.len() > MAX_URI_BYTES {
            return Err("workspace folder name is too long");
        }
        parsed.push(WorkspaceFolder {
            uri: uri.into(),
            name: name.into(),
        });
    }
    Ok(parsed)
}

fn parse_initialize(params: Option<&Value>) -> Result<(Configuration, bool), &'static str> {
    let map = object(params.ok_or("initialize params missing")?)?;
    let capabilities = object(required(map, "capabilities")?)?;
    if let Some(general) = capabilities.get("general") {
        let general = object(general)?;
        if let Some(encodings) = general.get("positionEncodings") {
            let encodings = encodings
                .as_array()
                .ok_or("positionEncodings must be an array")?;
            // LSP 3.17 makes UTF-16 mandatory even when omitted from this list.
            if !encodings.iter().all(Value::is_string) {
                return Err("positionEncodings entries must be strings");
            }
        }
    }
    let mut configuration = Configuration::default();
    let mut pull_diagnostics = false;
    if let Some(text_document) = capabilities.get("textDocument") {
        let text_document = object(text_document)?;
        if let Some(diagnostic) = text_document.get("diagnostic") {
            let diagnostic = object(diagnostic)?;
            for key in ["dynamicRegistration", "relatedDocumentSupport"] {
                if diagnostic.get(key).is_some_and(|value| !value.is_boolean()) {
                    return Err("diagnostic capability flags must be boolean");
                }
            }
            pull_diagnostics = true;
        }
        if let Some(folding) = text_document.get("foldingRange") {
            let folding = object(folding)?;
            if let Some(value) = folding.get("lineFoldingOnly") {
                configuration.line_folding_only =
                    value.as_bool().ok_or("lineFoldingOnly must be boolean")?;
            }
            if let Some(value) = folding.get("rangeLimit") {
                let limit = value
                    .as_u64()
                    .filter(|n| *n <= i32::MAX as u64)
                    .ok_or("rangeLimit must be an LSP uinteger")?;
                configuration.folding_range_limit = limit.min(structure::MAX_FOLDS as u64) as usize;
            }
        }
        if let Some(diagnostics) = text_document.get("publishDiagnostics") {
            let diagnostics = object(diagnostics)?;
            if let Some(value) = diagnostics.get("versionSupport") {
                configuration.diagnostic_version_support =
                    value.as_bool().ok_or("versionSupport must be boolean")?;
            }
        }
    }
    if let Some(value) = map.get("processId").filter(|v| !v.is_null()) {
        let Some(number) = value.as_number().filter(|n| n.is_i64() || n.is_u64()) else {
            return Err("processId must be an integer or null");
        };
        configuration.process_id = Some(number.clone());
    }
    if let Some(value) = map.get("rootUri").filter(|v| !v.is_null()) {
        configuration.root_uri = Some(uri_value(value)?.into());
    }
    if let Some(value) = map.get("workspaceFolders").filter(|v| !v.is_null()) {
        configuration.workspace_folders = parse_workspace_folders(value)?;
    }
    Ok((configuration, pull_diagnostics))
}

/// Run framed communication, including deferred parser completions. Input must
/// yield WouldBlock regularly when idle (the CLI uses `StdinInput`); a blocking
/// reader can delay completions. Output writes/flushes are synchronous and can
/// block on client backpressure. No input thread or input queue is created.
/// Framing/write errors are fatal; complete invalid JSON is recoverable.
/// EOF succeeds only after shutdown and confirmed owned-job cleanup.
pub fn run<R: BufRead, W: Write, E: Write>(reader: &mut R, writer: &mut W, errors: &mut E) -> i32 {
    // Resolve only beside this executable, never PATH, cwd, or an environment
    // override. Distribution must install the worker from the same build here.
    let mut session = std::env::current_exe()
        .ok()
        .and_then(|executable| {
            executable.parent().map(|parent| {
                parent.join(format!(
                    "ktlsp-parser-worker{}",
                    std::env::consts::EXE_SUFFIX
                ))
            })
        })
        .and_then(|worker| Session::with_parser(worker).ok())
        .unwrap_or_default();
    let mut frames = FrameReader::default();
    let mut output_failed = false;
    let status = 'exchange: loop {
        let mut responses = match frames.poll(reader) {
            Ok(Poll::Ready(Some(payload))) => match serde_json::from_slice::<Value>(&payload) {
                Ok(message) => session.handle(message),
                Err(_) => vec![response_error(Value::Null, -32700, "Parse error")],
            },
            Ok(Poll::Pending) => {
                thread::sleep(Duration::from_millis(1));
                Vec::new()
            }
            Ok(Poll::Ready(None)) => break i32::from(session.state != State::Shutdown),
            Err(error) => {
                let _ = writeln!(errors, "ktlsp: {error}");
                break 1;
            }
        };
        responses.extend(session.poll());
        for error in session.drain_errors() {
            let _ = writeln!(errors, "ktlsp: {error}");
        }
        for response in responses {
            if let Err(error) = send_response(writer, &response) {
                let _ = writeln!(errors, "ktlsp: {error}");
                output_failed = true;
                break 'exchange 1;
            }
        }
        if let Some(code) = session.exit_code() {
            break code;
        }
    };
    let (cancelled, clean) = session.close();
    if !clean {
        let _ = writeln!(errors, "ktlsp: document symbol cleanup incomplete");
    }
    if !output_failed {
        for response in cancelled {
            if let Err(error) = send_response(writer, &response) {
                let _ = writeln!(errors, "ktlsp: {error}");
                output_failed = true;
                break;
            }
        }
    }
    if clean && !output_failed { status } else { 1 }
}

fn send_response(writer: &mut impl Write, response: &Value) -> Result<(), String> {
    let payload = serde_json::to_vec(response).map_err(|error| error.to_string())?;
    write_frame(writer, &payload).map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;

    #[test]
    fn unproved_child_cleanup_cannot_become_clean_after_threads_finish() {
        let mut session = Session::new();
        session.state = State::Shutdown;
        // Pure control-state regression: no parser/provider or worker is run.
        session.cleanup_confirmed = false;
        assert_eq!(session.pending_requests(), 0);
        assert!(!session.close().1);
        assert!(
            !session.close().1,
            "later calls must not invent closure proof"
        );
    }

    #[test]
    fn idle_would_block_input_has_bounded_poll_rate() {
        struct IdleInput(usize);
        impl std::io::Read for IdleInput {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if buffer.is_empty() {
                    return Ok(0);
                }
                self.fill_buf().map(|bytes| bytes.len())
            }
        }
        impl BufRead for IdleInput {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                if self.0 == 0 {
                    return Ok(&[]);
                }
                self.0 -= 1;
                Err(std::io::ErrorKind::WouldBlock.into())
            }
            fn consume(&mut self, count: usize) {
                assert_eq!(count, 0);
            }
        }
        let started = Instant::now();
        assert_eq!(run(&mut IdleInput(16), &mut Vec::new(), &mut Vec::new()), 1);
        // At most 4,000 idle probes/second, not an exact scheduler interval.
        assert!(started.elapsed() >= Duration::from_millis(4));
    }
}
