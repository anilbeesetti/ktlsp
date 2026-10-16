//! Deliberately finite local binding, never a global unresolved-name answer.
use ktlsp_parser::{
    SnapshotResult, SourceSnapshot,
    local_value_binding::{
        LocalValueAnchor, LocalValueOccurrences, bind_lexical_type_parameter, bind_local_value,
        guarded_local_value_rename, local_value_occurrences, visible_local_values,
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

#[derive(Debug, Default)]
pub(crate) struct RenameClient {
    pub document_changes: bool,
    pub prepare: bool,
}
impl RenameClient {
    pub fn parse(capabilities: &Value) -> Result<Self, &'static str> {
        fn member<'a>(
            value: Option<&'a Value>,
            key: &str,
        ) -> Result<Option<&'a Value>, &'static str> {
            value
                .map(|v| v.as_object().ok_or("invalid rename capability object"))
                .transpose()
                .map(|object| object.and_then(|o| o.get(key)))
        }
        fn boolean(value: Option<&Value>) -> Result<bool, &'static str> {
            value
                .map(|v| v.as_bool().ok_or("invalid rename capability boolean"))
                .transpose()
                .map(|b| b.unwrap_or(false))
        }
        let workspace = member(Some(capabilities), "workspace")?;
        let edit = member(workspace, "workspaceEdit")?;
        let document_changes = boolean(member(edit, "documentChanges")?)?;
        let text = member(Some(capabilities), "textDocument")?;
        let rename = member(text, "rename")?;
        let prepare = boolean(member(rename, "prepareSupport")?)?;
        Ok(Self {
            document_changes,
            prepare,
        })
    }
}

#[derive(Debug)]
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
    PrepareRename {
        offset: usize,
    },
    Rename {
        offset: usize,
        new_name: String,
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
    let offset = match &query {
        Query::Definition { offset }
        | Query::Hover { offset, .. }
        | Query::References { offset, .. }
        | Query::Highlights { offset }
        | Query::PrepareRename { offset }
        | Query::Rename { offset, .. }
        | Query::Completion { offset } => *offset,
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
    let rename_query = matches!(query, Query::PrepareRename { .. } | Query::Rename { .. });
    let occurrence_query =
        rename_query || matches!(query, Query::References { .. } | Query::Highlights { .. });
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
        if rename_query {
            return Err(UNSUPPORTED);
        }
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
    if matches!(query, Query::Definition { .. })
        && let Some((index, expression)) = reference
        && expression.parent.is_some_and(|parent| tree.elements()[parent].kind == "USER_TYPE")
    {
        // Annotation role is selected positively, never as a failed-value fallback.
        let binding = bind_lexical_type_parameter(&directory, result, index, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        let target = binding.target();
        return render_target(tree.source(), snapshot, query, target, target.name_range());
    }
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
        if rename_query {
            let new_name = match &query {
                Query::Rename { new_name, .. } => Some(new_name.as_str()),
                _ => None,
            };
            let occurrences =
                guarded_local_value_rename(&directory, result, anchor, snapshot, new_name)
                    .map_err(|_| UNSUPPORTED)?;
            return render_rename(&occurrences, snapshot, &query, offset);
        }
        let occurrences = local_value_occurrences(&directory, result, anchor, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        return render_occurrences(&occurrences, snapshot, &query);
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
        Query::References { .. }
        | Query::Highlights { .. }
        | Query::PrepareRename { .. }
        | Query::Rename { .. }
        | Query::Completion { .. } => unreachable!("rendered above"),
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
        Query::References { .. }
        | Query::Highlights { .. }
        | Query::PrepareRename { .. }
        | Query::Rename { .. }
        | Query::Completion { .. } => unreachable!("rendered above"),
    }
}

fn render_occurrences(
    occurrences: &LocalValueOccurrences<'_>,
    snapshot: &SourceSnapshot,
    query: &Query,
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

fn rename_output_size(
    source_len: usize,
    removed: usize,
    count: usize,
    name_len: usize,
    uri_len: usize,
) -> Result<(), &'static str> {
    let inserted = count.checked_mul(name_len).ok_or(UNSUPPORTED)?;
    let resulting = source_len
        .checked_sub(removed)
        .and_then(|n| n.checked_add(inserted))
        .ok_or(UNSUPPORTED)?;
    let output = uri_len
        .checked_mul(6)
        .and_then(|n| n.checked_add(512))
        .and_then(|n| {
            count
                .checked_mul(name_len.checked_add(256)?)
                .and_then(|edits| n.checked_add(edits))
        })
        .ok_or(UNSUPPORTED)?;
    if resulting > ktlsp_parser::MAX_SOURCE_BYTES || output > 1024 * 1024 {
        return Err("rename source growth or rendered output limit exceeded");
    }
    Ok(())
}

fn render_rename(
    found: &LocalValueOccurrences<'_>,
    snapshot: &SourceSnapshot,
    query: &Query,
    offset: usize,
) -> Result<Value, &'static str> {
    if matches!(query, Query::PrepareRename { .. }) {
        let selected = std::iter::once(found.target().name_range())
            .chain(found.occurrences().iter().map(|site| site.range()))
            .find(|range| range.start <= offset && offset < range.end)
            .ok_or(UNSUPPORTED)?;
        let positions =
            crate::structure::positions(snapshot.source(), &[selected.start, selected.end]);
        let placeholder = found.target().semantic_spelling().ok_or(UNSUPPORTED)?;
        if placeholder
            .len()
            .checked_mul(6)
            .and_then(|n| n.checked_add(256))
            .is_none_or(|n| n > 1024 * 1024)
        {
            return Err(UNSUPPORTED);
        }
        return Ok(
            json!({"range":Range { start:positions[0], end:positions[1] }, "placeholder":placeholder}),
        );
    }
    let Query::Rename { new_name, .. } = query else {
        unreachable!("rename query")
    };
    let count = found.occurrences().len() + 1;
    let removed = std::iter::once(found.target().name_range())
        .chain(found.occurrences().iter().map(|site| site.range()))
        .try_fold(0usize, |n, range| n.checked_add(range.len()))
        .ok_or(UNSUPPORTED)?;
    // ASCII newName needs no JSON escaping. Validate growth/output BEFORE
    // allocating offsets, edit objects or repeated replacement strings.
    rename_output_size(
        snapshot.source().len(),
        removed,
        count,
        new_name.len(),
        snapshot.document().len(),
    )?;
    let mut offsets = Vec::with_capacity(count * 2);
    let name = found.target().name_range();
    offsets.extend([name.start, name.end]);
    for site in found.occurrences() {
        let range = site.range();
        offsets.extend([range.start, range.end]);
    }
    let positions = crate::structure::positions(snapshot.source(), &offsets);
    let edits = positions
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| json!({"range":Range { start:p[0], end:p[1] }, "newText":new_name}))
        .collect::<Vec<_>>();
    Ok(
        json!({"documentChanges":[{"textDocument":{"uri":snapshot.document(), "version":snapshot.revision()}, "edits":edits}]}),
    )
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

#[cfg(test)]
mod rename_limits {
    use super::*;
    #[test]
    fn growth_and_escaped_output_refuse_atomically() {
        rename_output_size(65536, 1, 1, 1, 1).unwrap();
        assert!(rename_output_size(65536, 1, 1, 2, 1).is_err());
        assert!(rename_output_size(0, 0, 1025, 1, 200000).is_err());
        assert!(rename_output_size(0, 0, usize::MAX, usize::MAX, usize::MAX).is_err());
    }
}
