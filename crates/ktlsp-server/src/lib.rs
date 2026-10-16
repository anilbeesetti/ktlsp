//! Native LSP lifecycle, bounded synchronization, lexical/structural providers
//! and finite authenticated local navigation. Project import is unsupported.

mod cache;
mod document_symbols;
mod input;
mod semantic_navigation;
pub use input::StdinInput;
mod structure;
mod uri;
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

type ParserResult = Result<Value, (i32, &'static str)>;
#[derive(Debug)]
struct PendingParser {
    id: Option<Value>,
    snapshot: SourceSnapshot,
    incarnation: u64,
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
    exit_code: Option<i32>,
    incarnations: HashMap<String, u64>,
    next_incarnation: u64,
    pending_parser: Vec<PendingParser>,
    cleanup_confirmed: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn response_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "error":{"code":code,"message":message}})
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
            exit_code: None,
            incarnations: HashMap::new(),
            next_incarnation: 0,
            pending_parser: Vec::new(),
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
        {
            return vec![response_error(
                Value::Null,
                -32600,
                "request id is already active",
            )];
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
        ) && self.state == State::Running
        {
            return match self.start_parser(id.clone(), method, params) {
                Ok(()) => Vec::new(),
                Err((code, message)) => vec![response_error(id.clone(), code, message)],
            };
        }
        let result = self.request(method, params);
        let mut responses = if method == "shutdown" && result.is_ok() {
            self.cancel_parser(None, -32800, "Request cancelled by shutdown")
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
                params.and_then(|p| p.get("capabilities")).expect("validated capabilities"),
            ).map_err(|e| (-32602, e))?;
            self.rename_client = rename_client;
            self.symbol_client = symbol_client;
            self.hover_format = semantic_navigation::HoverFormat::negotiate(
                params
                    .and_then(|p| p.get("capabilities"))
                    .expect("validated capabilities"),
            );
            self.configuration = configuration;
            self.pull_diagnostics = pull_diagnostics;
            self.state = State::Initializing;
            let mut result = json!({"capabilities":{"positionEncoding":"utf-16", "foldingRangeProvider":true, "textDocumentSync":{"openClose":true,"change":2,"save":false}}, "serverInfo":{"name":"ktlsp","version":env!("CARGO_PKG_VERSION")}});
            if self.parser_provider.is_some() {
                result["capabilities"]["documentSymbolProvider"] = json!(true);
                result["capabilities"]["definitionProvider"] = json!(true);
                result["capabilities"]["referencesProvider"] = json!(true);
                result["capabilities"]["documentHighlightProvider"] = json!(true);
                if self.hover_format.is_some() {
                    result["capabilities"]["hoverProvider"] = json!(true);
                }
                if self.rename_client.document_changes {
                    result["capabilities"]["renameProvider"] = if self.rename_client.prepare {
                        json!({"prepareProvider":true})
                    } else { json!(true) };
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
        let provider = self
            .parser_provider
            .as_ref()
            .ok_or((-32601, "Method not found"))?;
        if method == "textDocument/hover" && self.hover_format.is_none() {
            return Err((-32601, "Method not found"));
        }
        if matches!(method, "textDocument/rename" | "textDocument/prepareRename")
            && (!self.rename_client.document_changes
                || (method == "textDocument/prepareRename" && !self.rename_client.prepare)) {
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
        let document = self
            .documents
            .get(uri)
            .ok_or((-32602, "parser request refers to unopened document"))?;
        if document.text().len() > ktlsp_parser::MAX_SOURCE_BYTES {
            return Err((-32803, "request exceeds the parser's 64 KiB source limit"));
        }
        let query = if method == "textDocument/documentSymbol" {
            None
        } else {
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
                "textDocument/prepareRename" => semantic_navigation::Query::PrepareRename { offset },
                "textDocument/rename" => {
                    let name = required(map, "newName").and_then(|v| v.as_str()
                        .ok_or("newName must be a string")).map_err(|e| (-32602, e))?;
                    if !ktlsp_parser::local_value_binding::valid_local_rename_name(name) {
                        return Err((-32602, "unsupported ASCII rename spelling"));
                    }
                    semantic_navigation::Query::Rename { offset, new_name: name.to_owned() }
                }
                "textDocument/hover" => semantic_navigation::Query::Hover {
                    offset,
                    format: self.hover_format.expect("hover format checked"),
                },
                _ => return Err((-32601, "Method not found")),
            })
        };
        if self.pending_parser.len() >= MAX_PENDING_PARSER_REQUESTS {
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
        let snapshot = SourceSnapshot::new(
            uri,
            i64::from(document.version()),
            FileKind::Source,
            document.text(),
        )
        .map_err(|_| (-32803, "snapshot exceeds parser limits"))?;
        let incarnation = self.incarnations[uri];
        let worker_snapshot = snapshot.clone();
        let provider = Arc::clone(provider);
        let client = self.symbol_client.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let (sender, result) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("ktlsp-parser".into())
            .spawn(move || {
                let (parsed, cleanup_confirmed) =
                    provider.parse(&worker_snapshot, &worker_cancelled);
                let result = (|| {
                    let parsed = parsed.map_err(|error| {
                        if error == ParserError::Cancelled {
                            (-32800, "Request cancelled")
                        } else {
                            (-32803, "parser failed or exceeded its resource limits")
                        }
                    })?;
                    if worker_cancelled.load(Ordering::Relaxed) {
                        return Err((-32800, "Request cancelled"));
                    }
                    if let Some(query) = query {
                        semantic_navigation::render(&parsed, &worker_snapshot, query)
                            .map_err(|error| (-32803, error))
                    } else {
                        let tree = parsed
                            .tree_for(&worker_snapshot)
                            .map_err(|_| (-32801, "parser snapshot is stale"))?;
                        document_symbols::render(tree, &client, worker_snapshot.document())
                            .map_err(|error| (-32803, error))
                    }
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
            incarnation,
            cancelled,
            result,
            outcome,
        });
        Ok(())
    }

    /// Nonblocking completion collection. Call after handling incoming messages;
    /// committed document mutations invalidate work before it can be published.
    pub fn poll(&mut self) -> Vec<Value> {
        if let Ok(mut threads) = PARSER_THREADS.try_lock() {
            reap_parser_threads(&mut threads);
        }
        let mut responses = Vec::new();
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
                Err((-32803, "parser worker failed"))
            } else {
                match pending.result.try_recv() {
                    Ok((result, cleanup_confirmed)) => {
                        self.cleanup_confirmed &= cleanup_confirmed;
                        result
                    }
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
                        self.cleanup_confirmed = false;
                        Err((-32803, "parser worker failed"))
                    }
                }
            };
            let Some(id) = pending.id.take() else {
                continue;
            };
            let current = self.documents.get(pending.snapshot.document());
            let fresh = self.incarnations.get(pending.snapshot.document())
                == Some(&pending.incarnation)
                && current.is_some_and(|document| {
                    i64::from(document.version()) == pending.snapshot.revision()
                        && document.text() == pending.snapshot.source()
                });
            let result = if !fresh {
                Err((-32801, "parser snapshot is stale"))
            } else if pending.cancelled.load(Ordering::Relaxed) {
                Err((-32800, "Request cancelled"))
            } else {
                result
            };
            responses.push(match result {
                Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
                Err((code, message)) => response_error(id, code, message),
            });
        }
        responses
    }

    /// Retained jobs, including cancelled jobs awaiting actual thread completion.
    pub fn pending_requests(&self) -> usize {
        self.pending_parser.len()
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
        let responses = self.cancel_parser(None, -32800, "Request cancelled by transport close");
        let deadline = Instant::now() + CLEANUP_TIMEOUT;
        while !self.pending_parser.is_empty() && Instant::now() < deadline {
            let _ = self.poll();
            if !self.pending_parser.is_empty() {
                thread::sleep(Duration::from_millis(2));
            }
        }
        self.state = State::Exited;
        (
            responses,
            self.pending_parser.is_empty() && self.cleanup_confirmed,
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
            return self.cancel_parser(None, -32800, "Request cancelled by exit");
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
            return Vec::new();
        }
        if !matches!(
            method,
            "textDocument/didOpen"
                | "textDocument/didChange"
                | "textDocument/didClose"
                | "textDocument/didSave"
                | "$/cancelRequest"
        ) {
            return Vec::new();
        }
        if self.state != State::Running {
            self.record("document notification before initialized");
            return Vec::new();
        }
        if method == "$/cancelRequest" {
            let id = params
                .ok_or("cancel params missing")
                .and_then(object)
                .and_then(|map| required(map, "id"))
                .and_then(|id| valid_id(id).then_some(id).ok_or("invalid cancellation id"));
            return match id {
                Ok(id) => self
                    .pending_parser
                    .iter_mut()
                    .find(|pending| pending.id.as_ref() == Some(id))
                    .and_then(|pending| pending.cancel(-32800, "Request cancelled"))
                    .into_iter()
                    .collect(),
                Err(error) => {
                    self.record(error);
                    Vec::new()
                }
            };
        }
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
            self.cancel_parser(Some(uri), -32801, "parser snapshot is stale")
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
            self.incarnations.remove(uri);
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
        // Global bounded ownership retains unfinished handles. Admission/poll
        // reap them only after is_finished(); close() reports cleanup outcome.
    }
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
        let folders = value
            .as_array()
            .ok_or("workspaceFolders must be an array or null")?;
        if folders.len() > MAX_WORKSPACE_FOLDERS {
            return Err("workspace folder count limit exceeded");
        }
        for folder in folders {
            let folder = object(folder)?;
            let uri = uri_value(required(folder, "uri")?)?;
            let name = required(folder, "name")?
                .as_str()
                .ok_or("workspace folder name must be a string")?;
            if name.len() > MAX_URI_BYTES {
                return Err("workspace folder name is too long");
            }
            configuration.workspace_folders.push(WorkspaceFolder {
                uri: uri.into(),
                name: name.into(),
            });
        }
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
