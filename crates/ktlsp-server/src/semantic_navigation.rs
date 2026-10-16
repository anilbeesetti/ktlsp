//! Deliberately finite local binding, never a global unresolved-name answer.
use ktlsp_parser::{
    SnapshotResult, SourceSnapshot,
    local_value_binding::bind_local_value,
    lowering::{lower_declarations, reference_site},
    structural_regions::build_regions,
};
use ktlsp_text::Range;
use serde_json::{Value, json};

const UNSUPPORTED: &str = "navigation requires a supported unambiguous preceding local val binding; global resolution is unavailable";

#[derive(Debug, Clone, Copy)]
pub(crate) enum HoverFormat {
    Plaintext,
    Markdown,
}
impl HoverFormat {
    pub(crate) fn negotiate(capabilities: &Value) -> Option<Self> {
        let Some(hover) = capabilities
            .get("textDocument")
            .and_then(|text| text.get("hover"))
        else {
            return Some(Self::Plaintext);
        };
        let hover = hover.as_object()?;
        let Some(formats) = hover.get("contentFormat") else {
            return Some(Self::Plaintext);
        };
        let formats = formats.as_array()?;
        if !formats.iter().all(Value::is_string) {
            return None;
        }
        formats.iter().find_map(|format| match format.as_str() {
            Some("plaintext") => Some(Self::Plaintext),
            Some("markdown") => Some(Self::Markdown),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Query {
    Definition { offset: usize },
    Hover { offset: usize, format: HoverFormat },
}

pub(crate) fn render(
    result: &SnapshotResult,
    snapshot: &SourceSnapshot,
    query: Query,
) -> Result<Value, &'static str> {
    let tree = result.tree_for(snapshot).map_err(|_| UNSUPPORTED)?;
    let offset = match query {
        Query::Definition { offset } | Query::Hover { offset, .. } => offset,
    };
    let mut references = tree.elements().iter().enumerate().filter(|(_, e)| {
        !e.token && e.kind == "REFERENCE_EXPRESSION" && e.start <= offset && offset < e.end
    });
    let Some((index, _)) = references.next() else {
        return Ok(Value::Null);
    };
    if references.next().is_some() {
        return Err(UNSUPPORTED);
    }
    let lowered = lower_declarations(result, snapshot).map_err(|_| UNSUPPORTED)?;
    let directory = build_regions(&lowered, result, snapshot).map_err(|_| UNSUPPORTED)?;
    let site = reference_site(&lowered, result, snapshot, index).map_err(|_| UNSUPPORTED)?;
    let binding = bind_local_value(&directory, &site, snapshot).map_err(|_| UNSUPPORTED)?;
    let target = binding.target();
    let bytes = match query {
        Query::Definition { .. } => target.name_range(),
        Query::Hover { .. } => site.range(),
    };
    // Two authenticated original endpoints, using the existing UTF-16 sweep.
    let positions = crate::structure::positions(tree.source(), &[bytes.start, bytes.end]);
    let range = Range {
        start: positions[0],
        end: positions[1],
    };
    match query {
        Query::Definition { .. } => Ok(json!({"uri":snapshot.document(), "range":range})),
        Query::Hover { format, .. } => {
            let declaration = &tree.source()[target.range()];
            let (kind, value) = match format {
                HoverFormat::Plaintext => ("plaintext", declaration.to_owned()),
                HoverFormat::Markdown => {
                    // Longer than EVERY source run, so untrusted names/comments
                    // cannot terminate the code block. Source is parser-bounded.
                    let longest = declaration
                        .split(|c| c != '`')
                        .map(str::len)
                        .max()
                        .unwrap_or(0);
                    let fence = "`".repeat(3.max(longest + 1));
                    ("markdown", format!("{fence}kotlin\n{declaration}\n{fence}"))
                }
            };
            Ok(json!({"contents":{"kind":kind,"value":value},"range":range}))
        }
    }
}
