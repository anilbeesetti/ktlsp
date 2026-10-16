//! Syntactic names only: no resolved identities, types or reference information.
use ktlsp_parser::{Element, Parsed, Parser, ParserError, SourceSnapshot};
use ktlsp_text::Range;
use serde_json::{Value, json};
use std::{path::Path, sync::atomic::AtomicBool};

pub(crate) const MAX_SYMBOLS: usize = 1_024;
pub(crate) const MAX_SYMBOL_DEPTH: usize = 32;
const MAX_RENDERED_BYTES: usize = 1024 * 1024;
const UNSUPPORTED: &str = "document symbols require a complete supported declaration tree";

#[derive(Debug, Clone)]
pub(crate) struct Client {
    hierarchical: bool,
    kinds: [bool; 27],
}
impl Default for Client {
    fn default() -> Self {
        let mut kinds = [false; 27];
        kinds[1..=18].fill(true); // LSP's original SymbolKind set when valueSet is absent.
        Self {
            hierarchical: false,
            kinds,
        }
    }
}
impl Client {
    pub(crate) fn parse(capabilities: &Value) -> Result<Self, &'static str> {
        let mut client = Self::default();
        let Some(document) = capabilities.get("textDocument") else {
            return Ok(client);
        };
        let Some(symbols) = document.get("documentSymbol") else {
            return Ok(client);
        };
        let symbols = symbols
            .as_object()
            .ok_or("documentSymbol must be an object")?;
        if let Some(value) = symbols.get("hierarchicalDocumentSymbolSupport") {
            client.hierarchical = value
                .as_bool()
                .ok_or("hierarchicalDocumentSymbolSupport must be boolean")?;
        }
        if let Some(value) = symbols.get("symbolKind") {
            let value = value.as_object().ok_or("symbolKind must be an object")?;
            if let Some(value) = value.get("valueSet") {
                let values = value
                    .as_array()
                    .ok_or("symbolKind valueSet must be an array")?;
                if values.len() > 128 {
                    return Err("symbolKind valueSet exceeds limit");
                }
                client.kinds.fill(false);
                for value in values {
                    let kind = value
                        .as_u64()
                        .filter(|v| (1..=i32::MAX as u64).contains(v))
                        .ok_or("symbolKind must be a positive LSP integer")?;
                    if kind <= 26 {
                        client.kinds[kind as usize] = true;
                    }
                }
            }
        }
        Ok(client)
    }
    fn kind(&self, kind: usize, fallback: usize) -> Result<usize, &'static str> {
        if self.kinds[kind] {
            Ok(kind)
        } else if self.kinds[fallback] {
            Ok(fallback)
        } else {
            Err("client has no supported kind for a document symbol")
        }
    }
}

pub(crate) struct Provider(Parser);
impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Provider(trusted parser worker)")
    }
}
impl Provider {
    pub(crate) fn new(worker: &Path) -> Result<Self, ParserError> {
        if !worker.is_absolute() {
            return Err(ParserError::Io);
        }
        let metadata = worker.metadata().map_err(|_| ParserError::Io)?;
        if !metadata.is_file() {
            return Err(ParserError::Io);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(ParserError::Io);
            }
        }
        Parser::new(worker).map(Self)
    }
    pub(crate) fn parse(
        &self,
        snapshot: &SourceSnapshot,
        cancelled: &AtomicBool,
    ) -> (Result<ktlsp_parser::SnapshotResult, ParserError>, bool) {
        self.0.parse_snapshot_with_cleanup(snapshot, cancelled)
    }
}

struct Symbol<'a> {
    node: &'a Element,
    name: &'a Element,
    spelling: &'a str,
    kind: usize,
    owner: Option<usize>,
    depth: usize,
}

// Only validated Parser output enters this projection. Child jumps visit direct
// declaration structure, never text matches or arbitrary descendant identifiers.
fn declarations<'a>(tree: &'a Parsed, client: &Client) -> Result<Vec<Symbol<'a>>, &'static str> {
    if !tree.diagnostics().is_empty() {
        return Err(UNSUPPORTED);
    }
    let elements = tree.elements();
    let mut symbols: Vec<Symbol<'a>> = Vec::new();
    let mut owners: Vec<(usize, usize)> = Vec::new();
    for (index, node) in elements.iter().enumerate() {
        while owners.last().is_some_and(|&(end, _)| end <= index) {
            owners.pop();
        }
        if node.token {
            continue;
        }
        if matches!(
            node.kind.as_str(),
            "DESTRUCTURING_DECLARATION"
                | "DESTRUCTURING_DECLARATION_ENTRY"
                | "COMPANION_BLOCK"
                | "SCRIPT"
        ) {
            return Err(UNSUPPORTED);
        }
        if !matches!(
            node.kind.as_str(),
            "CLASS"
                | "OBJECT_DECLARATION"
                | "FUN"
                | "PROPERTY"
                | "TYPEALIAS"
                | "ENUM_ENTRY"
                | "PRIMARY_CONSTRUCTOR"
                | "SECONDARY_CONSTRUCTOR"
                | "VALUE_PARAMETER"
        ) {
            continue;
        }
        let mut identifier = None;
        let mut constructor = None;
        let mut interface = false;
        let mut class = false;
        let mut object = false;
        let mut function = false;
        let mut property = false;
        let mut alias = false;
        let mut enumeration = false;
        let mut child = index + 1;
        while child < node.subtree_end {
            let part = &elements[child];
            if part.token {
                match part.kind.as_str() {
                    "IDENTIFIER" => {
                        if identifier.replace(part).is_some() {
                            return Err(UNSUPPORTED);
                        }
                    }
                    "CONSTRUCTOR_KEYWORD" => {
                        if constructor.replace(part).is_some() {
                            return Err(UNSUPPORTED);
                        }
                    }
                    "INTERFACE_KEYWORD" => interface = true,
                    "CLASS_KEYWORD" => class = true,
                    "OBJECT_KEYWORD" => object = true,
                    "FUN_KEYWORD" => function = true,
                    "VAL_KEYWORD" | "VAR_KEYWORD" => property = true,
                    "TYPE_ALIAS_KEYWORD" => alias = true,
                    _ => {}
                }
            } else if part.kind == "MODIFIER_LIST" {
                let mut modifier = child + 1;
                while modifier < part.subtree_end {
                    let token = &elements[modifier];
                    if token.token && token.kind == "ENUM_KEYWORD" {
                        enumeration = true;
                    }
                    modifier = token.subtree_end;
                }
            }
            child = part.subtree_end;
        }
        let parent = node.parent.map(|p| &elements[p]);
        let (name, kind, fallback) = match node.kind.as_str() {
            "CLASS" if identifier.is_some() && (class ^ interface) => {
                let kind = if interface {
                    11
                } else if enumeration {
                    10
                } else {
                    5
                };
                (identifier, kind, kind)
            }
            "OBJECT_DECLARATION" if object => {
                if identifier.is_none() && parent.is_some_and(|p| p.kind == "OBJECT_LITERAL") {
                    continue;
                }
                (identifier, 19, 5)
            }
            "FUN" if function => {
                // The producer also permits absent names in declaration contexts;
                // only expression contexts can be anonymous-function exemptions.
                if identifier.is_none() {
                    if parent.is_none_or(|p| matches!(p.kind.as_str(), "KT_FILE" | "CLASS_BODY")) {
                        return Err(UNSUPPORTED);
                    }
                    continue;
                }
                let kind = if parent.is_some_and(|p| p.kind == "CLASS_BODY") {
                    6
                } else {
                    12
                };
                (identifier, kind, kind)
            }
            "PROPERTY" if property => (identifier, 7, 7),
            "TYPEALIAS" if alias => (identifier, 5, 5),
            "ENUM_ENTRY" => (identifier, 22, 10),
            "PRIMARY_CONSTRUCTOR" => {
                // No invented class-name selection for an implicit constructor.
                if constructor.is_none() {
                    continue;
                }
                (constructor, 9, 9)
            }
            "SECONDARY_CONSTRUCTOR" => (constructor, 9, 9),
            "VALUE_PARAMETER" => {
                if !property {
                    continue;
                }
                let primary = node
                    .parent
                    .and_then(|p| elements[p].parent)
                    .is_some_and(|p| elements[p].kind == "PRIMARY_CONSTRUCTOR");
                if !primary {
                    return Err(UNSUPPORTED);
                }
                (identifier, 7, 7)
            }
            _ => return Err(UNSUPPORTED),
        };
        let name = name.ok_or(UNSUPPORTED)?;
        if node.start > name.start || name.start >= name.end || name.end > node.end {
            return Err(UNSUPPORTED);
        }
        let owner = owners.last().map(|&(_, owner)| owner);
        let depth = owner.map_or(1, |owner| symbols[owner].depth + 1);
        if symbols.len() == MAX_SYMBOLS || depth > MAX_SYMBOL_DEPTH {
            return Err("document symbol output or nesting limit exceeded");
        }
        let spelling = tree.source().get(name.start..name.end).ok_or(UNSUPPORTED)?;
        let ordinal = symbols.len();
        symbols.push(Symbol {
            node,
            name,
            spelling,
            kind: client.kind(kind, fallback)?,
            owner,
            depth,
        });
        owners.push((node.subtree_end, ordinal));
    }
    Ok(symbols)
}

pub(crate) fn render(tree: &Parsed, client: &Client, uri: &str) -> Result<Value, &'static str> {
    let symbols = declarations(tree, client)?;
    // A conservative wire-size ceiling, including worst-case JSON escaping,
    // flat URI/container repetition, numeric fields and structural punctuation.
    let mut rendered_bytes = 2usize;
    for symbol in &symbols {
        let extra = if client.hierarchical {
            0
        } else {
            uri.len()
                + symbol
                    .owner
                    .map_or(0, |owner| symbols[owner].spelling.len())
        };
        rendered_bytes += 512 + 6 * (symbol.spelling.len() + extra);
        if rendered_bytes > MAX_RENDERED_BYTES {
            return Err("document symbol rendered output limit exceeded");
        }
    }
    let mut offsets: Vec<_> = symbols
        .iter()
        .flat_map(|s| [s.node.start, s.node.end, s.name.start, s.name.end])
        .collect();
    offsets.sort_unstable();
    offsets.dedup();
    let positions = crate::structure::positions(tree.source(), &offsets);
    let at = |offset| {
        positions[offsets
            .binary_search(&offset)
            .expect("retained symbol endpoint")]
    };
    let range = |start, end| Range {
        start: at(start),
        end: at(end),
    };
    if !client.hierarchical {
        return Ok(Value::Array(symbols.iter().map(|s| {
            let mut value = json!({"name":s.spelling,"kind":s.kind,"location":{"uri":uri,"range":range(s.node.start,s.node.end)}});
            if let Some(owner) = s.owner { value["containerName"] = json!(symbols[owner].spelling); }
            value
        }).collect()));
    }
    // Build bottom-up without recursively walking the CST or cloning JSON subtrees.
    let mut children: Vec<Vec<Value>> = (0..symbols.len()).map(|_| Vec::new()).collect();
    let mut roots = Vec::new();
    for (index, symbol) in symbols.into_iter().enumerate().rev() {
        let mut nested = std::mem::take(&mut children[index]);
        nested.reverse();
        let value = json!({"name":symbol.spelling,"kind":symbol.kind,
            "range":range(symbol.node.start,symbol.node.end),
            "selectionRange":range(symbol.name.start,symbol.name.end),"children":nested});
        if let Some(owner) = symbol.owner {
            children[owner].push(value);
        } else {
            roots.push(value);
        }
    }
    roots.reverse();
    Ok(Value::Array(roots))
}
