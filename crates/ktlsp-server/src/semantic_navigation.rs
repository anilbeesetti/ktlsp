//! Deliberately finite local binding, never a global unresolved-name answer.
use ktlsp_parser::{
    SnapshotResult, SourceSnapshot,
    local_value_binding::{
        LocalValueAnchor, LocalValueOccurrences, bind_local_value, local_value_occurrences,
        visible_local_values,
    },
    lowering::{lower_declarations, reference_site},
    structural_regions::build_regions,
};
use ktlsp_text::Range;
use serde_json::{Value, json};

const UNSUPPORTED: &str = "navigation requires a supported unambiguous function parameter or preceding local val binding; global resolution is unavailable";

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
    Definition {
        offset: usize,
    },
    Hover {
        offset: usize,
        format: HoverFormat,
    },
    References {
        offset: usize,
        include_declaration: bool,
    },
    Highlights {
        offset: usize,
    },
    Completion {
        offset: usize,
    },
}

pub(crate) fn render(
    result: &SnapshotResult,
    snapshot: &SourceSnapshot,
    query: Query,
) -> Result<Value, &'static str> {
    let tree = result.tree_for(snapshot).map_err(|_| UNSUPPORTED)?;
    let offset = match query {
        Query::Definition { offset }
        | Query::Hover { offset, .. }
        | Query::References { offset, .. }
        | Query::Highlights { offset }
        | Query::Completion { offset } => offset,
    };
    let mut references = tree.elements().iter().enumerate().filter(|(_, e)| {
        !e.token
            && e.kind == "REFERENCE_EXPRESSION"
            && e.start <= offset
            && (offset < e.end || matches!(query, Query::Completion { .. }) && offset == e.end)
    });
    let reference = references.next();
    if references.next().is_some() {
        return Err(UNSUPPORTED);
    }
    if matches!(query, Query::Completion { .. }) {
        let (index, _) =
            reference.ok_or("completion requires an existing whole-initializer identifier")?;
        let lowered = lower_declarations(result, snapshot).map_err(|_| UNSUPPORTED)?;
        let directory = build_regions(&lowered, result, snapshot).map_err(|_| UNSUPPORTED)?;
        // One semantic query authenticates the original site and every candidate,
        // without binding the unresolved prefix or resetting a per-item budget.
        let visible = visible_local_values(&directory, result, index, snapshot)
            .map_err(|_| "completion requires supported unambiguous finite local visibility")?;
        return render_completion(&visible, snapshot.source(), offset);
    }
    let occurrence_query = matches!(query, Query::References { .. } | Query::Highlights { .. });
    let declaration_hover = matches!(query, Query::Hover { .. })
        && reference.is_none()
        && tree.elements().iter().any(|element| {
            element.token
                && element.kind == "IDENTIFIER"
                && element.start <= offset
                && offset < element.end
                && element.parent.is_some_and(|parent| {
                    matches!(
                        tree.elements()[parent].kind.as_str(),
                        "CLASS" | "FUN" | "PROPERTY" | "VALUE_PARAMETER"
                    )
                })
        });
    if reference.is_none()
        && !declaration_hover
        && (!occurrence_query
            || !tree.elements().iter().any(|element| {
                element.token
                    && element.kind == "IDENTIFIER"
                    && element.start <= offset
                    && offset < element.end
            }))
    {
        return Ok(Value::Null);
    }
    let lowered = lower_declarations(result, snapshot).map_err(|_| UNSUPPORTED)?;
    if declaration_hover {
        let mut declarations = lowered.declarations().iter().filter(|declaration| {
            let range = declaration.name_range();
            range.start <= offset && offset < range.end
        });
        let target = declarations.next().ok_or(UNSUPPORTED)?;
        if declarations.next().is_some() || target.incomplete() {
            return Err(UNSUPPORTED);
        }
        return render_target(tree.source(), snapshot, query, target, target.name_range());
    }
    let directory = build_regions(&lowered, result, snapshot).map_err(|_| UNSUPPORTED)?;
    if occurrence_query {
        let anchor = if let Some((index, _)) = reference {
            LocalValueAnchor::ReferenceExpression(index)
        } else {
            let mut declarations = lowered.declarations().iter().filter(|declaration| {
                let range = declaration.name_range();
                range.start <= offset && offset < range.end
            });
            let target = declarations.next().ok_or(UNSUPPORTED)?;
            if declarations.next().is_some() {
                return Err(UNSUPPORTED);
            }
            LocalValueAnchor::Declaration(target)
        };
        // Authentication, use-anchor binding and complete occurrence collection
        // are one semantic query under the kernel's single shared work budget.
        let occurrences = local_value_occurrences(&directory, result, anchor, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        return render_occurrences(&occurrences, snapshot, query);
    }
    let site = reference_site(
        &lowered,
        result,
        snapshot,
        reference.expect("reference-only query checked").0,
    )
    .map_err(|_| UNSUPPORTED)?;
    let binding = bind_local_value(&directory, &site, snapshot).map_err(|_| UNSUPPORTED)?;
    let target = binding.target();
    let bytes = match query {
        Query::Definition { .. } => target.name_range(),
        Query::Hover { .. } => site.range(),
        Query::References { .. } | Query::Highlights { .. } | Query::Completion { .. } => {
            unreachable!("rendered above")
        }
    };
    render_target(tree.source(), snapshot, query, target, bytes)
}

fn render_target(
    source: &str,
    snapshot: &SourceSnapshot,
    query: Query,
    target: &ktlsp_parser::lowering::Declaration<'_>,
    bytes: std::ops::Range<usize>,
) -> Result<Value, &'static str> {
    // Two authenticated original endpoints, using the existing UTF-16 sweep.
    let positions = crate::structure::positions(source, &[bytes.start, bytes.end]);
    let range = Range {
        start: positions[0],
        end: positions[1],
    };
    match query {
        Query::Definition { .. } => Ok(json!({"uri":snapshot.document(), "range":range})),
        Query::Hover { format, .. } => {
            let declaration = &source[target.range()];
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
        Query::References { .. } | Query::Highlights { .. } | Query::Completion { .. } => {
            unreachable!("rendered above")
        }
    }
}

fn render_occurrences(
    occurrences: &LocalValueOccurrences<'_>,
    snapshot: &SourceSnapshot,
    query: Query,
) -> Result<Value, &'static str> {
    let include_declaration = !matches!(
        query,
        Query::References {
            include_declaration: false,
            ..
        }
    );
    let count = occurrences.occurrences().len() + usize::from(include_declaration);
    // Same conservative 1 MiB rendering ceiling as document symbols, including
    // worst-case JSON escaping of a repeated original URI. Kernel caps sites.
    let uri_bytes = if matches!(query, Query::References { .. }) {
        snapshot.document().len()
    } else {
        0
    };
    if 2 + count * (256 + 6 * uri_bytes) > 1024 * 1024 {
        return Err("local occurrence rendered output limit exceeded");
    }
    let mut offsets = Vec::with_capacity(count * 2);
    if include_declaration {
        let range = occurrences.target().name_range();
        offsets.extend([range.start, range.end]);
    }
    for site in occurrences.occurrences() {
        let range = site.range();
        offsets.extend([range.start, range.end]);
    }
    // The authenticated target precedes every retained site; sites are already
    // unique and source ordered. One sorted original-byte UTF-16 sweep.
    let positions = crate::structure::positions(snapshot.source(), &offsets);
    let values = positions
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .map(|(index, positions)| {
            let range = Range {
                start: positions[0],
                end: positions[1],
            };
            if matches!(query, Query::References { .. }) {
                json!({"uri":snapshot.document(),"range":range})
            } else {
                let kind = if include_declaration && index == 0 {
                    3
                } else {
                    2
                };
                json!({"range":range,"kind":kind})
            }
        })
        .collect::<Vec<_>>();
    Ok(Value::Array(values))
}

fn render_completion(
    visible: &ktlsp_parser::local_value_binding::VisibleLocalValues<'_>,
    source: &str,
    offset: usize,
) -> Result<Value, &'static str> {
    const LIMIT: &str = "local completion rendered output limit exceeded";
    let bytes = visible.site().range();
    let raw = visible.site().raw_spelling();
    let relative = offset.checked_sub(bytes.start).ok_or(UNSUPPORTED)?;
    let prefix = if raw.starts_with('`') {
        // The semantic query authenticated a complete quoted token. Exclude the
        // opening quote and, at token END, its closing quote; never validate a
        // partial prefix as a complete Kotlin name.
        raw.get(1.min(relative)..relative.min(raw.len() - 1))
    } else {
        raw.get(..relative)
    }
    .ok_or(UNSUPPORTED)?;
    let positions = crate::structure::positions(source, &[bytes.start, bytes.end]);
    let range = Range {
        start: positions[0],
        end: positions[1],
    };
    if range.start.line != range.end.line {
        return Err(UNSUPPORTED);
    }
    let mut work = prefix.len();
    let mut rendered = 64usize;
    // Preflight ALL candidates before allocating repeated JSON strings. Charge
    // both passes' spelling validation/prefix comparison against one 128 KiB
    // rendering-work ceiling, separate from the kernel's shared semantic budget.
    for target in visible.values() {
        let raw = target.raw_name();
        work = work
            .checked_add(2 * (raw.len() + prefix.len().min(raw.len())))
            .filter(|work| *work <= 131_072)
            .ok_or(LIMIT)?;
        let spelling = target.semantic_spelling().ok_or(UNSUPPORTED)?;
        if spelling.starts_with(prefix) {
            // Conservative JSON escaping (six bytes per input byte), both raw
            // copies, semantic filter and the shared numeric range per item.
            rendered = rendered
                .checked_add(256 + 6 * (2 * raw.len() + spelling.len()))
                .filter(|size| *size <= 1024 * 1024)
                .ok_or(LIMIT)?;
        }
    }
    let items = visible
        .values()
        .iter()
        .filter_map(|target| {
            let spelling = target.semantic_spelling()?;
            spelling.starts_with(prefix).then(|| {
                json!({
                    "label":target.raw_name(),
                    "filterText":spelling,
                    "kind":6,
                    "textEdit":{"range":range,"newText":target.raw_name()}
                })
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"isIncomplete":true,"items":items}))
}
