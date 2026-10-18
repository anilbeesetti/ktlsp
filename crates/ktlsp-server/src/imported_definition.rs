//! Finite explicit-import value navigation over original captured module facts.
//! No persistent index, guessed source ownership or successful partial directory.
use crate::{
    ParserFailure, ParserResult, document_symbols,
    semantic_navigation::{HoverFormat, Query},
    structure, workspace_symbols,
};
use ktlsp_parser::{
    Parsed, SnapshotResult, SourceSnapshot,
    local_value_binding::{ImportedValueBinding, OuterValueLookup},
    lowering::lower_declarations,
    structural_regions::{RegionDirectory, build_regions},
};
use serde_json::json;
use std::sync::{Arc, atomic::AtomicBool};

const LIMIT: &str = "imported definition aggregate limit exceeded";
const UNSUPPORTED: &str = "definition requires one supported explicit import and one original closed-module source property";

pub(super) struct Context<'a> {
    capture: Result<Arc<workspace_symbols::Capture>, &'static str>,
    budget: workspace_symbols::Budget,
    provider: &'a document_symbols::Provider,
    cancelled: &'a AtomicBool,
    clean: &'a mut bool,
    used: bool,
}
impl<'a> Context<'a> {
    pub(super) fn new(
        capture: Result<Arc<workspace_symbols::Capture>, &'static str>,
        budget: workspace_symbols::Budget,
        provider: &'a document_symbols::Provider,
        cancelled: &'a AtomicBool,
        clean: &'a mut bool,
    ) -> Self {
        Self {
            capture,
            budget,
            provider,
            cancelled,
            clean,
            used: false,
        }
    }

    // A failed global admission MUST NOT change an otherwise valid local answer.
    // Reserve selected lowering/regions before allocation, but defer its refusal
    // until the authenticated kernel actually selects Outer.
    pub(super) fn prepare_selected(&mut self, tree: &Parsed) {
        if self.capture.is_ok() && self.budget.retained_facts(tree, self.cancelled).is_err() {
            self.capture = Err(LIMIT);
        }
    }

    pub(super) fn publication(self) -> Option<Arc<workspace_symbols::Capture>> {
        if self.used { self.capture.ok() } else { None }
    }

    pub(super) fn render<'b>(
        &mut self,
        outer: OuterValueLookup<'b>,
        directory: &'b RegionDirectory<'b>,
        result: &'b SnapshotResult,
        current: &SourceSnapshot,
        query: &Query,
        response_id_bound: usize,
    ) -> ParserResult {
        // Every error on this finite path also fits this fixed envelope bound.
        // Keep the existing caller's conservative escaped-ID accounting.
        if response_id_bound
            .checked_add(256)
            .is_none_or(|bytes| bytes > ktlsp_protocol::MAX_PAYLOAD_BYTES)
        {
            return Err(ParserFailure::Protocol(-32803, "limit"));
        }
        let capture = self
            .capture
            .as_ref()
            .map_err(|error| ParserFailure::from(*error))?;
        let snapshot = capture.snapshot.as_ref().ok_or(UNSUPPORTED)?;
        self.used = true;
        self.budget.verify_capture(capture, self.cancelled)?;
        let answer = (|| {
            let mut requester = None;
            for member in &capture.members {
                if let Some((uri, _)) = &member.overlay {
                    self.budget
                        .work(uri.len() + current.document().len(), self.cancelled)
                        .map_err(|_| LIMIT)?;
                    if uri.as_ref() == current.document() && requester.replace(member).is_some() {
                        return Err(UNSUPPORTED.into());
                    }
                }
            }
            let requester = requester.ok_or(UNSUPPORTED)?;
            self.budget
                .work(
                    requester.source.source().len() + current.source().len(),
                    self.cancelled,
                )
                .map_err(|_| LIMIT)?;
            if requester.source.revision() != current.revision()
                || requester.source.source() != current.source()
            {
                return Err(ParserFailure::Protocol(
                    -32801,
                    "requesting source snapshot is stale",
                ));
            }
            let module =
                snapshot.source_module(capture, requester, &mut self.budget, self.cancelled)?;
            // ponytail: eagerly retain complete original module facts per request.
            // Worst-case CST/position reservations can refuse far below1024 files;
            // use a producer-owned cache only if measured workloads justify it.
            // Reserve all vector wrappers/entry metadata before allocation; CST
            // and lowered-record capacities have separate monotonic reservations.
            let count = capture.members.len();
            self.budget
                .storage(
                    count
                        .checked_mul(std::mem::size_of::<SnapshotResult>() + 1024)
                        .ok_or(LIMIT)?,
                )
                .map_err(|_| LIMIT)?;
            let mut results = Vec::new();
            results.try_reserve_exact(count).map_err(|_| LIMIT)?;
            for (index, member) in capture.members.iter().enumerate() {
                if !module.contains(member, &mut self.budget, self.cancelled)?
                    || std::ptr::eq(member, requester)
                {
                    continue;
                }
                self.budget
                    .retained_parse(member.source.source().len(), self.cancelled)
                    .map_err(|_| LIMIT)?;
                let (parsed, clean) = self.provider.parse(&member.source, self.cancelled);
                *self.clean &= clean;
                // A value alongside unproved child closure is NOT permission to
                // launch the next child, build a directory or publish a target.
                if !clean {
                    return Err(ParserFailure::Protocol(
                        -32803,
                        "parser I/O or cleanup failed",
                    ));
                }
                let parsed = parsed.map_err(crate::parser_failure)?;
                let tree = parsed.tree_for(&member.source).map_err(|_| UNSUPPORTED)?;
                self.budget
                    .retained_facts(tree, self.cancelled)
                    .map_err(|_| LIMIT)?;
                results.push((index, parsed));
            }
            let mut lowered = Vec::new();
            lowered
                .try_reserve_exact(results.len())
                .map_err(|_| LIMIT)?;
            for (index, parsed) in &results {
                lowered.push(
                    lower_declarations(parsed, &capture.members[*index].source)
                        .map_err(|_| UNSUPPORTED)?,
                );
            }
            let mut directories = Vec::new();
            directories
                .try_reserve_exact(results.len())
                .map_err(|_| LIMIT)?;
            for (lowered, (index, parsed)) in lowered.iter().zip(&results) {
                directories.push(
                    build_regions(lowered, parsed, &capture.members[*index].source)
                        .map_err(|_| UNSUPPORTED)?,
                );
            }
            let mut files = Vec::new();
            files
                .try_reserve_exact(results.len().checked_add(1).ok_or(LIMIT)?)
                .map_err(|_| LIMIT)?;
            // Keep the request's ORIGINAL raw-URI producer exactly once. The
            // canonical duplicate from Capture is NOT reparsed or substituted.
            files.push((directory, result, current));
            for (directory, (index, parsed)) in directories.iter().zip(&results) {
                files.push((directory, parsed, &capture.members[*index].source));
            }
            let binding = outer.resolve_import(&files).map_err(|_| UNSUPPORTED)?;
            if let Query::Hover { format, .. } = query {
                return render_hover(
                    &binding,
                    current,
                    *format,
                    response_id_bound,
                    &mut self.budget,
                    self.cancelled,
                );
            }
            let target = binding.target().name_range();
            let source = binding.result().snapshot();
            self.budget
                .work(source.source().len(), self.cancelled)
                .map_err(|_| LIMIT)?;
            if workspace_symbols::json_string_bytes(source.document())
                .and_then(|bytes| bytes.checked_add(response_id_bound))
                .and_then(|bytes| bytes.checked_add(256))
                .is_none_or(|bytes| bytes > ktlsp_protocol::MAX_PAYLOAD_BYTES)
            {
                return Err(ParserFailure::Protocol(
                    -32803,
                    "imported definition response limit exceeded",
                ));
            }
            // Reserve escaped URI plus fixed logical JSON-map/key/endpoint
            // storage separately from the 256-byte encoded frame allowance.
            self.budget
                .storage(
                    source
                        .document()
                        .len()
                        .checked_mul(6)
                        .and_then(|bytes| bytes.checked_add(4096))
                        .ok_or(LIMIT)?,
                )
                .map_err(|_| LIMIT)?;
            let positions = structure::positions(source.source(), &[target.start, target.end]);
            let start = positions[0];
            let end = positions[1];
            Ok(json!({"uri":source.document(),"range":{"start":start,"end":end}}))
        })();
        // Revalidate even a refused/negative directory: a late mutation must not
        // turn an earlier match into a prefix or a previous-generation answer.
        self.budget.verify_capture(capture, self.cancelled)?;
        answer
    }
}

fn render_hover(
    binding: &ImportedValueBinding<'_>,
    requester: &SourceSnapshot,
    format: HoverFormat,
    response_id_bound: usize,
    budget: &mut workspace_symbols::Budget,
    cancelled: &AtomicBool,
) -> ParserResult {
    let declaration = binding
        .result()
        .snapshot()
        .source()
        .get(binding.target().range())
        .ok_or(UNSUPPORTED)?;
    // The original binding authenticates BOTH producers: contents belong to the
    // target, but positions must sweep the requester's original reference site.
    let reference = binding.site().range();
    let markdown = matches!(format, HoverFormat::Markdown);
    budget
        .work(declaration.len(), cancelled)
        .map_err(|_| LIMIT)?;
    let escaped = workspace_symbols::json_string_bytes(declaration).ok_or(LIMIT)?;
    let fence = if markdown {
        budget
            .work(declaration.len(), cancelled)
            .map_err(|_| LIMIT)?;
        3.max(
            declaration
                .split(|character| character != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(LIMIT)?,
        )
    } else {
        0
    };
    let extra = if markdown {
        fence
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(8))
            .ok_or(LIMIT)?
    } else {
        0
    };
    let size = declaration.len().checked_add(extra).ok_or(LIMIT)?;
    // Two fence newlines each cost one extra escaped byte. The counted source
    // string includes its quotes; 256 covers keys/kind, range digits and frame.
    let frame = escaped
        .checked_add(extra)
        .and_then(|bytes| bytes.checked_add(if markdown { 2 } else { 0 }))
        .and_then(|bytes| bytes.checked_add(response_id_bound))
        .and_then(|bytes| bytes.checked_add(256))
        .ok_or(LIMIT)?;
    if frame > ktlsp_protocol::MAX_PAYLOAD_BYTES {
        return Err(ParserFailure::Protocol(
            -32803,
            "imported hover response limit exceeded",
        ));
    }
    // Reserve the single owned source value, eventual escaped frame and fixed
    // JSON-map/key storage before allocating either source content or JSON.
    budget
        .storage(
            size.checked_add(frame)
                .and_then(|bytes| bytes.checked_add(4096))
                .ok_or(LIMIT)?,
        )
        .map_err(|_| LIMIT)?;
    budget
        .work(
            size.checked_add(frame)
                .and_then(|bytes| bytes.checked_add(requester.source().len()))
                .ok_or(LIMIT)?,
            cancelled,
        )
        .map_err(|_| LIMIT)?;
    let mut value = String::new();
    value.try_reserve_exact(size).map_err(|_| LIMIT)?;
    if markdown {
        value.extend(std::iter::repeat_n('`', fence));
        value.push_str("kotlin\n");
    }
    value.push_str(declaration);
    if markdown {
        value.push('\n');
        value.extend(std::iter::repeat_n('`', fence));
    }
    let positions = structure::positions(requester.source(), &[reference.start, reference.end]);
    let mut answer = json!({
        "contents":{"kind":if markdown { "markdown" } else { "plaintext" },"value":null},
        "range":{"start":positions[0],"end":positions[1]}
    });
    answer["contents"]["value"] = serde_json::Value::String(value);
    Ok(answer)
}
