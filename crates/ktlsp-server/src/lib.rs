//! Native LSP lifecycle, bounded document synchronization and lexical/structural
//! providers. Semantic analysis and project import remain unsupported.

mod structure;
mod uri;
use ktlsp_protocol::{read_frame, write_frame};
use ktlsp_text::{Document, MAX_CHANGES, TextChange};
use serde_json::{Map, Number, Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, Write},
};

pub const MAX_OPEN_DOCUMENTS: usize = 128;
pub const MAX_OPEN_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_URI_BYTES: usize = 8 * 1024;
pub const MAX_ERROR_RECORDS: usize = 128;
pub const MAX_WORKSPACE_FOLDERS: usize = 128;

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
    open_bytes: usize,
    errors: Vec<String>,
    configuration: Configuration,
    exit_code: Option<i32>,
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
            open_bytes: 0,
            errors: Vec::new(),
            configuration: Configuration::default(),
            exit_code: None,
        }
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
        let result = self.request(method, params);
        vec![match result {
            Ok(result) => json!({"jsonrpc":"2.0", "id":id, "result":result}),
            Err((code, message)) => response_error(id.clone(), code, message),
        }]
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
            let configuration = parse_initialize(params).map_err(|e| (-32602, e))?;
            self.configuration = configuration;
            self.state = State::Initializing;
            return Ok(
                json!({"capabilities":{"positionEncoding":"utf-16", "foldingRangeProvider":true, "textDocumentSync":{"openClose":true,"change":2,"save":false}}, "serverInfo":{"name":"ktlsp","version":env!("CARGO_PKG_VERSION")}}),
            );
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
                let analysis =
                    structure::analyze(document.text(), self.configuration.folding_range_limit)
                        .map_err(|e| (-32803, e))?;
                let (folds, _) =
                    analysis.render(document.text(), self.configuration.line_folding_only);
                Ok(json!(folds))
            }
            _ => Err((-32601, "Method not found")),
        }
    }
    fn notification(&mut self, method: &str, params: Option<&Value>) -> Vec<Value> {
        if method == "exit" {
            if let Err(error) = empty_params(params) {
                self.record(error);
                return Vec::new();
            }
            self.exit_code = Some(if self.state == State::Shutdown { 0 } else { 1 });
            self.state = State::Exited;
            return Vec::new();
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
        let result = match method {
            "textDocument/didOpen" => self.did_open(params),
            "textDocument/didChange" => self.did_change(params),
            "textDocument/didClose" => self.did_close(params),
            "textDocument/didSave" => self.did_save(params),
            "$/cancelRequest" => params
                .ok_or("cancel params missing")
                .and_then(object)
                .and_then(|m| {
                    if valid_id(required(m, "id")?) {
                        Ok(())
                    } else {
                        Err("invalid cancellation id")
                    }
                })
                .map_err(str::to_owned),
            _ => unreachable!(),
        };
        if let Err(error) = result {
            self.record(error);
            return Vec::new();
        }
        if matches!(
            method,
            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose"
        ) {
            let uri = params.expect("validated notification params")["textDocument"]["uri"]
                .as_str()
                .expect("validated URI");
            let document = self.documents.get(uri);
            let diagnostics =
                document.map_or_else(Vec::new, |doc| match structure::analyze(doc.text(), 0) {
                    Ok(analysis) => analysis.render(doc.text(), false).1,
                    Err(error) => structure::limit_diagnostic(error),
                });
            let mut params = json!({"uri":uri,"diagnostics":diagnostics});
            if self.configuration.diagnostic_version_support
                && let Some(doc) = document
            {
                params["version"] = json!(doc.version());
            }
            return vec![
                json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":params}),
            ];
        }
        Vec::new()
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
        self.documents.insert(uri.into(), document);
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
        self.open_bytes = bytes;
        Ok(())
    }
    fn did_close(&mut self, params: Option<&Value>) -> Result<(), String> {
        let map = object(params.ok_or("close params missing")?)?;
        let doc = object(required(map, "textDocument")?)?;
        let uri = uri_value(required(doc, "uri")?)?;
        if let Some(document) = self.documents.remove(uri) {
            self.open_bytes -= document.text().len();
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

fn parse_initialize(params: Option<&Value>) -> Result<Configuration, &'static str> {
    let map = object(params.ok_or("initialize params missing")?)?;
    let capabilities = object(required(map, "capabilities")?)?;
    if let Some(general) = capabilities.get("general") {
        let general = object(general)?;
        if let Some(encodings) = general.get("positionEncodings") {
            let encodings = encodings
                .as_array()
                .ok_or("positionEncodings must be an array")?;
            if !encodings.iter().all(Value::is_string)
                || !encodings.iter().any(|e| e.as_str() == Some("utf-16"))
            {
                return Err("client must support utf-16 positions");
            }
        }
    }
    let mut configuration = Configuration::default();
    if let Some(text_document) = capabilities.get("textDocument") {
        let text_document = object(text_document)?;
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
    Ok(configuration)
}

/// Run framed stdio-style communication. Framing/write errors are fatal;
/// complete invalid JSON payloads produce recoverable parse-error responses.
/// Logs go exclusively to `errors`. EOF succeeds only after shutdown.
pub fn run<R: BufRead, W: Write, E: Write>(reader: &mut R, writer: &mut W, errors: &mut E) -> i32 {
    let mut session = Session::new();
    loop {
        let payload = match read_frame(reader) {
            Ok(Some(payload)) => payload,
            Ok(None) => {
                return if session.state == State::Shutdown {
                    0
                } else {
                    1
                };
            }
            Err(error) => {
                let _ = writeln!(errors, "ktlsp: {error}");
                return 1;
            }
        };
        let responses = match serde_json::from_slice::<Value>(&payload) {
            Ok(message) => session.handle(message),
            Err(_) => vec![response_error(Value::Null, -32700, "Parse error")],
        };
        for error in session.drain_errors() {
            let _ = writeln!(errors, "ktlsp: {error}");
        }
        for response in responses {
            let result = serde_json::to_vec(&response)
                .map_err(|e| e.to_string())
                .and_then(|payload| write_frame(writer, &payload).map_err(|e| e.to_string()))
                .and_then(|()| writer.flush().map_err(|e| e.to_string()));
            if let Err(error) = result {
                let _ = writeln!(errors, "ktlsp: {error}");
                return 1;
            }
        }
        if let Some(code) = session.exit_code() {
            return code;
        }
    }
}
