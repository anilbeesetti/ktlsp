//! Eager native syntactic search, not the unavailable official matcher/index.
//! Empty means all; otherwise exact case-sensitive literal projected spelling.
//! Whitespace/backticks/Unicode remain literal; no trim, normalization, fuzzy or
//! qualified matching. Original project/entry/CST order and duplicates survive.
//! Containers/kinds/ranges are the document outline's native projection: nearest
//! emitted owner, full declaration range, Object/Class and EnumMember/Enum
//! negotiated fallbacks. No official data-class/const/operator/package policy.
//!
//! A request retains one immutable workspace generation plus every mapped open
//! member's incarnation/version/exact bytes, and owns one global native slot.
//! Closed members and every original model verify on both sides of sequential
//! actual parser work, including negative/empty results and all-overlay models.
//! Additions require manual reload; changed/deleted recorded sources refuse.
//! Existing per-parser limits/deadlines and the two-generation ceiling stay put.
//! There is no persistent index, semantic ID, library search, partial response,
//! resolve/progress provider, hard aggregate latency or RSS promise.
use crate::{ParserFailure, ParserResult, document_symbols, workspace};
use ktlsp_import::source_inventory::{self, SnapshotBudget};
use ktlsp_parser::SourceSnapshot;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const MAX_SOURCES: usize = 1024;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECLARATIONS: usize = 16384;
const MAX_STORAGE: usize = 32 * 1024 * 1024;
const MAX_QUERY_BYTES: usize = 8192;
// Aggregate and workspace-waiter failures use bounded static messages; original
// selected-document/import reasons stay on their existing consumer paths.
const MAX_FAILURE_BYTES: usize = 512;
// response_error(id, -32803, "limit"): fixed JSON bytes excluding the encoded ID.
const COMPACT_ERROR_FIELDS_BYTES: usize = 65;
const LIMIT: &str = "workspace symbol aggregate limit exceeded";

#[derive(Debug)]
pub(super) struct Member {
    pub(super) source: SourceSnapshot,
    pub(super) overlay: Option<(Arc<str>, u64)>,
}
#[derive(Debug)]
pub(super) struct Capture {
    pub(super) snapshot: Option<Arc<workspace::Snapshot>>,
    pub(super) epoch: u64,
    pub(super) members: Vec<Member>,
    pub(super) closed: Vec<Vec<usize>>,
}

#[derive(Debug)]
pub(super) enum AdmissionError {
    UnreplyableId,
    Resource(&'static str),
}
impl From<&'static str> for AdmissionError {
    fn from(message: &'static str) -> Self {
        Self::Resource(message)
    }
}

/// Frozen monotonic aggregate allowances, including both verification phases.
/// Logical retained/requested bytes and work are not an RSS/latency guarantee.
#[derive(Debug, Default)]
pub(super) struct Budget {
    verification: SnapshotBudget,
    sources: usize,
    source_bytes: usize,
    parses: usize,
    declarations: usize,
    symbols: usize,
    storage: usize,
    rendered: usize,
}
fn charge(used: &mut usize, amount: usize, limit: usize) -> Result<(), &'static str> {
    let next = used.checked_add(amount).ok_or(LIMIT)?;
    *used = next.min(limit);
    if next > limit { Err(LIMIT) } else { Ok(()) }
}
pub(super) fn json_string_bytes(value: &str) -> Option<usize> {
    value.bytes().try_fold(2usize, |bytes, byte| {
        let width = match byte {
            b'"' | b'\\' | 8 | 9 | 10 | 12 | 13 => 2,
            0..=0x1f => 6,
            _ => 1,
        };
        bytes.checked_add(width)
    })
}
impl Budget {
    pub(super) fn new(query: &str, id: &Value) -> Result<Self, AdmissionError> {
        let mut budget = Self::default();
        // Count actual serde_json string escaping, without serializing/copying
        // the client ID. Numeric IDs need at most twenty bytes.
        let id_bytes = if let Some(id) = id.as_str() {
            if id.len() > ktlsp_protocol::MAX_PAYLOAD_BYTES - COMPACT_ERROR_FIELDS_BYTES - 2 {
                return Err(AdmissionError::UnreplyableId);
            }
            budget.work(id.len(), &AtomicBool::new(false))?;
            budget.storage(id.len())?;
            json_string_bytes(id).ok_or(AdmissionError::UnreplyableId)?
        } else {
            20
        };
        if id_bytes
            .checked_add(COMPACT_ERROR_FIELDS_BYTES)
            .is_none_or(|n| n > ktlsp_protocol::MAX_PAYLOAD_BYTES)
        {
            return Err(AdmissionError::UnreplyableId);
        }
        if id_bytes
            .checked_add(MAX_FAILURE_BYTES)
            .is_none_or(|n| n > ktlsp_protocol::MAX_PAYLOAD_BYTES)
        {
            return Err(AdmissionError::Resource("limit"));
        }
        budget.output(id_bytes.checked_add(64).ok_or(LIMIT)?)?;
        budget.query(query)?;
        Ok(budget)
    }
    pub(super) fn query(&mut self, query: &str) -> Result<(), &'static str> {
        if query.len() > MAX_QUERY_BYTES {
            return Err("workspace query limit exceeded");
        }
        self.work(query.len(), &AtomicBool::new(false))?;
        self.storage(query.len())
    }
    pub(super) fn work(
        &mut self,
        amount: usize,
        cancelled: &AtomicBool,
    ) -> Result<(), &'static str> {
        self.verification
            .reserve_work(amount, cancelled)
            .map_err(|_| LIMIT)
    }
    pub(super) fn storage(&mut self, amount: usize) -> Result<(), &'static str> {
        charge(&mut self.storage, amount, MAX_STORAGE)
    }
    pub(super) fn source(&mut self, bytes: usize) -> Result<(), &'static str> {
        if bytes > ktlsp_parser::MAX_SOURCE_BYTES {
            return Err("workspace source exceeds parser limits");
        }
        charge(&mut self.sources, 1, MAX_SOURCES)?;
        charge(&mut self.source_bytes, bytes, MAX_SOURCE_BYTES)?;
        self.storage(bytes)
    }
    fn output(&mut self, amount: usize) -> Result<(), &'static str> {
        charge(
            &mut self.rendered,
            amount,
            ktlsp_protocol::MAX_PAYLOAD_BYTES,
        )
    }
    fn parse(&mut self, bytes: usize, cancelled: &AtomicBool) -> Result<(), &'static str> {
        charge(&mut self.parses, 1, MAX_SOURCES)?;
        // Precharge the complete delegated chain BEFORE invoking the parser.
        // Logical units cover bounded reply/decode bytes, maximum validation
        // element/ancestor visits, and four source-byte passes (preflight,
        // normalization/map, worker input, original-source capture). This is
        // conservative logical accounting, not measured CPU/allocation/RSS/time.
        // Smaller replies and failed invocations never refund this reservation.
        // ponytail: fixed worst-case delegate reservation admits at most 42
        // tiny-source parses within 256 MiB; other work reduces that ceiling.
        // A parser-internal shared actual-work path could refine this later.
        let delegated = ktlsp_parser::MAX_OUTPUT_BYTES
            .checked_add(
                ktlsp_parser::MAX_ELEMENTS
                    .checked_mul(ktlsp_parser::MAX_NESTING)
                    .ok_or(LIMIT)?,
            )
            .and_then(|n| bytes.checked_mul(4).and_then(|bytes| n.checked_add(bytes)))
            .ok_or(LIMIT)?;
        self.work(delegated, cancelled)
    }
}

pub(super) fn query(params: Option<&Value>) -> Result<&str, (i32, &'static str)> {
    let params = params.ok_or((-32602, "workspace symbol params missing"))?;
    let map = crate::object(params).map_err(|e| (-32602, e))?;
    let query = crate::required(map, "query")
        .and_then(|v| v.as_str().ok_or("query must be a string"))
        .map_err(|e| (-32602, e))?;
    for name in ["workDoneToken", "partialResultToken"] {
        if map.get(name).is_some_and(|token| {
            !token.is_string() && token.as_i64().is_none_or(|n| i32::try_from(n).is_err())
        }) {
            return Err((-32602, "progress token must be a string or LSP integer"));
        }
    }
    if query.len() > MAX_QUERY_BYTES {
        return Err((-32803, "workspace query limit exceeded"));
    }
    Ok(query)
}
fn failure(error: &'static str, cancelled: &AtomicBool) -> ParserFailure {
    if cancelled.load(Ordering::Relaxed) {
        ParserFailure::Protocol(-32800, "Request cancelled")
    } else {
        ParserFailure::Protocol(-32803, error)
    }
}
fn source_failure(error: source_inventory::Error) -> ParserFailure {
    match error {
        source_inventory::Error::Budget(_) | source_inventory::Error::InvalidSource(_) => {
            ParserFailure::Protocol(
                -32803,
                "workspace verification resource or source limit exceeded",
            )
        }
        source_inventory::Error::Cancelled => ParserFailure::Protocol(-32800, "Request cancelled"),
        _ => ParserFailure::Protocol(
            -32801,
            "workspace source snapshot is stale or unavailable; reload required",
        ),
    }
}
fn render(
    tree: &ktlsp_parser::Parsed,
    client: &document_symbols::Client,
    uri: &str,
    query: &str,
    budget: &mut Budget,
    cancelled: &AtomicBool,
    result: &mut Vec<Value>,
) -> Result<(), &'static str> {
    let symbols =
        document_symbols::declarations_with_work(tree, client, |n| budget.work(n, cancelled))?;
    charge(&mut budget.declarations, symbols.len(), MAX_DECLARATIONS)?;
    budget.storage(
        symbols
            .len()
            .checked_mul(std::mem::size_of::<document_symbols::Symbol<'_>>())
            .ok_or(LIMIT)?,
    )?;
    let mut offsets = Vec::new();
    let endpoints = symbols.len().checked_mul(2).ok_or(LIMIT)?;
    budget.storage(
        endpoints
            .checked_mul(std::mem::size_of::<usize>() + std::mem::size_of::<ktlsp_text::Position>())
            .ok_or(LIMIT)?,
    )?;
    offsets.try_reserve_exact(endpoints).map_err(|_| LIMIT)?;
    for symbol in &symbols {
        budget.work(
            symbol
                .spelling
                .len()
                .checked_add(query.len())
                .ok_or(LIMIT)?,
            cancelled,
        )?;
        if symbol.spelling.contains(query) {
            charge(&mut budget.symbols, 1, MAX_DECLARATIONS)?;
            let container = symbol
                .owner
                .map_or(0, |owner| symbols[owner].spelling.len());
            let strings = symbol
                .spelling
                .len()
                .checked_add(uri.len())
                .and_then(|n| n.checked_add(container))
                .ok_or(LIMIT)?;
            let bytes = strings
                .checked_mul(6)
                .and_then(|n| n.checked_add(512))
                .ok_or(LIMIT)?;
            budget.output(bytes)?;
            budget.storage(bytes)?;
            budget.work(bytes, cancelled)?;
            offsets.extend([symbol.node.start, symbol.node.end]);
        }
    }
    let sort_work = offsets
        .len()
        .checked_mul(usize::BITS as usize - offsets.len().leading_zeros() as usize)
        .ok_or(LIMIT)?;
    budget.work(sort_work, cancelled)?;
    offsets.sort_unstable();
    offsets.dedup();
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(offsets.len())
        .map_err(|_| LIMIT)?;
    crate::structure::visit_positions(
        tree.source(),
        &offsets,
        |n| budget.work(n, cancelled),
        |p| positions.push(p),
    )?;
    result
        .try_reserve(budget.symbols.saturating_sub(result.len()))
        .map_err(|_| LIMIT)?;
    for symbol in &symbols {
        budget.work(
            symbol
                .spelling
                .len()
                .checked_add(query.len())
                .ok_or(LIMIT)?,
            cancelled,
        )?;
        if !symbol.spelling.contains(query) {
            continue;
        }
        let at = |offset| {
            positions[offsets
                .binary_search(&offset)
                .expect("projected endpoint retained")]
        };
        let range = ktlsp_text::Range {
            start: at(symbol.node.start),
            end: at(symbol.node.end),
        };
        let mut value =
            json!({"name":symbol.spelling,"kind":symbol.kind,"location":{"uri":uri,"range":range}});
        if let Some(owner) = symbol.owner {
            value["containerName"] = json!(symbols[owner].spelling);
        }
        result.push(value);
    }
    Ok(())
}

pub(super) fn run(
    capture: &Capture,
    provider: &document_symbols::Provider,
    client: &document_symbols::Client,
    query: &str,
    mut budget: Budget,
    cancelled: &AtomicBool,
) -> (ParserResult, bool) {
    let mut clean = true;
    let result = (|| {
        if let Some(snapshot) = &capture.snapshot {
            snapshot
                .verify_all(&capture.closed, &mut budget.verification, cancelled)
                .map_err(source_failure)?;
        }
        let mut result = Vec::new();
        for member in &capture.members {
            budget
                .parse(member.source.source().len(), cancelled)
                .map_err(|e| failure(e, cancelled))?;
            let (parsed, closed) = provider.parse(&member.source, cancelled);
            clean &= closed;
            let parsed = parsed.map_err(crate::parser_failure)?;
            // Never start another child after an unproved closure, even if a
            // worker happened to return a value alongside that refusal.
            if !closed {
                return Err(ParserFailure::Protocol(
                    -32803,
                    "parser I/O or cleanup failed",
                ));
            }
            let tree = parsed
                .tree_for(&member.source)
                .map_err(|_| ParserFailure::Protocol(-32801, "parser snapshot is stale"))?;
            for element in tree.elements() {
                budget
                    .work(
                        element
                            .kind
                            .len()
                            .checked_add(std::mem::size_of::<ktlsp_parser::Element>())
                            .ok_or(ParserFailure::Protocol(-32803, LIMIT))?,
                        cancelled,
                    )
                    .map_err(|e| failure(e, cancelled))?;
            }
            render(
                tree,
                client,
                member.source.document(),
                query,
                &mut budget,
                cancelled,
                &mut result,
            )
            .map_err(|e| failure(e, cancelled))?;
        }
        if let Some(snapshot) = &capture.snapshot {
            snapshot
                .verify_all(&capture.closed, &mut budget.verification, cancelled)
                .map_err(source_failure)?;
        }
        budget
            .work(1, cancelled)
            .map_err(|e| failure(e, cancelled))?;
        Ok(Value::Array(result))
    })();
    (result, clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_exact_and_exceeded_allowances_do_not_reset_per_source() {
        let mut budget = Budget::new("", &json!("actual\u{0000}id")).unwrap();
        for _ in 0..MAX_SOURCES {
            budget.source(MAX_SOURCE_BYTES / MAX_SOURCES).unwrap();
        }
        assert!(budget.source(1).is_err());
        charge(&mut budget.parses, MAX_SOURCES, MAX_SOURCES).unwrap();
        assert!(budget.parse(0, &AtomicBool::new(false)).is_err());
        let mut budget = Budget::default();
        budget
            .source(MAX_SOURCE_BYTES.min(ktlsp_parser::MAX_SOURCE_BYTES))
            .unwrap();
        assert!(budget.source(ktlsp_parser::MAX_SOURCE_BYTES + 1).is_err());
        let mut bytes = Budget::default();
        for _ in 0..(MAX_SOURCE_BYTES / ktlsp_parser::MAX_SOURCE_BYTES) {
            bytes.source(ktlsp_parser::MAX_SOURCE_BYTES).unwrap();
        }
        assert!(bytes.source(1).is_err());
        budget.storage(MAX_STORAGE - budget.storage).unwrap();
        assert!(budget.storage(1).is_err());
        budget.output(ktlsp_protocol::MAX_PAYLOAD_BYTES).unwrap();
        assert!(budget.output(1).is_err());
        charge(&mut budget.declarations, MAX_DECLARATIONS, MAX_DECLARATIONS).unwrap();
        assert!(charge(&mut budget.declarations, 1, MAX_DECLARATIONS).is_err());
        charge(&mut budget.symbols, MAX_DECLARATIONS, MAX_DECLARATIONS).unwrap();
        assert!(charge(&mut budget.symbols, 1, MAX_DECLARATIONS).is_err());
        let mut used = usize::MAX;
        assert!(charge(&mut used, 1, usize::MAX).is_err());
        let delegated = ktlsp_parser::MAX_OUTPUT_BYTES
            + ktlsp_parser::MAX_ELEMENTS * ktlsp_parser::MAX_NESTING
            + 4 * 19;
        for allowance in [delegated - 1, delegated] {
            let mut budget = Budget {
                verification: SnapshotBudget::new(source_inventory::SnapshotLimits {
                    work: allowance,
                    ..source_inventory::SnapshotLimits::default()
                })
                .unwrap(),
                ..Budget::default()
            };
            assert_eq!(
                budget.parse(19, &AtomicBool::new(false)).is_ok(),
                allowance == delegated
            );
            assert!(budget.parse(19, &AtomicBool::new(false)).is_err());
        }
    }
    #[test]
    fn actual_escaped_id_and_query_bounds_are_admitted_before_source_work() {
        assert!(Budget::new(&"a".repeat(MAX_QUERY_BYTES), &json!(u64::MAX)).is_ok());
        assert!(Budget::new(&"a".repeat(MAX_QUERY_BYTES + 1), &json!(0)).is_err());
        let id = json!("\u{0000}".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES / 6));
        assert!(Budget::new("", &id).is_err());
        let id = json!("a".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES / 2));
        assert!(Budget::new("", &id).is_ok());
        let escaped = json!("quote\" slash\\ tab\t newline\n \u{0001} 😀");
        let budget = Budget::new("", &escaped).unwrap();
        assert_eq!(
            budget.rendered,
            serde_json::to_vec(&escaped).unwrap().len() + 64
        );
        let mut budget = Budget::default();
        assert!(budget.work(1, &AtomicBool::new(true)).is_err());
    }
}
