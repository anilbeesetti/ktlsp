//! Snapshot-local, same-body local-value binding; not general Kotlin resolution.
use crate::{
    Element, Parsed, SnapshotResult, SourceSnapshot,
    lowering::{
        Declaration, ReferenceBudget, ReferenceSite, ReferenceSiteError, authenticated_tree,
        reference_site_with_budget,
    },
    structural_regions::{Region, RegionDirectory},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBindingError {
    InvalidOrigin,
    StaleSnapshot,
    UnsupportedContext,
    UnsupportedSpelling,
    AmbiguousLocal,
    NoEligibleLocal,
    Resource,
}
impl std::fmt::Display for LocalBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "local value binding rejected request: {self:?}")
    }
}
impl std::error::Error for LocalBindingError {}
impl From<ReferenceSiteError> for LocalBindingError {
    fn from(error: ReferenceSiteError) -> Self {
        match error {
            ReferenceSiteError::InvalidOrigin => Self::InvalidOrigin,
            ReferenceSiteError::StaleSnapshot => Self::StaleSnapshot,
            ReferenceSiteError::UnsupportedContext => Self::UnsupportedContext,
            ReferenceSiteError::Resource => Self::Resource,
        }
    }
}
type Result<T, E = LocalBindingError> = std::result::Result<T, E>;

#[derive(Debug)]
pub struct LocalValueBinding<'a> {
    site: &'a ReferenceSite<'a>,
    body: &'a Region<'a>,
    target: &'a Declaration<'a>,
}
impl<'a> LocalValueBinding<'a> {
    pub fn site(&self) -> &'a ReferenceSite<'a> {
        self.site
    }
    pub fn body(&self) -> &'a Region<'a> {
        self.body
    }
    pub fn target(&self) -> &'a Declaration<'a> {
        self.target
    }
}

// A forward-only walk of the complete ordinary function, once per query. All
// element visits, kind comparisons, snapshot checks and name bytes share b.
struct BodyCheck<'a, 'b> {
    parsed: &'a Parsed,
    cursor: usize,
    b: &'b mut ReferenceBudget,
}
impl<'a> BodyCheck<'a, '_> {
    fn peek(&mut self) -> Result<&'a Element> {
        Ok(self.b.element(self.parsed, self.cursor)?)
    }
    fn advance(&mut self) -> Result<()> {
        self.cursor = self
            .cursor
            .checked_add(1)
            .ok_or(LocalBindingError::Resource)?;
        Ok(())
    }
    fn trivia(&mut self, parent: usize, end: usize) -> Result<()> {
        while self.cursor < end {
            let e = self.peek()?;
            let trivia = self.b.equal(&e.kind, "WHITE_SPACE")?
                || self.b.equal(&e.kind, "EOL_COMMENT")?
                || self.b.equal(&e.kind, "BLOCK_COMMENT")?;
            if !trivia || !e.token || e.parent != Some(parent) {
                break;
            }
            self.advance()?;
        }
        Ok(())
    }
    fn expect(
        &mut self,
        kind: &str,
        token: bool,
        parent: usize,
        end: usize,
    ) -> Result<(usize, &'a Element)> {
        self.trivia(parent, end)?;
        if self.cursor >= end {
            return Err(LocalBindingError::UnsupportedContext);
        }
        let index = self.cursor;
        let e = self.peek()?;
        if e.token != token || e.parent != Some(parent) || !self.b.equal(&e.kind, kind)? {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.advance()?;
        Ok((index, e))
    }
    fn finish(&mut self, parent: usize, end: usize) -> Result<()> {
        self.trivia(parent, end)?;
        if self.cursor != end {
            return Err(LocalBindingError::UnsupportedContext);
        }
        Ok(())
    }
    fn property(&mut self, body: usize, end: usize) -> Result<()> {
        let (index, node) = self.expect("PROPERTY", false, body, end)?;
        self.expect("VAL_KEYWORD", true, index, node.subtree_end)?;
        self.expect("IDENTIFIER", true, index, node.subtree_end)?;
        self.trivia(index, node.subtree_end)?;
        let colon = if self.cursor < node.subtree_end {
            let next = self.peek()?;
            self.b.equal(&next.kind, "COLON")?
        } else {
            false
        };
        if colon {
            self.expect("COLON", true, index, node.subtree_end)?;
            let (_, annotation) = self.expect("TYPE_REFERENCE", false, index, node.subtree_end)?;
            // The authenticated lowerer already checked the exact annotation
            // subtree; do not introduce a second type grammar in the binder.
            self.cursor = annotation.subtree_end;
        }
        self.expect("EQ", true, index, node.subtree_end)?;
        self.trivia(index, node.subtree_end)?;
        let expression_index = self.cursor;
        let expression = self.peek()?;
        if expression.token || expression.parent != Some(index) {
            return Err(LocalBindingError::UnsupportedContext);
        }
        let reference = self.b.equal(&expression.kind, "REFERENCE_EXPRESSION")?;
        if !reference && !self.b.equal(&expression.kind, "INTEGER_CONSTANT")? {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.advance()?;
        let (_, token) = self.expect(
            if reference {
                "IDENTIFIER"
            } else {
                "INTEGER_LITERAL"
            },
            true,
            expression_index,
            expression.subtree_end,
        )?;
        if reference && (token.start != expression.start || token.end != expression.end) {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.finish(expression_index, expression.subtree_end)?;
        self.finish(index, node.subtree_end)
    }
    fn function(&mut self, declaration: &Declaration<'_>, body_index: usize) -> Result<()> {
        let function_index = declaration.cst_index();
        let function = self.b.element(self.parsed, function_index)?;
        if function.token || function.parent != Some(0) || declaration.incomplete() {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.cursor = function_index
            .checked_add(1)
            .ok_or(LocalBindingError::Resource)?;
        self.expect("FUN_KEYWORD", true, function_index, function.subtree_end)?;
        self.expect("IDENTIFIER", true, function_index, function.subtree_end)?;
        let (parameters_index, parameters) = self.expect(
            "VALUE_PARAMETER_LIST",
            false,
            function_index,
            function.subtree_end,
        )?;
        self.expect("LPAR", true, parameters_index, parameters.subtree_end)?;
        self.expect("RPAR", true, parameters_index, parameters.subtree_end)?;
        self.finish(parameters_index, parameters.subtree_end)?;
        let (checked_body_index, body) =
            self.expect("BLOCK", false, function_index, function.subtree_end)?;
        if checked_body_index != body_index {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.expect("LBRACE", true, body_index, body.subtree_end)?;
        loop {
            self.trivia(body_index, body.subtree_end)?;
            if self.cursor >= body.subtree_end {
                return Err(LocalBindingError::UnsupportedContext);
            }
            let e = self.peek()?;
            if e.token && self.b.equal(&e.kind, "RBRACE")? {
                break;
            }
            if e.token && e.parent == Some(body_index) && self.b.equal(&e.kind, "SEMICOLON")? {
                self.advance()?;
            } else {
                self.property(body_index, body.subtree_end)?;
            }
        }
        self.expect("RBRACE", true, body_index, body.subtree_end)?;
        self.finish(body_index, body.subtree_end)?;
        self.finish(function_index, function.subtree_end)
    }
}

// Only called on an authenticated lowerer's tree: USER_TYPE references belong
// exclusively to its exact retained annotation shape, never to value syntax.
fn annotation_reference(
    parsed: &Parsed,
    element: &Element,
    b: &mut ReferenceBudget,
) -> Result<bool> {
    let parent = b.element(
        parsed,
        element
            .parent
            .ok_or(LocalBindingError::UnsupportedContext)?,
    )?;
    Ok(!parent.token && b.equal(&parent.kind, "USER_TYPE")?)
}

/// Return the unique earlier complete direct local val, preserving original identity.
/// A refusal is not a claim that a reference is globally unresolved. Recheck the
/// authoritative open-document incarnation under its synchronization before use.
pub fn bind_local_value<'a>(
    directory: &'a RegionDirectory<'a>,
    site: &'a ReferenceSite<'a>,
    current: &SourceSnapshot,
) -> Result<LocalValueBinding<'a>> {
    bind_with_budget(directory, site, current, &mut ReferenceBudget::default())
}
pub(super) fn bind_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    site: &'a ReferenceSite<'a>,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<LocalValueBinding<'a>> {
    if !std::ptr::eq(directory.lowered(), site.origin())
        || !std::ptr::eq(directory.snapshot(), site.origin().snapshot())
    {
        return Err(LocalBindingError::InvalidOrigin);
    }
    let parsed = site.tree(current, b)?;
    let spelling = b
        .spelling(site.raw_spelling())?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    BodyCheck {
        parsed,
        cursor: 0,
        b,
    }
    .function(site.function(), site.body_index())?;
    let (body_ordinal, body) = find_body(directory, site.body_index(), b)?;
    let target = unique_local(directory, body_ordinal, site.body_index(), spelling, b)?;
    if std::ptr::eq(target, site.containing_property())
        || target.cst_index() >= site.containing_property().cst_index()
        || target.range().end > site.range().start
    {
        return Err(LocalBindingError::NoEligibleLocal);
    }
    Ok(LocalValueBinding { site, body, target })
}

fn find_body<'a>(
    directory: &'a RegionDirectory<'a>,
    body_index: usize,
    b: &mut ReferenceBudget,
) -> Result<(usize, &'a Region<'a>)> {
    let mut body = None;
    for (ordinal, region) in directory.regions().iter().enumerate() {
        b.charge(1)?;
        if region.cst_index() == body_index {
            body = Some((ordinal, region));
        }
    }
    body.ok_or(LocalBindingError::UnsupportedContext)
}

fn unique_local<'a>(
    directory: &'a RegionDirectory<'a>,
    body_ordinal: usize,
    body_index: usize,
    spelling: &str,
    b: &mut ReferenceBudget,
) -> Result<&'a Declaration<'a>> {
    let mut target = None;
    let mut matches = 0usize;
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(2)?;
        let membership = directory
            .memberships()
            .get(ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        if membership.region() != body_ordinal {
            continue;
        }
        if declaration.cst_parent_index() != Some(body_index)
            || !b.equal(declaration.kind(), "PROPERTY")?
            || declaration.incomplete()
        {
            return Err(LocalBindingError::UnsupportedContext);
        }
        let name = b
            .spelling(declaration.raw_name())?
            .ok_or(LocalBindingError::UnsupportedSpelling)?;
        if b.equal(name, spelling)? {
            matches = matches.checked_add(1).ok_or(LocalBindingError::Resource)?;
            target = Some(declaration);
        }
    }
    if matches > 1 {
        return Err(LocalBindingError::AmbiguousLocal);
    }
    target.ok_or(LocalBindingError::NoEligibleLocal)
}

/// Complete occurrences for one original declaration in the admitted same body.
/// The declaration is retained even when no eligible initializer refers to it.
/// This is not a document/workspace references result or cross-revision identity.
#[derive(Debug)]
pub struct LocalValueOccurrences<'a> {
    directory: &'a RegionDirectory<'a>,
    target: &'a Declaration<'a>,
    body: &'a Region<'a>,
    occurrences: Vec<ReferenceSite<'a>>,
}
impl<'a> LocalValueOccurrences<'a> {
    pub fn directory(&self) -> &'a RegionDirectory<'a> {
        self.directory
    }
    pub fn target(&self) -> &'a Declaration<'a> {
        self.target
    }
    pub fn body(&self) -> &'a Region<'a> {
        self.body
    }
    /// Original whole-initializer occurrences, in source order; excludes declaration.
    pub fn occurrences(&self) -> &[ReferenceSite<'a>] {
        &self.occurrences
    }
}

/// Authenticate a target and collect its complete admitted same-body occurrences.
/// All errors discard the result. Self/forward references do not bind to this
/// target, nor does a nonmatching name imply global unresolvedness.
/// Recheck the authoritative open-document incarnation before publication.
pub fn local_value_occurrences<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    target: &'a Declaration<'a>,
    current: &SourceSnapshot,
) -> Result<LocalValueOccurrences<'a>> {
    occurrences_with_budget(
        directory,
        result,
        target,
        current,
        &mut ReferenceBudget::default(),
    )
}

const MAX_OCCURRENCES: usize = 1_024;
const OCCURRENCE_BYTES: usize = 131_072;

fn occurrence_storage(count: usize) -> Result<()> {
    if count > MAX_OCCURRENCES
        || count
            .checked_mul(std::mem::size_of::<ReferenceSite<'_>>())
            .ok_or(LocalBindingError::Resource)?
            > OCCURRENCE_BYTES
    {
        return Err(LocalBindingError::Resource);
    }
    Ok(())
}

pub(super) fn occurrences_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    target: &'a Declaration<'a>,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<LocalValueOccurrences<'a>> {
    if !std::ptr::eq(directory.snapshot(), directory.lowered().snapshot()) {
        return Err(LocalBindingError::InvalidOrigin);
    }
    let parsed = authenticated_tree(directory.lowered(), result, current, b)?;
    let mut target_ordinal = None;
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(1)?;
        if std::ptr::eq(declaration, target) {
            target_ordinal = Some(ordinal);
        }
    }
    let target_ordinal = target_ordinal.ok_or(LocalBindingError::InvalidOrigin)?;
    let body_index = target
        .cst_parent_index()
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let (body_ordinal, body) = find_body(directory, body_index, b)?;
    b.charge(2)?;
    if directory
        .memberships()
        .get(target_ordinal)
        .ok_or(LocalBindingError::InvalidOrigin)?
        .region()
        != body_ordinal
    {
        return Err(LocalBindingError::UnsupportedContext);
    }
    let function = target
        .owner()
        .and_then(|ordinal| directory.lowered().declarations().get(ordinal))
        .ok_or(LocalBindingError::UnsupportedContext)?;
    BodyCheck {
        parsed,
        cursor: 0,
        b,
    }
    .function(function, body_index)?;
    let spelling = b
        .spelling(target.raw_name())?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    let unique = unique_local(directory, body_ordinal, body_index, spelling, b)?;
    if !std::ptr::eq(unique, target) {
        return Err(LocalBindingError::InvalidOrigin);
    }

    // Count before allocation; at most one reference initializer per direct val.
    let mut count = 0usize;
    for index in body_index..body.subtree_end() {
        let element = b.element(parsed, index)?;
        if !element.token
            && b.equal(&element.kind, "REFERENCE_EXPRESSION")?
            && !annotation_reference(parsed, element, b)?
        {
            count = count.checked_add(1).ok_or(LocalBindingError::Resource)?;
        }
    }
    occurrence_storage(count)?;
    b.charge(count)?;
    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(count)
        .map_err(|_| LocalBindingError::Resource)?;
    occurrence_storage(occurrences.capacity())?;

    // ponytail: repeated authentication/binding scans are quadratic (including
    // snapshot bytes); one shared work ceiling bounds them. Add a validated-body
    // binding index only if real consumers need larger admitted bodies.
    for index in body_index..body.subtree_end() {
        let element = b.element(parsed, index)?;
        if element.token
            || !b.equal(&element.kind, "REFERENCE_EXPRESSION")?
            || annotation_reference(parsed, element, b)?
        {
            continue;
        }
        let site = reference_site_with_budget(directory.lowered(), result, current, index, b)?;
        let name = b
            .spelling(site.raw_spelling())?
            .ok_or(LocalBindingError::UnsupportedSpelling)?;
        if !b.equal(name, spelling)? {
            continue;
        }
        match bind_with_budget(directory, &site, current, b) {
            Ok(binding) => {
                if !std::ptr::eq(binding.target(), target) || !std::ptr::eq(binding.body(), body) {
                    return Err(LocalBindingError::InvalidOrigin);
                }
            }
            Err(LocalBindingError::NoEligibleLocal) => continue,
            Err(error) => return Err(error),
        }
        b.charge(1)?;
        // The count pass reserved every reference node, not merely matching ones.
        if occurrences.len() >= count {
            return Err(LocalBindingError::Resource);
        }
        occurrences.push(site);
    }
    Ok(LocalValueOccurrences {
        directory,
        target,
        body,
        occurrences,
    })
}

#[cfg(test)]
mod occurrence_storage_tests {
    use super::*;

    #[test]
    fn result_storage_refuses_count_and_byte_overflow_before_reservation() {
        occurrence_storage(0).unwrap();
        occurrence_storage(MAX_OCCURRENCES).unwrap();
        assert_eq!(
            occurrence_storage(MAX_OCCURRENCES + 1),
            Err(LocalBindingError::Resource)
        );
        assert_eq!(
            occurrence_storage(usize::MAX),
            Err(LocalBindingError::Resource)
        );
    }
}
