//! Finite producer-owned prose for original declaration-name hover.
//!
//! No semantic tags, links, code, annotations, inheritance or reference lookup.
//! The caller owns the original lowering and shares one budget through output.
use crate::{
    Element, Parsed, SnapshotResult, SourceSnapshot,
    lowering::{self, Declaration, LoweredDeclarations, LoweringError},
};
use std::{
    borrow::Cow,
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Lowering(LoweringError),
    Unsupported,
    InvalidOrigin,
    Cancelled,
    Output,
    Allocation,
}
impl From<LoweringError> for Error {
    fn from(error: LoweringError) -> Self {
        if error == LoweringError::Cancelled {
            Self::Cancelled
        } else {
            Self::Lowering(error)
        }
    }
}
type Result<T, E = Error> = std::result::Result<T, E>;

// A checked nonreused operation stamp prevents allocator-address ABA after a
// caller drops a lowering. It authenticates the query, never replaces original
// declaration identity. No allocation, registry or counter reset is involved.
static NEXT_OPERATION: AtomicUsize = AtomicUsize::new(1);
fn next_operation(counter: &AtomicUsize) -> Result<usize> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .map_err(|_| Error::InvalidOrigin)
}

/// Monotonic work and allocation counters shared with the actual lowerer.
/// A budget admits exactly one lowering and one original-name selection.
/// Cancellation observes the caller's existing token; it creates no deadline.
pub struct Budget<'c> {
    inner: lowering::Budget<'c>,
    used: bool,
    selected: bool,
    origin: Option<(*const Parsed, usize)>,
}
impl<'c> Budget<'c> {
    pub fn new(cancelled: &'c dyn Fn() -> bool) -> Self {
        Self {
            inner: lowering::Budget::documentation(cancelled),
            used: false,
            selected: false,
            origin: None,
        }
    }
    /// Charge before scans, comparisons, coordinate conversion or emission.
    pub fn work(&mut self, units: usize) -> Result<()> {
        self.inner.work(units).map_err(Into::into)
    }
    /// Reserve once into a fresh empty string. All retained capacity is charged,
    /// including allocator over-allocation; callers must never grow it implicitly.
    pub fn reserve_string(&mut self, output: &mut String, additional: usize) -> Result<()> {
        self.work(1)?;
        if !output.is_empty() || output.capacity() != 0 {
            return Err(Error::InvalidOrigin);
        }
        self.inner.storage(additional, 0)?;
        output
            .try_reserve_exact(additional)
            .map_err(|_| Error::Allocation)?;
        if let Err(error) = self.inner.storage(0, output.capacity()) {
            *output = String::new();
            return Err(error.into());
        }
        Ok(())
    }
    /// Charge caller-owned storage before requesting it and after observing
    /// actual retained capacity. No counters are refunded.
    pub fn storage(&mut self, requested: usize, retained: usize) -> Result<()> {
        self.work(1)?;
        self.inner.storage(requested, retained).map_err(Into::into)
    }
    /// Check the complete escaped JSON response bound, not just prose bytes.
    pub fn output(&mut self, complete_json_bytes: usize) -> Result<()> {
        self.work(complete_json_bytes)?;
        if complete_json_bytes > 1_048_576 {
            return Err(Error::Output);
        }
        Ok(())
    }
}

/// Eligibility only: inspect the original direct name's declaration prefix.
/// False keeps the old docless operation unchanged; true still requires
/// `lower` and `select` under this same budget, never a raw-source fallback.
pub fn candidate(
    parsed: &Parsed,
    name_token_index: usize,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    let name = candidate_element(parsed, name_token_index, &mut budget.inner)?;
    budget.work(name.kind.len())?;
    if !name.token || name.kind != "IDENTIFIER" {
        return Err(Error::Unsupported);
    }
    let index = name.parent.ok_or(Error::Unsupported)?;
    let declaration = candidate_element(parsed, index, &mut budget.inner)?;
    budget.work(declaration.kind.len())?;
    if declaration.token
        || !matches!(
            declaration.kind.as_str(),
            "CLASS" | "FUN" | "PROPERTY" | "VALUE_PARAMETER" | "TYPE_PARAMETER"
        )
    {
        return Err(Error::Unsupported);
    }
    let mut cursor = index.checked_add(1).ok_or(Error::InvalidOrigin)?;
    while cursor < name_token_index {
        let child = candidate_element(parsed, cursor, &mut budget.inner)?;
        budget.work(child.kind.len())?;
        if child.parent != Some(index)
            || child.subtree_end <= cursor
            || child.subtree_end > declaration.subtree_end
        {
            return Err(Error::InvalidOrigin);
        }
        if !child.token && child.kind == "DOC_COMMENT" {
            return Ok(true);
        }
        if !child.token && child.kind == "MODIFIER_LIST" {
            return Ok(false);
        }
        if !child.token
            || !matches!(
                child.kind.as_str(),
                "WHITE_SPACE" | "EOL_COMMENT" | "BLOCK_COMMENT"
            )
        {
            return Ok(false);
        }
        cursor = child.subtree_end;
    }
    Ok(false)
}

pub fn lower<'a>(
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    budget: &mut Budget<'_>,
) -> Result<LoweredDeclarations<'a>> {
    budget.work(4)?;
    if budget.used {
        return Err(Error::InvalidOrigin);
    }
    budget.used = true;
    let mut lowered = lowering::lower_with_budget(result, current, &mut budget.inner)?;
    let operation = next_operation(&NEXT_OPERATION)?;
    lowered.documentation_operation = Some(operation);
    budget.origin = Some((lowered.parsed(), operation));
    Ok(lowered)
}

/// Authenticated prose borrows the caller-owned original declaration. Only the
/// necessarily decoded prose is owned; no tree/source/declaration copy is made.
#[derive(Debug)]
pub struct Attached<'a> {
    declaration: &'a Declaration<'a>,
    documentation: Option<Range<usize>>,
    prose: Option<Cow<'a, str>>,
}
impl<'a> Attached<'a> {
    pub fn declaration(&self) -> &'a Declaration<'a> {
        self.declaration
    }
    pub fn prose(&self) -> Option<&str> {
        self.prose.as_deref()
    }
    pub fn documentation_range(&self) -> Option<Range<usize>> {
        self.documentation.clone()
    }
    pub fn source_range(&self) -> Range<usize> {
        self.documentation.as_ref().map_or_else(
            || self.declaration.range(),
            |doc| doc.end..self.declaration.range().end,
        )
    }
}

pub fn select<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    original_offset: usize,
    budget: &mut Budget<'_>,
) -> Result<Attached<'a>> {
    budget.work(1)?;
    if budget.selected
        || !budget.origin.is_some_and(|(parsed, operation)| {
            std::ptr::eq(parsed, lowered.parsed())
                && lowered.documentation_operation == Some(operation)
        })
    {
        return Err(Error::InvalidOrigin);
    }
    budget.selected = true;
    for source in [current.source(), result.snapshot().source()] {
        budget.work(source.len())?;
    }
    for document in [current.document(), result.snapshot().document()] {
        budget.work(document.len())?;
    }
    budget.work(4)?;
    let parsed = result.tree_for(current).map_err(|_| Error::InvalidOrigin)?;
    if !std::ptr::eq(parsed, lowered.parsed())
        || !std::ptr::eq(result.snapshot(), lowered.snapshot())
    {
        return Err(Error::InvalidOrigin);
    }
    budget.work(1)?;
    if !parsed.source().is_char_boundary(original_offset) {
        return Err(Error::Unsupported);
    }
    let mut target = None;
    for declaration in lowered.declarations() {
        budget.work(4)?;
        let name = declaration.name_range();
        if name.contains(&original_offset) {
            if target.is_some() || declaration.incomplete() {
                return Err(Error::Unsupported);
            }
            target = Some(declaration);
        }
    }
    let declaration = target.ok_or(Error::Unsupported)?;
    let index = declaration.cst_index();
    budget.work(1)?;
    let node = parsed.elements().get(index).ok_or(Error::InvalidOrigin)?;
    let mut doc = None;
    let mut cursor = index.checked_add(1).ok_or(Error::InvalidOrigin)?;
    while cursor < node.subtree_end {
        let child = get(parsed, cursor, &mut budget.inner)?;
        if child.parent != Some(index) {
            return Err(Error::InvalidOrigin);
        }
        budget.work(child.kind.len())?;
        if child.token
            && matches!(
                child.kind.as_str(),
                "WHITE_SPACE" | "EOL_COMMENT" | "BLOCK_COMMENT"
            )
        {
            cursor = child.subtree_end;
            continue;
        }
        if !child.token && child.kind == "DOC_COMMENT" {
            if doc.replace(cursor).is_some() {
                return Err(Error::Unsupported);
            }
            cursor = child.subtree_end;
            continue;
        }
        // A modifier list is the final prefix structure: docs within or after
        // it cannot acquire ownership. The complete lowerer already proved it.
        break;
    }
    let Some(doc_index) = doc else {
        return Ok(Attached {
            declaration,
            documentation: None,
            prose: None,
        });
    };
    let doc_node = get(parsed, doc_index, &mut budget.inner)?;
    let range = doc_node.start..doc_node.end;
    validate_doc(parsed, doc_index, &mut budget.inner)?;
    let mut length = 0usize;
    let mut retained_start = None;
    let mut retained_end = 0;
    for i in doc_index + 1..doc_node.subtree_end {
        let leaf = get(parsed, i, &mut budget.inner)?;
        if retained_leaf(leaf, &mut budget.inner)? {
            length = length
                .checked_add(leaf.end - leaf.start)
                .ok_or(Error::Output)?;
            retained_start.get_or_insert(leaf.start);
            retained_end = leaf.end;
        }
    }
    let retained_start = retained_start.ok_or(Error::Unsupported)?;
    if retained_end.checked_sub(retained_start) == Some(length) {
        budget.work(2)?;
        let prose = parsed
            .source()
            .get(retained_start..retained_end)
            .ok_or(Error::InvalidOrigin)?;
        budget.work(prose.len())?;
        if !prose.chars().any(|ch| !ch.is_whitespace()) {
            return Err(Error::Unsupported);
        }
        return Ok(Attached {
            declaration,
            documentation: Some(range),
            prose: Some(Cow::Borrowed(prose)),
        });
    }
    let mut prose = String::new();
    budget.reserve_string(&mut prose, length)?;
    let mut content = false;
    for i in doc_index + 1..doc_node.subtree_end {
        let leaf = get(parsed, i, &mut budget.inner)?;
        if retained_leaf(leaf, &mut budget.inner)? {
            let text = text(parsed, leaf, &mut budget.inner)?;
            budget.work(text.len())?;
            content |= text.chars().any(|ch| !ch.is_whitespace());
            budget.work(text.len())?;
            if prose.capacity() - prose.len() < text.len() {
                return Err(Error::Allocation);
            }
            prose.push_str(text);
        }
    }
    if !content || prose.len() != length {
        return Err(Error::Unsupported);
    }
    Ok(Attached {
        declaration,
        documentation: Some(range),
        prose: Some(Cow::Owned(prose)),
    })
}
// Eligibility performs at most six kind comparisons (literal bytes <=64) and
// twelve fixed checks, not the full sixteen-comparison documentation visit.
// In particular, ordinary leading trivia must reach the legacy docless path.
fn candidate_element<'a>(
    parsed: &'a Parsed,
    index: usize,
    budget: &mut lowering::Budget<'_>,
) -> std::result::Result<&'a Element, LoweringError> {
    budget.work(1)?;
    let element = parsed
        .elements()
        .get(index)
        .ok_or(LoweringError::InvalidCst {
            element: Some(index),
        })?;
    let work = element
        .kind
        .len()
        .checked_mul(6)
        .and_then(|n| n.checked_add(76))
        .ok_or(LoweringError::Limit(lowering::LoweringLimit::Work))?;
    budget.work(work)?;
    Ok(element)
}

fn get<'a>(
    parsed: &'a Parsed,
    index: usize,
    budget: &mut lowering::Budget<'_>,
) -> std::result::Result<&'a Element, LoweringError> {
    budget.work(1)?;
    let element = parsed
        .elements()
        .get(index)
        .ok_or(LoweringError::InvalidCst {
            element: Some(index),
        })?;
    // Each visit has a closed set of at most sixteen kind comparisons plus
    // fixed index/range/parent checks. Pay their byte bound before inspecting.
    let work = element
        .kind
        .len()
        .checked_mul(16)
        .and_then(|n| n.checked_add(256))
        .ok_or(LoweringError::Limit(lowering::LoweringLimit::Work))?;
    budget.work(work)?;
    Ok(element)
}
fn text<'a>(
    parsed: &'a Parsed,
    leaf: &Element,
    budget: &mut lowering::Budget<'_>,
) -> std::result::Result<&'a str, LoweringError> {
    budget.work(2)?;
    parsed
        .source()
        .get(leaf.start..leaf.end)
        .ok_or(LoweringError::InvalidCst { element: None })
}
fn retained_leaf(
    leaf: &Element,
    budget: &mut lowering::Budget<'_>,
) -> std::result::Result<bool, LoweringError> {
    budget.work(leaf.kind.len())?;
    Ok(leaf.token
        && matches!(
            leaf.kind.as_str(),
            "KDOC_TEXT" | "WHITE_SPACE" | "KDOC_LPAR" | "KDOC_RPAR"
        ))
}
fn refuse(index: usize) -> LoweringError {
    LoweringError::Unsupported { element: index }
}

pub(crate) fn validate_prefix(
    parsed: &Parsed,
    index: usize,
    budget: &mut lowering::Budget<'_>,
) -> std::result::Result<(), LoweringError> {
    let node = get(parsed, index, budget)?;
    budget.work(node.kind.len())?;
    if node.kind == "DOC_COMMENT" {
        return validate_doc(parsed, index, budget);
    }
    if node.kind != "MODIFIER_LIST" || node.token {
        return Err(refuse(index));
    }
    // ponytail: plain visibility/finality modifiers only; annotation arguments
    // require an independent finite header proof before admission.
    let mut modifiers = 0usize;
    let mut visibility = false;
    let mut finality = false;
    for i in index + 1..node.subtree_end {
        let leaf = get(parsed, i, budget)?;
        budget.work(leaf.kind.len())?;
        if !leaf.token || leaf.parent != Some(index) {
            return Err(refuse(i));
        }
        let expected = match leaf.kind.as_str() {
            "PUBLIC_KEYWORD" => "public",
            "PRIVATE_KEYWORD" => "private",
            "PROTECTED_KEYWORD" => "protected",
            "INTERNAL_KEYWORD" => "internal",
            "FINAL_KEYWORD" => "final",
            "OPEN_KEYWORD" => "open",
            "WHITE_SPACE" | "EOL_COMMENT" | "BLOCK_COMMENT" => continue,
            _ => return Err(refuse(i)),
        };
        let group = if matches!(leaf.kind.as_str(), "FINAL_KEYWORD" | "OPEN_KEYWORD") {
            &mut finality
        } else {
            &mut visibility
        };
        if *group {
            return Err(refuse(i));
        }
        *group = true;
        let raw = text(parsed, leaf, budget)?;
        budget.work(raw.len() + expected.len())?;
        if raw != expected {
            return Err(refuse(i));
        }
        modifiers += 1;
    }
    if modifiers == 0 {
        return Err(refuse(index));
    }
    Ok(())
}

fn validate_doc(
    parsed: &Parsed,
    index: usize,
    budget: &mut lowering::Budget<'_>,
) -> std::result::Result<(), LoweringError> {
    let node = get(parsed, index, budget)?;
    if node.token {
        return Err(refuse(index));
    }
    let mut section = None;
    let mut start = false;
    let mut end = false;
    for i in index + 1..node.subtree_end {
        let leaf = get(parsed, i, budget)?;
        budget.work(leaf.kind.len())?;
        if !leaf.token {
            if leaf.kind != "KDOC_SECTION"
                || leaf.parent != Some(index)
                || section.is_some()
                || !start
                || end
            {
                return Err(refuse(i));
            }
            section = Some(i);
            continue;
        }
        if leaf.parent != Some(index) && leaf.parent != section {
            return Err(refuse(i));
        }
        let raw = text(parsed, leaf, budget)?;
        budget.work(raw.len())?;
        match leaf.kind.as_str() {
            "KDOC_START"
                if !start
                    && !end
                    && section.is_none()
                    && leaf.parent == Some(index)
                    && leaf.start == node.start
                    && raw == "/**" =>
            {
                start = true
            }
            "KDOC_END"
                if start
                    && !end
                    && section.is_some()
                    && leaf.parent == Some(index)
                    && leaf.end == node.end =>
            {
                let stars = raw.strip_suffix('/').ok_or_else(|| refuse(i))?;
                if stars.is_empty() || !stars.bytes().all(|byte| byte == b'*') {
                    return Err(refuse(i));
                }
                end = true;
            }
            "KDOC_LEADING_ASTERISK" if start && !end => {
                if raw.is_empty() || !raw.bytes().all(|byte| byte == b'*') {
                    return Err(refuse(i));
                }
            }
            "WHITE_SPACE" if start && !end => {
                if !raw.chars().all(char::is_whitespace) {
                    return Err(refuse(i));
                }
            }
            "KDOC_TEXT" | "KDOC_LPAR" | "KDOC_RPAR" if start && !end && leaf.parent == section => {
                // Empty code fences can be emitted as TEXT by the producer.
                // Refuse code fences, lexical/external/reference/image links
                // and nested comment delimiters, including TEXT-only forms.
                budget.work(raw.len().checked_mul(6).ok_or_else(|| refuse(i))?)?;
                if raw.contains('`')
                    || raw.contains("~~~")
                    || raw.contains('[')
                    || raw.contains(']')
                    || raw.contains("/*")
                    || raw.contains("*/")
                    || (leaf.kind == "KDOC_LPAR" && raw != "(")
                    || (leaf.kind == "KDOC_RPAR" && raw != ")")
                {
                    return Err(refuse(i));
                }
            }
            _ => return Err(refuse(i)),
        }
    }
    if !start || !end || section.is_none() {
        return Err(refuse(index));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileKind, Parsed};
    use std::{cell::Cell, sync::Arc};

    // Synthetic defensive/accounting fixture, never producer acceptance.
    fn snapshot() -> SnapshotResult {
        fn e(
            kind: &str,
            start: usize,
            end: usize,
            parent: Option<usize>,
            subtree_end: usize,
            token: bool,
        ) -> Element {
            Element {
                kind: kind.into(),
                start,
                end,
                parent,
                subtree_end,
                token,
            }
        }
        let source: Arc<str> = Arc::from("/** prose */fun named(){}");
        SnapshotResult {
            snapshot: SourceSnapshot::new("synthetic", 1, FileKind::Source, source.clone())
                .unwrap(),
            parsed: Parsed {
                source,
                diagnostics: vec![],
                elements: vec![
                    e("KT_FILE", 0, 25, None, 16, false),
                    e("FUN", 0, 25, Some(0), 16, false),
                    e("DOC_COMMENT", 0, 12, Some(1), 7, false),
                    e("KDOC_START", 0, 3, Some(2), 4, true),
                    e("KDOC_SECTION", 3, 10, Some(2), 6, false),
                    e("KDOC_TEXT", 3, 10, Some(4), 6, true),
                    e("KDOC_END", 10, 12, Some(2), 7, true),
                    e("FUN_KEYWORD", 12, 15, Some(1), 8, true),
                    e("WHITE_SPACE", 15, 16, Some(1), 9, true),
                    e("IDENTIFIER", 16, 21, Some(1), 10, true),
                    e("VALUE_PARAMETER_LIST", 21, 23, Some(1), 13, false),
                    e("LPAR", 21, 22, Some(10), 12, true),
                    e("RPAR", 22, 23, Some(10), 13, true),
                    e("BLOCK", 23, 25, Some(1), 16, false),
                    e("LBRACE", 23, 24, Some(13), 15, true),
                    e("RBRACE", 24, 25, Some(13), 16, true),
                ],
            },
        }
    }
    fn operation(result: &SnapshotResult, budget: &mut Budget<'_>) -> Result<()> {
        assert!(candidate(
            result.tree_for(result.snapshot()).unwrap(),
            9,
            budget
        )?);
        let lowered = lower(result, result.snapshot(), budget)?;
        let attached = select(&lowered, result, result.snapshot(), 16, budget)?;
        assert_eq!(attached.prose(), Some(" prose "));
        assert!(matches!(attached.prose.as_ref(), Some(Cow::Borrowed(_))));
        assert_eq!(attached.documentation_range(), Some(0..12));
        assert_eq!(attached.source_range(), 12..25);
        assert!(std::ptr::eq(
            attached.declaration(),
            &lowered.declarations()[0]
        ));
        Ok(())
    }
    #[test]
    fn one_budget_covers_original_lowering_selection_decode_and_late_output() {
        let result = snapshot();
        let mut measured = Budget::new(&|| false);
        operation(&result, &mut measured).unwrap();
        let (work, requested, retained) = measured.inner.usage();
        assert!(requested >= std::mem::size_of::<Declaration<'_>>());
        assert!(retained >= requested);
        let mut exact = Budget::new(&|| false);
        exact.work(2_097_152 - work).unwrap();
        operation(&result, &mut exact).unwrap();
        assert!(
            exact.work(1).is_err(),
            "render cannot renew the lowering allowance"
        );
        let mut short = Budget::new(&|| false);
        short.work(2_097_152 - work + 1).unwrap();
        assert!(operation(&result, &mut short).is_err());
        assert!(measured.work(usize::MAX).is_err());
        assert!(lower(&result, result.snapshot(), &mut measured).is_err());
        assert!(
            select(
                &lower(&result, result.snapshot(), &mut Budget::new(&|| false)).unwrap(),
                &result,
                result.snapshot(),
                16,
                &mut measured
            )
            .is_err()
        );
    }
    #[test]
    fn actual_capacity_and_late_growth_cannot_hide_in_existing_buffers() {
        let mut budget = Budget::new(&|| false);
        let mut first = String::new();
        budget.reserve_string(&mut first, 32).unwrap();
        assert!(first.capacity() >= 32);
        let retained = budget.inner.usage().2;
        assert_eq!(retained, first.capacity());
        assert!(budget.reserve_string(&mut first, 0).is_err());
        let mut foreign = String::with_capacity(131_073);
        assert!(budget.reserve_string(&mut foreign, 0).is_err());
        budget.storage(0, 131_072 - retained).unwrap();
        let mut late = String::new();
        assert!(matches!(
            budget.reserve_string(&mut late, 1),
            Err(Error::Lowering(LoweringError::Limit(
                lowering::LoweringLimit::RetainedCapacity
            )))
        ));
        assert_eq!(
            late.capacity(),
            0,
            "refused capacity must not remain retained"
        );
        let mut requested = Budget::new(&|| false);
        requested.storage(131_072, 0).unwrap();
        assert!(requested.reserve_string(&mut String::new(), 1).is_err());
        assert!(requested.storage(usize::MAX, 0).is_err());
        let mut retained = Budget::new(&|| false);
        retained.storage(0, 131_072).unwrap();
        assert!(retained.storage(0, 1).is_err());
    }
    #[test]
    fn output_and_existing_cancellation_have_no_new_allowance_or_deadline() {
        let cancelled = Cell::new(false);
        let observer = || cancelled.get();
        let mut budget = Budget::new(&observer);
        let result = snapshot();
        let lower = lower(&result, result.snapshot(), &mut budget).unwrap();
        cancelled.set(true);
        assert_eq!(
            select(&lower, &result, result.snapshot(), 16, &mut budget).unwrap_err(),
            Error::Cancelled
        );
        assert!(matches!(
            budget.reserve_string(&mut String::new(), 1),
            Err(Error::Cancelled)
        ));
        assert!(matches!(budget.output(1), Err(Error::Cancelled)));
        let mut exact = Budget::new(&|| false);
        exact.output(1_048_576).unwrap();
        let mut over = Budget::new(&|| false);
        assert_eq!(over.output(1_048_577), Err(Error::Output));
        assert!(over.output(usize::MAX).is_err());
    }
    #[test]
    fn operation_stamp_cannot_wrap_or_authorize_a_reallocated_foreign_lowering() {
        let counter = AtomicUsize::new(usize::MAX - 1);
        assert_eq!(next_operation(&counter), Ok(usize::MAX - 1));
        assert_eq!(next_operation(&counter), Err(Error::InvalidOrigin));
        assert_eq!(next_operation(&counter), Err(Error::InvalidOrigin));
        let result = snapshot();
        let mut original = Budget::new(&|| false);
        let first = lower(&result, result.snapshot(), &mut original).unwrap();
        let stamp = first.documentation_operation;
        drop(first);
        let mut foreign_budget = Budget::new(&|| false);
        let foreign = lower(&result, result.snapshot(), &mut foreign_budget).unwrap();
        assert_ne!(foreign.documentation_operation, stamp);
        assert_eq!(
            select(&foreign, &result, result.snapshot(), 16, &mut original).unwrap_err(),
            Error::InvalidOrigin
        );
        assert_eq!(
            select(
                &foreign,
                &result,
                result.snapshot(),
                16,
                &mut foreign_budget
            )
            .unwrap()
            .prose(),
            Some(" prose ")
        );
    }
    #[test]
    fn decoration_decode_owns_only_discontiguous_prose_under_the_same_budget() {
        let mut result = snapshot();
        let source: Arc<str> = Arc::from("/** *rose */fun named(){}");
        result.snapshot.source = source.clone();
        result.parsed.source = source;
        // Synthetic producer-kind boundary, not an actual lexer/ownership claim.
        for leaf in &mut result.parsed.elements {
            if leaf.subtree_end >= 6 {
                leaf.subtree_end += 2;
            }
            if let Some(parent) = &mut leaf.parent {
                if *parent >= 6 {
                    *parent += 2;
                }
            }
        }
        result.parsed.elements[5].end = 4;
        result.parsed.elements[5].subtree_end = 6;
        result.parsed.elements.insert(
            6,
            Element {
                kind: "KDOC_LEADING_ASTERISK".into(),
                start: 4,
                end: 5,
                parent: Some(4),
                subtree_end: 7,
                token: true,
            },
        );
        result.parsed.elements.insert(
            7,
            Element {
                kind: "KDOC_TEXT".into(),
                start: 5,
                end: 10,
                parent: Some(4),
                subtree_end: 8,
                token: true,
            },
        );
        let operation = |budget: &mut Budget<'_>| -> Result<()> {
            let lower = lower(&result, result.snapshot(), budget)?;
            let attached = select(&lower, &result, result.snapshot(), 16, budget)?;
            assert_eq!(attached.prose(), Some(" rose "));
            assert!(matches!(attached.prose.as_ref(), Some(Cow::Owned(_))));
            assert!(budget.inner.usage().2 >= std::mem::size_of::<Declaration<'_>>() + 6);
            Ok(())
        };
        let mut measured = Budget::new(&|| false);
        operation(&mut measured).unwrap();
        let mut exact = Budget::new(&|| false);
        exact.work(2_097_152 - measured.inner.usage().0).unwrap();
        operation(&mut exact).unwrap();
        assert!(exact.work(1).is_err());
        let mut short = Budget::new(&|| false);
        short
            .work(2_097_152 - measured.inner.usage().0 + 1)
            .unwrap();
        assert!(operation(&mut short).is_err());
    }
}
