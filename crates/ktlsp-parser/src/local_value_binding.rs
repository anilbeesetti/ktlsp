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
        loop {
            self.trivia(parameters_index, parameters.subtree_end)?;
            let next = self.peek()?;
            if next.token && self.b.equal(&next.kind, "RPAR")? {
                break;
            }
            let (_, parameter) = self.expect(
                "VALUE_PARAMETER",
                false,
                parameters_index,
                parameters.subtree_end,
            )?;
            // The authenticated lowerer owns the exact name/colon/type grammar.
            self.cursor = parameter.subtree_end;
            self.trivia(parameters_index, parameters.subtree_end)?;
            let next = self.peek()?;
            if next.token && self.b.equal(&next.kind, "RPAR")? {
                break;
            }
            self.expect("COMMA", true, parameters_index, parameters.subtree_end)?;
        }
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

/// Return the unique eligible preceding local val or same-function parameter.
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
    let (body, target) = binding_target_with_budget(directory, site, current, b)?;
    Ok(LocalValueBinding { site, body, target })
}

fn binding_target_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    site: &ReferenceSite<'a>,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<(&'a Region<'a>, &'a Declaration<'a>)> {
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
    let (local, parameter) = value_candidates(
        directory,
        body_ordinal,
        site.body_index(),
        site.function(),
        spelling,
        b,
    )?;
    b.charge(3)?;
    let target = local
        .filter(|target| {
            !std::ptr::eq(*target, site.containing_property())
                && target.cst_index() < site.containing_property().cst_index()
                && target.range().end <= site.range().start
        })
        .or(parameter)
        .ok_or(LocalBindingError::NoEligibleLocal)?;
    Ok((body, target))
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

fn value_candidates<'a>(
    directory: &'a RegionDirectory<'a>,
    body_ordinal: usize,
    body_index: usize,
    function: &Declaration<'a>,
    spelling: &str,
    b: &mut ReferenceBudget,
) -> Result<(Option<&'a Declaration<'a>>, Option<&'a Declaration<'a>>)> {
    let (mut local, mut parameter) = (None, None);
    let (mut local_matches, mut parameter_matches) = (0usize, 0usize);
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(3)?;
        let membership = directory
            .memberships()
            .get(ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        let is_parameter = b.equal(declaration.kind(), "VALUE_PARAMETER")?;
        if is_parameter {
            let owner = declaration
                .owner()
                .and_then(|owner| directory.lowered().declarations().get(owner))
                .ok_or(LocalBindingError::InvalidOrigin)?;
            if !std::ptr::eq(owner, function) {
                continue;
            }
            if !b.equal(declaration.cst_parent_kind(), "VALUE_PARAMETER_LIST")? {
                return Err(LocalBindingError::UnsupportedContext);
            }
        } else {
            if membership.region() != body_ordinal {
                continue;
            }
            if declaration.cst_parent_index() != Some(body_index)
                || !b.equal(declaration.kind(), "PROPERTY")?
            {
                return Err(LocalBindingError::UnsupportedContext);
            }
        }
        if declaration.incomplete() {
            return Err(LocalBindingError::UnsupportedContext);
        }
        let name = b
            .spelling(declaration.raw_name())?
            .ok_or(LocalBindingError::UnsupportedSpelling)?;
        if b.equal(name, spelling)? {
            let matches = if is_parameter {
                &mut parameter_matches
            } else {
                &mut local_matches
            };
            *matches = matches.checked_add(1).ok_or(LocalBindingError::Resource)?;
            if is_parameter {
                parameter = Some(declaration);
            } else {
                local = Some(declaration);
            }
        }
    }
    if local_matches > 1 || parameter_matches > 1 {
        return Err(LocalBindingError::AmbiguousLocal);
    }
    Ok((local, parameter))
}

/// Original visible declarations at one authenticated whole-initializer reference.
/// This is a snapshot-local value subset, not a completion or general scope API.
#[derive(Debug)]
pub struct VisibleLocalValues<'a> {
    directory: &'a RegionDirectory<'a>,
    site: ReferenceSite<'a>,
    body: &'a Region<'a>,
    values: Vec<&'a Declaration<'a>>,
}
impl<'a> VisibleLocalValues<'a> {
    pub fn directory(&self) -> &'a RegionDirectory<'a> {
        self.directory
    }
    pub fn site(&self) -> &ReferenceSite<'a> {
        &self.site
    }
    pub fn body(&self) -> &'a Region<'a> {
        self.body
    }
    /// Surviving original declarations in declaration preorder.
    pub fn values(&self) -> &[&'a Declaration<'a>] {
        &self.values
    }
}

/// Enumerate values without requiring the query spelling to bind successfully.
/// Any ambiguity, unsupported context, stale snapshot or exhausted budget refuses
/// the entire result. Recheck the authoritative open document before publication.
pub fn visible_local_values<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    reference_expression_cst_index: usize,
    current: &SourceSnapshot,
) -> Result<VisibleLocalValues<'a>> {
    visible_with_budget(
        directory,
        result,
        reference_expression_cst_index,
        current,
        &mut ReferenceBudget::default(),
    )
}

const MAX_VISIBLE_VALUES: usize = 1_024;
const VISIBLE_VALUE_BYTES: usize = 131_072;

fn visible_storage(count: usize) -> Result<()> {
    if count > MAX_VISIBLE_VALUES
        || count
            .checked_mul(std::mem::size_of::<&Declaration<'_>>())
            .ok_or(LocalBindingError::Resource)?
            > VISIBLE_VALUE_BYTES
    {
        return Err(LocalBindingError::Resource);
    }
    Ok(())
}

fn visible_winner(
    directory: &RegionDirectory<'_>,
    site: &ReferenceSite<'_>,
    body_ordinal: usize,
    ordinal: usize,
    b: &mut ReferenceBudget,
) -> Result<bool> {
    b.charge(3)?;
    let declaration = &directory.lowered().declarations()[ordinal];
    let parameter = b.equal(declaration.kind(), "VALUE_PARAMETER")?;
    if parameter {
        let owner = declaration
            .owner()
            .and_then(|owner| directory.lowered().declarations().get(owner))
            .ok_or(LocalBindingError::InvalidOrigin)?;
        if !std::ptr::eq(owner, site.function()) {
            return Ok(false);
        }
    } else {
        let membership = directory
            .memberships()
            .get(ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        if membership.region() != body_ordinal
            || declaration.cst_index() >= site.containing_property().cst_index()
            || declaration.range().end > site.range().start
        {
            return Ok(false);
        }
    }
    let spelling = b
        .spelling(declaration.raw_name())?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    let (local, parameter) = value_candidates(
        directory,
        body_ordinal,
        site.body_index(),
        site.function(),
        spelling,
        b,
    )?;
    b.charge(3)?;
    let winner = local
        .filter(|local| {
            local.cst_index() < site.containing_property().cst_index()
                && local.range().end <= site.range().start
        })
        .or(parameter);
    Ok(winner.is_some_and(|winner| std::ptr::eq(winner, declaration)))
}

pub(super) fn visible_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    index: usize,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<VisibleLocalValues<'a>> {
    if !std::ptr::eq(directory.snapshot(), directory.lowered().snapshot()) {
        return Err(LocalBindingError::InvalidOrigin);
    }
    let site = reference_site_with_budget(directory.lowered(), result, current, index, b)?;
    // Authenticate query spelling independently of any declaration-name match.
    b.spelling(site.raw_spelling())?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    let parsed = site.tree(current, b)?;
    BodyCheck {
        parsed,
        cursor: 0,
        b,
    }
    .function(site.function(), site.body_index())?;
    let (body_ordinal, body) = find_body(directory, site.body_index(), b)?;

    // ponytail: borrowed quadratic candidate scans share the 2,097,152-work
    // ceiling; add a validated-body index only if larger bodies are needed.
    let mut count = 0usize;
    for ordinal in 0..directory.lowered().declarations().len() {
        if visible_winner(directory, &site, body_ordinal, ordinal, b)? {
            count = count.checked_add(1).ok_or(LocalBindingError::Resource)?;
        }
    }
    visible_storage(count)?;
    b.charge(count)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| LocalBindingError::Resource)?;
    visible_storage(values.capacity())?;
    for ordinal in 0..directory.lowered().declarations().len() {
        if visible_winner(directory, &site, body_ordinal, ordinal, b)? {
            b.charge(1)?;
            if values.len() >= count {
                return Err(LocalBindingError::Resource);
            }
            values.push(&directory.lowered().declarations()[ordinal]);
        }
    }
    Ok(VisibleLocalValues {
        directory,
        site,
        body,
        values,
    })
}

/// Select one original declaration or authenticate an initializer-reference CST node.
/// Reference selection and occurrence collection share the same semantic budget.
#[derive(Debug, Clone, Copy)]
pub enum LocalValueAnchor<'a> {
    Declaration(&'a Declaration<'a>),
    ReferenceExpression(usize),
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

/// Authenticate an anchor and collect its complete admitted same-body occurrences.
/// All errors discard the result. Local shadowing partitions parameter/local sites;
/// a nonmatching or ineligible name never implies global unresolvedness.
/// Recheck the authoritative open-document incarnation before publication.
pub fn local_value_occurrences<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: LocalValueAnchor<'a>,
    current: &SourceSnapshot,
) -> Result<LocalValueOccurrences<'a>> {
    occurrences_with_budget(
        directory,
        result,
        anchor,
        current,
        &mut ReferenceBudget::default(),
    )
}

/// Supported rename output is deliberately bare ASCII, not Kotlin's full name
/// grammar. Existing quoted targets remain eligible; output is never autoquoted.
pub fn valid_local_rename_name(name: &str) -> bool {
    name.len() <= crate::MAX_SOURCE_BYTES
        && name.as_bytes().first().is_some_and(|b| *b != b'`')
        && crate::lowering::semantic_spelling(name).is_some()
}

/// Authenticate a local immutable-val rename (None prepares the original target).
/// Parameters are refused: same-body occurrences omit external named arguments.
/// Collection and hypothetical-name conflict/capture checks share ONE budget.
pub fn guarded_local_value_rename<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: LocalValueAnchor<'a>,
    current: &SourceSnapshot,
    new_name: Option<&str>,
) -> Result<LocalValueOccurrences<'a>> {
    rename_with_budget(
        directory,
        result,
        anchor,
        current,
        new_name,
        &mut ReferenceBudget::default(),
    )
}

pub(super) fn rename_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: LocalValueAnchor<'a>,
    current: &SourceSnapshot,
    new_name: Option<&str>,
    b: &mut ReferenceBudget,
) -> Result<LocalValueOccurrences<'a>> {
    if let Some(name) = new_name {
        // Bound/charge validation before scanning or comparing user name bytes.
        b.spelling(name)?;
        if !valid_local_rename_name(name) {
            return Err(LocalBindingError::UnsupportedSpelling);
        }
    }
    let found = occurrences_with_budget(directory, result, anchor, current, b)?;
    if !b.equal(found.target.kind(), "PROPERTY")? {
        return Err(LocalBindingError::UnsupportedContext);
    }
    for range in std::iter::once(found.target.name_range())
        .chain(found.occurrences.iter().map(|site| site.range()))
    {
        b.charge(4)?;
        let bytes = current.source().as_bytes();
        let before = range.start.checked_sub(1).and_then(|i| bytes.get(i));
        let after = bytes.get(range.end);
        if [before, after]
            .into_iter()
            .flatten()
            .any(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || !byte.is_ascii())
        {
            // Removing quotes from val`x` would produce valfresh, not a
            // declaration. Check every original boundary even for preparation
            // and unchanged semantic spelling; never add unrequested trivia.
            return Err(LocalBindingError::UnsupportedContext);
        }
    }
    let Some(new_name) = new_name else {
        return Ok(found);
    };
    let old_name = b
        .spelling(found.target.raw_name())?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    if b.equal(old_name, new_name)? {
        // Quote removal changes spelling, not identity. Keep parameter/local
        // shadow partitions rather than rejecting the existing parameter.
        return Ok(found);
    }
    let function = directory
        .lowered()
        .declarations()
        .get(
            found
                .target
                .owner()
                .ok_or(LocalBindingError::InvalidOrigin)?,
        )
        .ok_or(LocalBindingError::InvalidOrigin)?;
    let body_index = found.body.cst_index();
    let (ordinal, _) = find_body(directory, body_index, b)?;
    let (local, parameter) =
        value_candidates(directory, ordinal, body_index, function, new_name, b)?;
    if local.is_some() || parameter.is_some() {
        return Err(LocalBindingError::AmbiguousLocal);
    }
    let parsed = authenticated_tree(directory.lowered(), result, current, b)?;
    for index in body_index..found.body.subtree_end() {
        let element = b.element(parsed, index)?;
        if element.token
            || !b.equal(&element.kind, "REFERENCE_EXPRESSION")?
            || annotation_reference(parsed, element, b)?
        {
            continue;
        }
        let site = reference_site_with_budget(directory.lowered(), result, current, index, b)?;
        let spelling = b
            .spelling(site.raw_spelling())?
            .ok_or(LocalBindingError::UnsupportedSpelling)?;
        if b.equal(spelling, new_name)?
            && site.containing_property().cst_index() > found.target.cst_index()
            && found.target.range().end <= site.range().start
        {
            // No global resolver exists. A previously unresolved value may be
            // external; never turn it into a newly captured local reference.
            return Err(LocalBindingError::UnsupportedContext);
        }
    }
    Ok(found)
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
    anchor: LocalValueAnchor<'a>,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<LocalValueOccurrences<'a>> {
    if !std::ptr::eq(directory.snapshot(), directory.lowered().snapshot()) {
        return Err(LocalBindingError::InvalidOrigin);
    }
    let parsed = authenticated_tree(directory.lowered(), result, current, b)?;
    let target = match anchor {
        LocalValueAnchor::Declaration(target) => target,
        LocalValueAnchor::ReferenceExpression(index) => {
            let site = reference_site_with_budget(directory.lowered(), result, current, index, b)?;
            binding_target_with_budget(directory, &site, current, b)?.1
        }
    };
    let mut target_ordinal = None;
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(1)?;
        if std::ptr::eq(declaration, target) {
            target_ordinal = Some(ordinal);
        }
    }
    let target_ordinal = target_ordinal.ok_or(LocalBindingError::InvalidOrigin)?;
    let function_ordinal = target
        .owner()
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let function = directory
        .lowered()
        .declarations()
        .get(function_ordinal)
        .ok_or(LocalBindingError::InvalidOrigin)?;
    let parameter = b.equal(target.kind(), "VALUE_PARAMETER")?;
    let (body_ordinal, body) = if parameter {
        let mut body = None;
        for (ordinal, region) in directory.regions().iter().enumerate() {
            b.charge(1)?;
            if region.introducer() == Some(function_ordinal)
                && b.equal(region.kind(), "BLOCK")?
                && body.replace((ordinal, region)).is_some()
            {
                return Err(LocalBindingError::InvalidOrigin);
            }
        }
        body.ok_or(LocalBindingError::UnsupportedContext)?
    } else if b.equal(target.kind(), "PROPERTY")? {
        let index = target
            .cst_parent_index()
            .ok_or(LocalBindingError::UnsupportedContext)?;
        find_body(directory, index, b)?
    } else {
        return Err(LocalBindingError::UnsupportedContext);
    };
    let body_index = body.cst_index();
    b.charge(2)?;
    let expected_membership = if parameter {
        directory
            .memberships()
            .get(function_ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?
            .region()
    } else {
        body_ordinal
    };
    if directory
        .memberships()
        .get(target_ordinal)
        .ok_or(LocalBindingError::InvalidOrigin)?
        .region()
        != expected_membership
    {
        return Err(LocalBindingError::UnsupportedContext);
    }
    BodyCheck {
        parsed,
        cursor: 0,
        b,
    }
    .function(function, body_index)?;
    let spelling = b
        .spelling(target.raw_name())?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    let (local, parameter_target) =
        value_candidates(directory, body_ordinal, body_index, function, spelling, b)?;
    let unique =
        if parameter { parameter_target } else { local }.ok_or(LocalBindingError::InvalidOrigin)?;
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
        match binding_target_with_budget(directory, &site, current, b) {
            Ok((bound_body, bound_target)) => {
                if !std::ptr::eq(bound_body, body) {
                    return Err(LocalBindingError::InvalidOrigin);
                }
                if !std::ptr::eq(bound_target, target) {
                    continue;
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
    fn rename_output_name_is_deliberately_ascii_without_autoquoting() {
        for valid in ["x", "_x", "x_0", "field", "get", "value"] {
            assert!(valid_local_rename_name(valid), "{valid}");
        }
        for invalid in [
            "", "_", "___", "fun", "val", "typeof", "1x", "`x`", "a.b", "a/b", "a\\b", "a:b",
            "a;b", "a[b", "a<b", "a>b", "a b", "a\tb", "a\0b", "a\nb", "é", "x😀",
        ] {
            assert!(!valid_local_rename_name(invalid), "{invalid:?}");
        }
        assert!(!valid_local_rename_name(
            &"x".repeat(crate::MAX_SOURCE_BYTES + 1)
        ));
    }

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

#[cfg(test)]
mod visible_storage_tests {
    use super::*;

    #[test]
    fn visible_storage_checks_count_capacity_and_arithmetic_before_allocation() {
        visible_storage(0).unwrap();
        visible_storage(MAX_VISIBLE_VALUES).unwrap();
        for count in [
            MAX_VISIBLE_VALUES + 1,
            VISIBLE_VALUE_BYTES / std::mem::size_of::<&Declaration<'_>>() + 1,
            usize::MAX,
        ] {
            assert_eq!(visible_storage(count), Err(LocalBindingError::Resource));
        }
    }
}
