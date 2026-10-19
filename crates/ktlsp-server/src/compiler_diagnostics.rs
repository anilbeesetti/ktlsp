//! Singleton compiler diagnostics from authenticated, supervised worker output.
use crate::{ParserFailure, ParserResult, structure, workspace_symbols};
use ktlsp_analysis::{Analysis, Error, Severity};
use ktlsp_parser::SourceSnapshot;
use ktlsp_text::Range;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_RENDERED_BYTES: usize = 1024 * 1024;
const LIMIT: &str = "compiler diagnostic output limit exceeded";

pub(super) fn standalone(options: Option<&Value>) -> Result<bool, &'static str> {
    let Some(mode) = options.and_then(|options| options.get("nativeAnalysis")) else {
        return Ok(false);
    };
    if mode.as_str() == Some("standalone") {
        Ok(true)
    } else {
        Err("nativeAnalysis must be the literal string 'standalone'")
    }
}

pub(super) fn params(params: Option<&Value>) -> Result<&str, (i32, &'static str)> {
    let map = params
        .ok_or((-32602, "diagnostic params missing"))
        .and_then(|params| crate::object(params).map_err(|error| (-32602, error)))?;
    for key in ["identifier", "previousResultId"] {
        if map.get(key).is_some_and(|value| !value.is_string()) {
            return Err((-32602, "diagnostic identifiers must be strings"));
        }
    }
    for key in ["workDoneToken", "partialResultToken"] {
        if map.get(key).is_some_and(|token| {
            !token.is_string()
                && token
                    .as_i64()
                    .is_none_or(|number| i32::try_from(number).is_err())
        }) {
            return Err((-32602, "progress token must be a string or LSP integer"));
        }
    }
    crate::required(map, "textDocument")
        .and_then(crate::object)
        .and_then(|document| crate::required(document, "uri"))
        .and_then(crate::uri_value)
        .map_err(|error| (-32602, error))
}

pub(super) fn is_script(uri: &str) -> bool {
    let mut path = uri.split(['?', '#']).next().unwrap_or(uri);
    // URI spelling is authoritative, but percent-encoded extensions are still
    // scripts. Decode only the suffix, without allocating a filesystem path.
    for &expected in b"stk." {
        let bytes = path.as_bytes();
        let Some(&last) = bytes.last() else {
            return false;
        };
        let (actual, width) = if bytes.len() >= 3 && bytes[bytes.len() - 3] == b'%' {
            let Ok(actual) = u8::from_str_radix(&path[path.len() - 2..], 16) else {
                return false;
            };
            (actual, 3)
        } else {
            (last, 1)
        };
        if actual != expected {
            return false;
        }
        path = &path[..path.len() - width];
    }
    true
}

pub(super) fn failure(error: Error) -> ParserFailure {
    match error {
        Error::Parser(error) => crate::parser_failure(error),
        Error::Resource => {
            ParserFailure::Protocol(-32803, "compiler analysis resource limit exceeded")
        }
        Error::InvalidResponse => {
            ParserFailure::Protocol(-32803, "compiler output validation failed")
        }
        Error::Unsupported => {
            ParserFailure::Protocol(-32803, "compiler diagnostics do not support Kotlin scripts")
        }
    }
}

pub(super) fn render(
    analysis: &Analysis,
    snapshot: &SourceSnapshot,
    mut items: Vec<Value>,
    cancelled: &AtomicBool,
    response_id_bound: usize,
) -> ParserResult {
    let check_cancelled = || {
        if cancelled.load(Ordering::Relaxed) {
            Err(ParserFailure::Protocol(-32800, "Request cancelled"))
        } else {
            Ok(())
        }
    };
    check_cancelled()?;
    let diagnostics = analysis.diagnostics_for(snapshot).map_err(failure)?;
    if items
        .len()
        .checked_add(diagnostics.len())
        .is_none_or(|count| count > structure::MAX_DIAGNOSTICS)
        || items.iter().any(|item| {
            matches!(
                item["code"].as_str(),
                Some("analysis-limit" | "structural-diagnostic-limit")
            )
        })
    {
        return Err(LIMIT.into());
    }
    // Static lexical messages/codes and four bounded integer coordinates fit
    // 512 bytes each. Compiler messages use the shared actual JSON escaping count;
    // 256 bytes cover their remaining fields, separators and coordinate digits.
    let mut bytes = response_id_bound
        .checked_add(256)
        .and_then(|bytes| bytes.checked_add(items.len() * 512))
        .ok_or(LIMIT)?;
    for diagnostic in diagnostics {
        check_cancelled()?;
        bytes = workspace_symbols::json_string_bytes(&diagnostic.message)
            .and_then(|message| message.checked_add(256))
            .and_then(|diagnostic| bytes.checked_add(diagnostic))
            .ok_or(LIMIT)?;
        if bytes > MAX_RENDERED_BYTES || bytes > ktlsp_protocol::MAX_PAYLOAD_BYTES {
            return Err(LIMIT.into());
        }
    }
    if bytes > MAX_RENDERED_BYTES || bytes > ktlsp_protocol::MAX_PAYLOAD_BYTES {
        return Err(LIMIT.into());
    }
    let source = snapshot.source();
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(diagnostics.len() * 2)
        .map_err(|_| LIMIT)?;
    for diagnostic in diagnostics {
        if diagnostic.start > diagnostic.end {
            return Err(failure(Error::InvalidResponse));
        }
        for offset in [diagnostic.start, diagnostic.end] {
            // visit_positions expects strict boundaries; authenticate UTF-8 and
            // reject the unaddressable interior of an original CRLF first.
            if !source.is_char_boundary(offset)
                || (source.as_bytes().get(offset) == Some(&b'\n')
                    && offset != 0
                    && source.as_bytes()[offset - 1] == b'\r')
            {
                return Err(failure(Error::InvalidResponse));
            }
            offsets.push(offset);
        }
    }
    offsets.sort_unstable();
    offsets.dedup();
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(offsets.len())
        .map_err(|_| LIMIT)?;
    structure::visit_positions(
        source,
        &offsets,
        |_| check_cancelled(),
        |position| positions.push(position),
    )?;
    items
        .try_reserve_exact(diagnostics.len())
        .map_err(|_| LIMIT)?;
    for diagnostic in diagnostics {
        check_cancelled()?;
        let at = |offset| {
            positions[offsets
                .binary_search(&offset)
                .expect("retained compiler endpoint")]
        };
        items.push(json!({
            "range":Range { start:at(diagnostic.start), end:at(diagnostic.end) },
            "severity":match diagnostic.severity { Severity::Error => 1, Severity::Warning => 2 },
            "source":"krusty",
            "message":diagnostic.message,
        }));
    }
    check_cancelled()?;
    Ok(json!({"kind":"full","items":items}))
}
