//! Snapshot-local, same-body local-value binding; not general Kotlin resolution.
use crate::{
    Element, Parsed, SourceSnapshot,
    lowering::{Declaration, ReferenceBudget, ReferenceSite, ReferenceSiteError},
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
    fn function(&mut self, site: &ReferenceSite<'_>) -> Result<()> {
        let function_index = site.function().cst_index();
        let function = self.b.element(self.parsed, function_index)?;
        if function.token || function.parent != Some(0) || site.function().incomplete() {
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
        let (body_index, body) =
            self.expect("BLOCK", false, function_index, function.subtree_end)?;
        if body_index != site.body_index() {
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
    .function(site)?;
    let mut body = None;
    for (ordinal, region) in directory.regions().iter().enumerate() {
        b.charge(1)?;
        if region.cst_index() == site.body_index() {
            body = Some((ordinal, region));
        }
    }
    let (body_ordinal, body) = body.ok_or(LocalBindingError::UnsupportedContext)?;
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
        if declaration.cst_parent_index() != Some(site.body_index())
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
            if !std::ptr::eq(declaration, site.containing_property())
                && declaration.cst_index() < site.containing_property().cst_index()
                && declaration.range().end <= site.range().start
            {
                target = Some(declaration);
            }
        }
    }
    if matches > 1 {
        return Err(LocalBindingError::AmbiguousLocal);
    }
    let target = target.ok_or(LocalBindingError::NoEligibleLocal)?;
    Ok(LocalValueBinding { site, body, target })
}
