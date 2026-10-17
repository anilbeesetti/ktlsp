//! Deliberately finite local binding, never a global unresolved-name answer.
use crate::{ParserResult, imported_definition};
use ktlsp_parser::{
    SnapshotResult, SourceSnapshot,
    documentation::{self, Budget as DocumentationBudget},
    local_value_binding::{
        LocalValueAnchor, TypeParameterAnchor, ValueLookup, bind_lexical_type_parameter,
        bind_local_argument_label, bind_local_function_callee, bind_local_value,
        guarded_local_value_rename, guarded_type_parameter_rename, local_call_signature,
        local_value_occurrences, lookup_value, type_parameter_occurrences, visible_local_values,
    },
    lowering::{Declaration, lower_declarations, reference_site},
    structural_regions::build_regions,
};
use ktlsp_text::Range;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};

const UNSUPPORTED: &str = "navigation requires a supported unambiguous function parameter or preceding local val binding; general global resolution is unavailable";

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

// Context is wire shape only. Even plausible previous help never authenticates
// a declaration, an active formal or a current source snapshot.
pub(crate) fn signature_context(context: Option<&Value>) -> Result<(), &'static str> {
    let Some(context) = context else {
        return Ok(());
    };
    let context = context
        .as_object()
        .ok_or("invalid signature help context")?;
    if !context
        .get("triggerKind")
        .and_then(Value::as_u64)
        .is_some_and(|kind| (1..=3).contains(&kind))
        || !context.get("isRetrigger").is_some_and(Value::is_boolean)
        || context
            .get("triggerCharacter")
            .is_some_and(|value| !value.is_string())
    {
        return Err("invalid signature help context");
    }
    fn index(object: &serde_json::Map<String, Value>, key: &str) -> bool {
        object
            .get(key)
            .is_none_or(|value| value.as_u64().is_some_and(|n| u32::try_from(n).is_ok()))
    }
    fn documentation(value: Option<&Value>) -> bool {
        value.is_none_or(|value| {
            value.is_string()
                || value.as_object().is_some_and(|value| {
                    value
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| matches!(kind, "plaintext" | "markdown"))
                        && value.get("value").is_some_and(Value::is_string)
                })
        })
    }
    if let Some(help) = context.get("activeSignatureHelp") {
        let help = help.as_object().ok_or("invalid previous signature help")?;
        let signatures = help
            .get("signatures")
            .and_then(Value::as_array)
            .ok_or("invalid previous signatures")?;
        if !index(help, "activeSignature") || !index(help, "activeParameter") {
            return Err("invalid previous active signature index");
        }
        for signature in signatures {
            let signature = signature.as_object().ok_or("invalid previous signature")?;
            if !signature.get("label").is_some_and(Value::is_string)
                || !documentation(signature.get("documentation"))
                || !index(signature, "activeParameter")
            {
                return Err("invalid previous signature");
            }
            if let Some(parameters) = signature.get("parameters") {
                for parameter in parameters.as_array().ok_or("invalid previous parameters")? {
                    let parameter = parameter.as_object().ok_or("invalid previous parameter")?;
                    let label = parameter
                        .get("label")
                        .ok_or("previous parameter label missing")?;
                    let offsets = label.as_array().is_some_and(|offsets| {
                        offsets.len() == 2
                            && offsets.iter().all(|offset| {
                                offset.as_u64().is_some_and(|n| u32::try_from(n).is_ok())
                            })
                            && offsets[0].as_u64() <= offsets[1].as_u64()
                    });
                    if (!label.is_string() && !offsets)
                        || !documentation(parameter.get("documentation"))
                    {
                        return Err("invalid previous parameter");
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum Query {
    SignatureHelp {
        offset: usize,
    },
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
    cancelled: &AtomicBool,
    response_id_bound: usize,
    mut imported: Option<&mut imported_definition::Context<'_>>,
) -> ParserResult {
    if let Query::SignatureHelp { offset } = query {
        if response_id_bound
            .checked_add(256)
            .is_none_or(|bytes| bytes > 1_048_576)
        {
            return Err("signature help response limit exceeded".into());
        }
        let lowered = lower_declarations(result, snapshot).map_err(|_| UNSUPPORTED)?;
        let directory = build_regions(&lowered, result, snapshot).map_err(|_| UNSUPPORTED)?;
        let Some(proof) =
            local_call_signature(&directory, result, snapshot, offset).map_err(|_| UNSUPPORTED)?
        else {
            return Ok(Value::Null);
        };
        let (label, parameter) = proof
            .into_labels(response_id_bound)
            .map_err(|_| UNSUPPORTED)?;
        let mut value = json!({"signatures":[{"label":null,"parameters":[{"label":null}]}],
            "activeSignature":0,"activeParameter":0});
        value["signatures"][0]["label"] = Value::String(label);
        value["signatures"][0]["parameters"][0]["label"] = Value::String(parameter);
        return Ok(value);
    }
    let observe_cancelled = || cancelled.load(Ordering::Relaxed);
    let mut budget = DocumentationBudget::new(&observe_cancelled);
    let hover = matches!(query, Query::Hover { .. });
    if hover {
        for bytes in [
            result.snapshot().document().len(),
            snapshot.document().len(),
            result.snapshot().source().len(),
            snapshot.source().len(),
            4,
        ] {
            budget.work(bytes).map_err(|_| UNSUPPORTED)?;
        }
    }
    let tree = result.tree_for(snapshot).map_err(|_| UNSUPPORTED)?;
    if let Some(context) = imported.as_deref_mut() {
        context.prepare_selected(tree);
    }
    let offset = match &query {
        Query::SignatureHelp { offset }
        | Query::Definition { offset }
        | Query::Hover { offset, .. }
        | Query::References { offset, .. }
        | Query::Highlights { offset }
        | Query::PrepareRename { offset }
        | Query::Rename { offset, .. }
        | Query::Completion { offset } => *offset,
    };
    let mut reference = None;
    let mut declaration_hover = false;
    let mut declaration_token = None;
    let mut identifier = false;
    for (index, element) in tree.elements().iter().enumerate() {
        if hover {
            budget
                .work(element.kind.len() + 1)
                .map_err(|_| UNSUPPORTED)?;
        }
        if !element.token
            && element.kind == "REFERENCE_EXPRESSION"
            && element.start <= offset
            && (offset < element.end
                || matches!(query, Query::Completion { .. }) && offset == element.end)
            && reference.replace((index, element)).is_some()
        {
            return Err(UNSUPPORTED.into());
        }
        if element.token
            && element.kind == "IDENTIFIER"
            && element.start <= offset
            && offset < element.end
        {
            identifier = true;
            declaration_hover |= hover
                && element.parent.is_some_and(|parent| {
                    matches!(
                        tree.elements()[parent].kind.as_str(),
                        "CLASS" | "FUN" | "PROPERTY" | "VALUE_PARAMETER"
                    )
                });
            if declaration_hover {
                declaration_token = Some(index);
            }
        }
    }
    declaration_hover &= reference.is_none();
    if matches!(query, Query::Completion { .. }) {
        let (index, _) =
            reference.ok_or("completion requires an existing whole-initializer identifier")?;
        let lowered = lower_declarations(result, snapshot).map_err(|_| UNSUPPORTED)?;
        let directory = build_regions(&lowered, result, snapshot).map_err(|_| UNSUPPORTED)?;
        // One semantic query authenticates the original site and every candidate,
        // without binding the unresolved prefix or resetting a per-item budget.
        let visible = visible_local_values(&directory, result, index, snapshot)
            .map_err(|_| "completion requires supported unambiguous finite local visibility")?;
        return render_completion(&visible, snapshot.source(), offset).map_err(Into::into);
    }
    let rename_query = matches!(query, Query::PrepareRename { .. } | Query::Rename { .. });
    let occurrence_query =
        rename_query || matches!(query, Query::References { .. } | Query::Highlights { .. });
    if reference.is_none() && !declaration_hover && (!occurrence_query || !identifier) {
        if rename_query {
            return Err(UNSUPPORTED.into());
        }
        return Ok(Value::Null);
    }
    if declaration_hover
        && documentation::candidate(tree, declaration_token.ok_or(UNSUPPORTED)?, &mut budget)
            .map_err(|_| UNSUPPORTED)?
    {
        let lowered =
            documentation::lower(result, snapshot, &mut budget).map_err(|_| UNSUPPORTED)?;
        let attached = documentation::select(&lowered, result, snapshot, offset, &mut budget)
            .map_err(|_| UNSUPPORTED)?;
        let prose = attached.prose().ok_or(UNSUPPORTED)?;
        let Query::Hover { format, .. } = query else {
            unreachable!("positive hover dispatch")
        };
        return render_documentation(
            snapshot.source(),
            Some(prose),
            attached.source_range(),
            attached.declaration().name_range(),
            format,
            response_id_bound,
            &mut budget,
        ).map_err(Into::into);
    }
    let lowered = lower_declarations(result, snapshot).map_err(|_| UNSUPPORTED)?;
    if declaration_hover {
        // No directly owned leading KDoc: retain the exact old lowerer and
        // renderer, including docless prefix refusals and fence acceptance.
        let mut declarations = lowered.declarations().iter().filter(|declaration| {
            let range = declaration.name_range();
            range.start <= offset && offset < range.end
        });
        let target = declarations.next().ok_or(UNSUPPORTED)?;
        if declarations.next().is_some() || target.incomplete() {
            return Err(UNSUPPORTED.into());
        }
        return render_target(tree.source(), snapshot, query, target, target.name_range())
            .map_err(Into::into);
    }
    let directory = build_regions(&lowered, result, snapshot).map_err(|_| UNSUPPORTED)?;
    if matches!(query, Query::Definition { .. })
        && let Some((index, expression)) = reference
        && expression
            .parent
            .is_some_and(|parent| tree.elements()[parent].kind == "USER_TYPE")
    {
        // Annotation role is selected positively, never as a failed-value fallback.
        let binding = bind_lexical_type_parameter(&directory, result, index, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        let target = binding.target();
        return render_target(tree.source(), snapshot, query, target, target.name_range())
            .map_err(Into::into);
    }
    if matches!(query, Query::Definition { .. })
        && let Some((index, expression)) = reference
        && expression
            .parent
            .is_some_and(|parent| tree.elements()[parent].kind == "VALUE_ARGUMENT_NAME")
    {
        // Positive label-role proof only; other consumers retain their guards.
        let binding = bind_local_argument_label(&directory, result, index, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        let target = binding.target();
        return render_target(tree.source(), snapshot, query, target, target.name_range())
            .map_err(Into::into);
    }
    if matches!(query, Query::Definition { .. })
        && let Some((index, expression)) = reference
        && expression
            .parent
            .is_some_and(|parent| tree.elements()[parent].kind == "CALL_EXPRESSION")
    {
        // Positive callee-role dispatch, never a failed-value textual fallback.
        let binding = bind_local_function_callee(&directory, result, index, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        let target = binding.target();
        return render_target(tree.source(), snapshot, query, target, target.name_range())
            .map_err(Into::into);
    }
    if occurrence_query {
        let annotation_anchor = reference.filter(|(_, expression)| {
            expression
                .parent
                .is_some_and(|parent| tree.elements()[parent].kind == "USER_TYPE")
        });
        let declaration_anchor = if reference.is_none() {
            let mut selected = lowered.declarations().iter().filter(|declaration| {
                let range = declaration.name_range();
                range.start <= offset && offset < range.end
            });
            let first = selected.next();
            if selected.next().is_some() {
                return Err(UNSUPPORTED.into());
            }
            first.filter(|declaration| declaration.kind() == "TYPE_PARAMETER")
        } else {
            None
        };
        let anchor = annotation_anchor
            .map(|(index, _)| TypeParameterAnchor::AnnotationReference(index))
            .or_else(|| declaration_anchor.map(TypeParameterAnchor::Declaration));
        if let Some(anchor) = anchor {
            // Positive type roles, never failed value-binding fallback. The
            // kernel authenticates the original target/body once per query.
            let found = if rename_query {
                let name = match &query {
                    Query::Rename { new_name, .. } => Some(new_name.as_str()),
                    _ => None,
                };
                guarded_type_parameter_rename(&directory, result, anchor, snapshot, name)
            } else {
                type_parameter_occurrences(&directory, result, anchor, snapshot)
            }
            .map_err(|_| UNSUPPORTED)?;
            let ranges = found
                .occurrences()
                .iter()
                .map(|annotation| annotation.range());
            return (if rename_query {
                render_rename(found.target(), ranges, snapshot, &query, offset)
            } else {
                render_occurrences(found.target(), ranges, snapshot, &query)
            }).map_err(Into::into);
        }
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
                return Err(UNSUPPORTED.into());
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
            return render_rename(
                occurrences.target(),
                occurrences.occurrences().iter().map(|site| site.range()),
                snapshot,
                &query,
                offset,
            ).map_err(Into::into);
        }
        let occurrences = local_value_occurrences(&directory, result, anchor, snapshot)
            .map_err(|_| UNSUPPORTED)?;
        return render_occurrences(
            occurrences.target(),
            occurrences.occurrences().iter().map(|site| site.range()),
            snapshot,
            &query,
        ).map_err(Into::into);
    }
    let site = reference_site(
        &lowered,
        result,
        snapshot,
        reference.expect("reference-only query checked").0,
    )
    .map_err(|_| UNSUPPORTED)?;
    let target = if matches!(query, Query::Definition { .. }) {
        match lookup_value(&directory, &site, snapshot).map_err(|_| UNSUPPORTED)? {
            ValueLookup::Local(binding) => binding.target(),
            ValueLookup::Outer(outer) => {
                if lowered.imports().is_empty() {
                    return Err(UNSUPPORTED.into());
                }
                return imported.ok_or(UNSUPPORTED)?
                    .render(outer, &directory, result, snapshot, response_id_bound);
            }
        }
    } else {
        bind_local_value(&directory, &site, snapshot)
            .map_err(|_| UNSUPPORTED)?.target()
    };
    let bytes = match query {
        Query::Definition { .. } => target.name_range(),
        Query::Hover { .. } => site.range(),
        Query::SignatureHelp { .. }
        | Query::References { .. }
        | Query::Highlights { .. }
        | Query::PrepareRename { .. }
        | Query::Rename { .. }
        | Query::Completion { .. } => unreachable!("rendered above"),
    };
    render_target(tree.source(), snapshot, query, target, bytes).map_err(Into::into)
}

// This is literal source prose, not a Markdown renderer. Escaping every ASCII
// punctuation character prevents links, entities, HTML and extension autolinks
// from acquiring source-authored destinations on Markdown clients.
fn render_documentation(
    source: &str,
    prose: Option<&str>,
    source_range: std::ops::Range<usize>,
    name_range: std::ops::Range<usize>,
    format: HoverFormat,
    response_id_bound: usize,
    budget: &mut DocumentationBudget<'_>,
) -> Result<Value, &'static str> {
    let declaration = source.get(source_range).ok_or(UNSUPPORTED)?;
    let markdown = matches!(format, HoverFormat::Markdown);
    let mut size = declaration.len();
    // Preflight exact escaped value bytes during the required size/fence pass,
    // not by serializing a second source-sized buffer.
    let mut json_value = 0usize;
    let mut longest = 0usize;
    let mut run = 0usize;
    for byte in declaration.bytes() {
        budget.work(1).map_err(|_| UNSUPPORTED)?;
        if markdown {
            run = if byte == b'`' {
                run.checked_add(1).ok_or(UNSUPPORTED)?
            } else {
                0
            };
            longest = longest.max(run);
        }
        json_value = json_value
            .checked_add(json_byte_width(byte))
            .ok_or(UNSUPPORTED)?;
    }
    let fence = 3.max(longest.checked_add(1).ok_or(UNSUPPORTED)?);
    if markdown {
        size = size
            .checked_add(fence.checked_mul(2).ok_or(UNSUPPORTED)?)
            .and_then(|n| n.checked_add(8))
            .ok_or(UNSUPPORTED)?;
        json_value = json_value
            .checked_add(fence.checked_mul(2).ok_or(UNSUPPORTED)?)
            .and_then(|n| n.checked_add(10))
            .ok_or(UNSUPPORTED)?;
    }
    if let Some(prose) = prose {
        size = size
            .checked_add(prose.len())
            .and_then(|n| n.checked_add(2))
            .ok_or(UNSUPPORTED)?;
        json_value = json_value.checked_add(4).ok_or(UNSUPPORTED)?;
        for byte in prose.bytes() {
            budget.work(1).map_err(|_| UNSUPPORTED)?;
            json_value = json_value
                .checked_add(json_byte_width(byte))
                .ok_or(UNSUPPORTED)?;
            if markdown && byte.is_ascii_punctuation() {
                size = size.checked_add(1).ok_or(UNSUPPORTED)?;
                json_value = json_value.checked_add(2).ok_or(UNSUPPORTED)?;
            }
        }
    }
    // Fixed JSON punctuation/keys/kind/jsonrpc need <160 bytes, four u32
    // coordinates need <=40 digits, framing <=32 bytes: all fit in 256.
    // The arbitrary request id has its own real-caller checked upper bound.
    let json_bytes = response_id_bound
        .checked_add(256)
        .and_then(|n| n.checked_add(json_value))
        .ok_or(UNSUPPORTED)?;
    budget.output(json_bytes).map_err(|_| UNSUPPORTED)?;
    let mut value = String::new();
    budget
        .reserve_string(&mut value, size)
        .map_err(|_| UNSUPPORTED)?;
    if let Some(prose) = prose {
        append_documentation(&mut value, prose, markdown, budget)?;
        append_documentation(&mut value, "\n\n", false, budget)?;
    }
    if markdown {
        for _ in 0..fence {
            append_documentation(&mut value, "`", false, budget)?;
        }
        append_documentation(&mut value, "kotlin\n", false, budget)?;
    }
    append_documentation(&mut value, declaration, false, budget)?;
    if markdown {
        append_documentation(&mut value, "\n", false, budget)?;
        for _ in 0..fence {
            append_documentation(&mut value, "`", false, budget)?;
        }
    }
    let mut positions = [ktlsp_text::Position {
        line: 0,
        character: 0,
    }; 2];
    let mut index = 0;
    crate::structure::visit_positions(
        source,
        &[name_range.start, name_range.end],
        |units| budget.work(units),
        |position| {
            positions[index] = position;
            index += 1;
        },
    )
    .map_err(|_| UNSUPPORTED)?;
    let range = Range {
        start: positions[0],
        end: positions[1],
    };
    // Moving the owned String avoids serde_json::to_value's extra source copy.
    let mut result = json!({
        "contents":{"kind":if markdown { "markdown" } else { "plaintext" },"value":null},
        "range":range
    });
    result["contents"]["value"] = Value::String(value);
    budget.work(1).map_err(|_| UNSUPPORTED)?;
    Ok(result)
}

fn json_byte_width(byte: u8) -> usize {
    match byte {
        b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
        0..=0x1f => 6,
        _ => 1,
    }
}

fn append_documentation(
    output: &mut String,
    text: &str,
    escape: bool,
    budget: &mut DocumentationBudget<'_>,
) -> Result<(), &'static str> {
    for character in text.chars() {
        budget.work(character.len_utf8()).map_err(|_| UNSUPPORTED)?;
        if escape && character.is_ascii_punctuation() {
            output.push('\\');
        }
        output.push(character);
    }
    Ok(())
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
        Query::SignatureHelp { .. }
        | Query::References { .. }
        | Query::Highlights { .. }
        | Query::PrepareRename { .. }
        | Query::Rename { .. }
        | Query::Completion { .. } => unreachable!("rendered above"),
    }
}

fn render_occurrences(
    target: &Declaration<'_>,
    ranges: impl ExactSizeIterator<Item = std::ops::Range<usize>>,
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
    let count = ranges.len() + usize::from(include_declaration);
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
        let range = target.name_range();
        offsets.extend([range.start, range.end]);
    }
    for range in ranges {
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
    target: &Declaration<'_>,
    ranges: impl ExactSizeIterator<Item = std::ops::Range<usize>> + Clone,
    snapshot: &SourceSnapshot,
    query: &Query,
    offset: usize,
) -> Result<Value, &'static str> {
    if matches!(query, Query::PrepareRename { .. }) {
        let selected = std::iter::once(target.name_range())
            .chain(ranges)
            .find(|range| range.start <= offset && offset < range.end)
            .ok_or(UNSUPPORTED)?;
        let positions =
            crate::structure::positions(snapshot.source(), &[selected.start, selected.end]);
        let placeholder = target.semantic_spelling().ok_or(UNSUPPORTED)?;
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
    let count = ranges.len() + 1;
    let removed = std::iter::once(target.name_range())
        .chain(ranges.clone())
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
    let name = target.name_range();
    offsets.extend([name.start, name.end]);
    for range in ranges {
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

#[cfg(test)]
mod documentation_rendering {
    use super::*;

    fn render(
        source: &str,
        prose: Option<&str>,
        format: HoverFormat,
        id_bound: usize,
    ) -> Result<Value, &'static str> {
        let cancelled = || false;
        let mut budget = DocumentationBudget::new(&cancelled);
        render_documentation(
            source,
            prose,
            0..source.len(),
            0..1,
            format,
            id_bound,
            &mut budget,
        )
    }

    #[test]
    fn literal_prose_escapes_all_markdown_destinations_and_preserves_unicode() {
        let prose = "<b>&amp; ![x](command:run) [r]: file:../x\n1. *a* `b` \\ https://x\n\t😀e\u{301}\r\n\rparagraph";
        let escaped = "\\<b\\>\\&amp\\; \\!\\[x\\]\\(command\\:run\\) \\[r\\]\\: file\\:\\.\\.\\/x\n1\\. \\*a\\* \\`b\\` \\\\ https\\:\\/\\/x\n\t😀e\u{301}\r\n\rparagraph";
        let source = "val /* ```` */ x=1";
        assert_eq!(
            render(source, Some(prose), HoverFormat::Markdown, 20).unwrap()["contents"]["value"],
            format!("{escaped}\n\n`````kotlin\n{source}\n`````")
        );
        assert_eq!(
            render(source, Some(prose), HoverFormat::Plaintext, 20).unwrap()["contents"]["value"],
            format!("{prose}\n\n{source}")
        );
        assert_eq!(
            render(source, None, HoverFormat::Markdown, 20).unwrap()["contents"]["value"],
            format!("`````kotlin\n{source}\n`````")
        );
        assert_eq!(
            render(source, None, HoverFormat::Plaintext, 20).unwrap()["contents"]["value"],
            source
        );
    }

    #[test]
    fn complete_json_id_envelope_exact_and_plus_one_refuse_without_truncation() {
        // The value contains JSON quotes, backslash, newline, NUL, Unicode;
        // its encoded payload is 2+2+2+6+4 bytes, not its UTF-8 source length.
        let source = "\"\\\n\0😀";
        let payload = 16;
        let exact = 1024 * 1024 - 256 - payload;
        assert_eq!(
            render(source, None, HoverFormat::Plaintext, exact).unwrap()["contents"]["value"],
            source
        );
        assert!(render(source, None, HoverFormat::Plaintext, exact + 1).is_err());
        assert!(render("x", None, HoverFormat::Plaintext, usize::MAX).is_err());
    }

    #[test]
    fn retained_growth_and_late_cancellation_refuse_atomically() {
        let prose = "!".repeat(65536);
        assert!(render("x", Some(&prose), HoverFormat::Markdown, 20).is_err());
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() > 128
        };
        let mut budget = DocumentationBudget::new(&cancelled);
        let source = format!("val x=\"{}\"", "a".repeat(4096));
        assert!(
            render_documentation(
                &source,
                Some("safe"),
                0..source.len(),
                4..5,
                HoverFormat::Markdown,
                20,
                &mut budget
            )
            .is_err()
        );
        assert!(calls.get() > 128);
    }

    #[test]
    fn original_name_utf16_uses_shared_lf_crlf_and_lone_cr_sweep() {
        let source = "// 😀\r\n// e\u{301}\rval `x😀`=1\n";
        let start = source.find("`x").unwrap();
        let cancelled = || false;
        let mut budget = DocumentationBudget::new(&cancelled);
        let result = render_documentation(
            source,
            Some("paragraph"),
            start - 4..source.len(),
            start..start + "`x😀`".len(),
            HoverFormat::Plaintext,
            20,
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            result["range"],
            json!({
                "start":{"line":2,"character":4},"end":{"line":2,"character":9}
            })
        );
    }
}
