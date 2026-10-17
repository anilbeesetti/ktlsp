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
    AmbiguousTypeParameter,
    NoEligibleTypeParameter,
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
    directory: &'a RegionDirectory<'a>,
    selected: Option<usize>,
    selected_target: Option<&'a Declaration<'a>>,
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
    fn empty_function(&mut self, body: usize, end: usize) -> Result<()> {
        let (index, node) = self.expect("FUN", false, body, end)?;
        self.expect("FUN_KEYWORD", true, index, node.subtree_end)?;
        self.expect("IDENTIFIER", true, index, node.subtree_end)?;
        let (parameters, list) =
            self.expect("VALUE_PARAMETER_LIST", false, index, node.subtree_end)?;
        self.expect("LPAR", true, parameters, list.subtree_end)?;
        self.expect("RPAR", true, parameters, list.subtree_end)?;
        self.finish(parameters, list.subtree_end)?;
        let (block, node_body) = self.expect("BLOCK", false, index, node.subtree_end)?;
        self.expect("LBRACE", true, block, node_body.subtree_end)?;
        self.expect("RBRACE", true, block, node_body.subtree_end)?;
        self.finish(block, node_body.subtree_end)?;
        self.finish(index, node.subtree_end)
    }
    fn call(&mut self, function: &Declaration<'a>, body: usize, end: usize) -> Result<()> {
        let (index, node) = self.expect("CALL_EXPRESSION", false, body, end)?;
        let (callee, reference) =
            self.expect("REFERENCE_EXPRESSION", false, index, node.subtree_end)?;
        let (_, token) = self.expect("IDENTIFIER", true, callee, reference.subtree_end)?;
        if token.start != reference.start || token.end != reference.end {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.finish(callee, reference.subtree_end)?;
        let (arguments, list) =
            self.expect("VALUE_ARGUMENT_LIST", false, index, node.subtree_end)?;
        self.expect("LPAR", true, arguments, list.subtree_end)?;
        self.expect("RPAR", true, arguments, list.subtree_end)?;
        self.finish(arguments, list.subtree_end)?;
        self.finish(index, node.subtree_end)?;
        let target = callable_target(
            self.directory,
            self.parsed,
            function,
            body,
            index,
            reference,
            self.b,
        )?;
        if self.selected == Some(callee) {
            self.selected_target = Some(target);
        }
        Ok(())
    }
    fn function(&mut self, declaration: &'a Declaration<'a>, body_index: usize) -> Result<()> {
        let function_index = declaration.cst_index();
        let function = self.b.element(self.parsed, function_index)?;
        if function.token || function.parent != Some(0) || declaration.incomplete() {
            return Err(LocalBindingError::UnsupportedContext);
        }
        self.cursor = function_index
            .checked_add(1)
            .ok_or(LocalBindingError::Resource)?;
        self.expect("FUN_KEYWORD", true, function_index, function.subtree_end)?;
        self.trivia(function_index, function.subtree_end)?;
        if self.cursor < function.subtree_end {
            let next = self.peek()?;
            if !next.token && self.b.equal(&next.kind, "TYPE_PARAMETER_LIST")? {
                let (_, list) = self.expect(
                    "TYPE_PARAMETER_LIST",
                    false,
                    function_index,
                    function.subtree_end,
                )?;
                // The authenticated lowerer owns the complete leading-list grammar.
                self.b.charge(list.subtree_end - self.cursor)?;
                self.cursor = list.subtree_end;
            }
        }
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
            } else if !e.token && self.b.equal(&e.kind, "FUN")? {
                self.empty_function(body_index, body.subtree_end)?;
            } else if !e.token && self.b.equal(&e.kind, "CALL_EXPRESSION")? {
                self.call(declaration, body_index, body.subtree_end)?;
            } else {
                self.property(body_index, body.subtree_end)?;
            }
        }
        self.expect("RBRACE", true, body_index, body.subtree_end)?;
        self.finish(body_index, body.subtree_end)?;
        self.finish(function_index, function.subtree_end)?;
        Ok(())
    }
}

/// Original local function identity for an authenticated standalone empty call.
/// This is definition-only, not an initializer ReferenceSite or overload resolver.
#[derive(Debug)]
pub struct LocalFunctionBinding<'a> {
    target: &'a Declaration<'a>,
    function: &'a Declaration<'a>,
    body: &'a Region<'a>,
    callee: &'a Element,
}
impl<'a> LocalFunctionBinding<'a> {
    pub fn target(&self) -> &'a Declaration<'a> {
        self.target
    }
    pub fn function(&self) -> &'a Declaration<'a> {
        self.function
    }
    pub fn body(&self) -> &'a Region<'a> {
        self.body
    }
    pub fn callee(&self) -> &'a Element {
        self.callee
    }
}

/// Authenticate the entire ordinary outer body and every call under one budget.
pub fn bind_local_function_callee<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    callee_index: usize,
    current: &SourceSnapshot,
) -> Result<LocalFunctionBinding<'a>> {
    callable_with_budget(
        directory,
        result,
        callee_index,
        current,
        &mut ReferenceBudget::default(),
    )
}

pub(super) fn callable_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    callee_index: usize,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<LocalFunctionBinding<'a>> {
    if !std::ptr::eq(directory.snapshot(), directory.lowered().snapshot()) {
        return Err(LocalBindingError::InvalidOrigin);
    }
    let parsed = authenticated_tree(directory.lowered(), result, current, b)?;
    let callee = b.element(parsed, callee_index)?;
    if callee.token || !b.equal(&callee.kind, "REFERENCE_EXPRESSION")? {
        return Err(LocalBindingError::UnsupportedContext);
    }
    let call = b.element(
        parsed,
        callee.parent.ok_or(LocalBindingError::UnsupportedContext)?,
    )?;
    if call.token || !b.equal(&call.kind, "CALL_EXPRESSION")? {
        return Err(LocalBindingError::UnsupportedContext);
    }
    let body_index = call.parent.ok_or(LocalBindingError::UnsupportedContext)?;
    let (_, body) = find_body(directory, body_index, b)?;
    let body_node = b.element(parsed, body_index)?;
    let function_index = body_node
        .parent
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let mut function = None;
    for declaration in directory.lowered().declarations() {
        b.charge(1)?;
        if declaration.cst_index() == function_index {
            function = Some(declaration);
        }
    }
    let function = function.ok_or(LocalBindingError::UnsupportedContext)?;
    let mut check = BodyCheck {
        parsed,
        directory,
        selected: Some(callee_index),
        selected_target: None,
        cursor: 0,
        b,
    };
    check.function(function, body_index)?;
    let target = check
        .selected_target
        .ok_or(LocalBindingError::UnsupportedContext)?;
    Ok(LocalFunctionBinding {
        target,
        function,
        body,
        callee,
    })
}

// Private lookup is called only by the complete-body walk, never recursively
// through the public query. No name/candidate allocation or per-call reset.
fn callable_target<'a>(
    directory: &'a RegionDirectory<'a>,
    parsed: &'a Parsed,
    function: &Declaration<'a>,
    body: usize,
    call: usize,
    callee: &Element,
    b: &mut ReferenceBudget,
) -> Result<&'a Declaration<'a>> {
    b.charge(1)?;
    let raw = parsed
        .source()
        .get(callee.start..callee.end)
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let spelling = b
        .spelling(raw)?
        .ok_or(LocalBindingError::UnsupportedSpelling)?;
    let (body_ordinal, _) = find_body(directory, body, b)?;
    let mut target = None;
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(3)?;
        let parameter = b.equal(declaration.kind(), "VALUE_PARAMETER")?
            && declaration
                .owner()
                .and_then(|owner| directory.lowered().declarations().get(owner))
                .is_some_and(|owner| std::ptr::eq(owner, function));
        let local = directory
            .memberships()
            .get(ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?
            .region()
            == body_ordinal;
        if !parameter && !local {
            continue;
        }
        if declaration.incomplete() || local && declaration.cst_parent_index() != Some(body) {
            return Err(LocalBindingError::UnsupportedContext);
        }
        // Inspect every relevant spelling before any candidate eligibility.
        let name = b
            .spelling(declaration.raw_name())?
            .ok_or(LocalBindingError::UnsupportedSpelling)?;
        if !b.equal(name, spelling)? {
            continue;
        }
        if parameter || !b.equal(declaration.kind(), "FUN")? {
            return Err(LocalBindingError::AmbiguousLocal);
        }
        // Future duplicates are conservative conflicts, NOT forward candidates.
        if target.replace(declaration).is_some() {
            return Err(LocalBindingError::AmbiguousLocal);
        }
    }
    let target = target.ok_or(LocalBindingError::NoEligibleLocal)?;
    b.charge(3)?;
    if target.cst_index() >= call || target.range().end > callee.start {
        return Err(LocalBindingError::NoEligibleLocal);
    }
    // BodyCheck separately proves every local FUN's exact empty-body grammar.
    Ok(target)
}

// Only after BodyCheck proves every call is a local FUN may its callee be
// excluded from value occurrences. Unknown reference roles still refuse.
fn non_value_reference(
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
    Ok(!parent.token
        && (b.equal(&parent.kind, "USER_TYPE")? || b.equal(&parent.kind, "CALL_EXPRESSION")?))
}

/// An annotation-only lexical identity, not a value binding or inferred type.
#[derive(Debug)]
pub struct LexicalTypeParameterBinding<'a> {
    annotated: &'a Declaration<'a>,
    function: &'a Declaration<'a>,
    body: &'a Region<'a>,
    reference: &'a Element,
    name_token: &'a Element,
    target: &'a Declaration<'a>,
}
impl<'a> LexicalTypeParameterBinding<'a> {
    /// Original parameter/property, or the original Function for a return annotation.
    pub fn annotated(&self) -> &'a Declaration<'a> {
        self.annotated
    }
    pub fn function(&self) -> &'a Declaration<'a> {
        self.function
    }
    pub fn body(&self) -> &'a Region<'a> {
        self.body
    }
    pub fn reference(&self) -> &'a Element {
        self.reference
    }
    pub fn name_token(&self) -> &'a Element {
        self.name_token
    }
    pub fn target(&self) -> &'a Declaration<'a> {
        self.target
    }
}

/// Resolve an original simple annotation in a closed ordinary-function tree.
/// Own type declarations precede ancestors; imports and values are never candidates.
pub fn bind_lexical_type_parameter<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    annotation_reference_cst_index: usize,
    current: &SourceSnapshot,
) -> Result<LexicalTypeParameterBinding<'a>> {
    type_parameter_with_budget(
        directory,
        result,
        annotation_reference_cst_index,
        current,
        &mut ReferenceBudget::default(),
    )
}

pub(super) fn type_parameter_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    index: usize,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<LexicalTypeParameterBinding<'a>> {
    let parsed = type_tree(directory, result, current, b)?;
    let (ordinal, annotation) = type_annotation(directory, parsed, index, b)?;
    let owner = annotation_owner(ordinal, &annotation, b)?;
    type_root(directory, parsed, owner, b)?;
    let context = type_function(directory, parsed, owner, b)?;
    check_type_annotation(directory, parsed, ordinal, &annotation, &context, b)?;
    let spelling = annotation_spelling(parsed, &annotation, b)?;
    let target = type_target(directory, &context, spelling, None, b)?
        .ok_or(LocalBindingError::NoEligibleTypeParameter)?;
    let context = type_function(directory, parsed, target.owner(), b)?;
    Ok(LexicalTypeParameterBinding {
        annotated: annotation.annotated,
        function: context.function,
        body: context.body,
        reference: annotation.reference,
        name_token: annotation.name_token,
        target,
    })
}

fn type_tree<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    b: &mut ReferenceBudget,
) -> Result<&'a Parsed> {
    if !std::ptr::eq(directory.snapshot(), directory.lowered().snapshot()) {
        return Err(LocalBindingError::InvalidOrigin);
    }
    Ok(authenticated_tree(directory.lowered(), result, current, b)?)
}

/// An original simple annotation, never an initializer or callee ReferenceSite.
#[derive(Debug)]
pub struct TypeParameterAnnotation<'a> {
    annotated: &'a Declaration<'a>,
    reference: &'a Element,
    name_token: &'a Element,
    reference_index: usize,
}
impl<'a> TypeParameterAnnotation<'a> {
    /// Original parameter/property, or the original Function for a return annotation.
    pub fn annotated(&self) -> &'a Declaration<'a> {
        self.annotated
    }
    pub fn reference(&self) -> &'a Element {
        self.reference
    }
    pub fn name_token(&self) -> &'a Element {
        self.name_token
    }
    pub fn reference_index(&self) -> usize {
        self.reference_index
    }
    pub fn range(&self) -> std::ops::Range<usize> {
        self.reference.start..self.reference.end
    }
}

// The lowerer owns the grammar. These helpers only authenticate original
// ancestry/membership and separate that work from ONCE complete-body validation.
fn annotation_parts<'a>(
    parsed: &'a Parsed,
    index: usize,
    b: &mut ReferenceBudget,
) -> Result<(usize, &'a Element, &'a Element)> {
    let reference = b.element(parsed, index)?;
    let token_index = index.checked_add(1).ok_or(LocalBindingError::Resource)?;
    let token = b.element(parsed, token_index)?;
    if reference.token
        || !b.equal(&reference.kind, "REFERENCE_EXPRESSION")?
        || !token.token
        || !b.equal(&token.kind, "IDENTIFIER")?
        || token.parent != Some(index)
        || reference.subtree_end
            != token_index
                .checked_add(1)
                .ok_or(LocalBindingError::Resource)?
        || reference.start != token.start
        || reference.end != token.end
        || reference.start >= reference.end
    {
        return Err(LocalBindingError::UnsupportedContext);
    }
    let user_index = reference
        .parent
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let user = b.element(parsed, user_index)?;
    let annotation_index = user.parent.ok_or(LocalBindingError::UnsupportedContext)?;
    let annotation = b.element(parsed, annotation_index)?;
    let annotated_index = annotation
        .parent
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let annotated_node = b.element(parsed, annotated_index)?;
    if user.token
        || !b.equal(&user.kind, "USER_TYPE")?
        || annotation.token
        || !b.equal(&annotation.kind, "TYPE_REFERENCE")?
        || annotated_node.token
    {
        return Err(LocalBindingError::UnsupportedContext);
    }
    Ok((annotated_index, reference, token))
}

fn type_annotation<'a>(
    directory: &'a RegionDirectory<'a>,
    parsed: &'a Parsed,
    index: usize,
    b: &mut ReferenceBudget,
) -> Result<(usize, TypeParameterAnnotation<'a>)> {
    let (annotated_index, reference, name_token) = annotation_parts(parsed, index, b)?;
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(1)?;
        if declaration.cst_index() == annotated_index {
            return Ok((
                ordinal,
                TypeParameterAnnotation {
                    annotated: declaration,
                    reference,
                    name_token,
                    reference_index: index,
                },
            ));
        }
    }
    Err(LocalBindingError::UnsupportedContext)
}

fn annotation_owner(
    ordinal: usize,
    annotation: &TypeParameterAnnotation<'_>,
    b: &mut ReferenceBudget,
) -> Result<Option<usize>> {
    Ok(if b.equal(annotation.annotated.kind(), "FUN")? {
        Some(ordinal)
    } else {
        annotation.annotated.owner()
    })
}

struct TypeFunction<'a> {
    ordinal: usize,
    function: &'a Declaration<'a>,
    body_ordinal: usize,
    body: &'a Region<'a>,
}

fn type_function<'a>(
    directory: &'a RegionDirectory<'a>,
    parsed: &'a Parsed,
    function_ordinal: Option<usize>,
    b: &mut ReferenceBudget,
) -> Result<TypeFunction<'a>> {
    let function_ordinal = function_ordinal.ok_or(LocalBindingError::UnsupportedContext)?;
    b.charge(1)?;
    let function = directory
        .lowered()
        .declarations()
        .get(function_ordinal)
        .ok_or(LocalBindingError::InvalidOrigin)?;
    if function.incomplete() || !b.equal(function.kind(), "FUN")? {
        return Err(LocalBindingError::UnsupportedContext);
    }
    let mut body = None;
    for (body_ordinal, region) in directory.regions().iter().enumerate() {
        b.charge(1)?;
        if region.introducer() == Some(function_ordinal)
            && b.equal(region.kind(), "BLOCK")?
            && body.replace((body_ordinal, region)).is_some()
        {
            return Err(LocalBindingError::InvalidOrigin);
        }
    }
    let (body_ordinal, body) = body.ok_or(LocalBindingError::UnsupportedContext)?;
    let node = b.element(parsed, function.cst_index())?;
    let membership = directory
        .memberships()
        .get(function_ordinal)
        .ok_or(LocalBindingError::InvalidOrigin)?
        .region();
    if let Some(owner) = function.owner() {
        b.charge(2)?;
        let outer = directory
            .lowered()
            .declarations()
            .get(owner)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        let enclosing = directory
            .regions()
            .get(membership)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        if owner >= function_ordinal
            || !b.equal(outer.kind(), "FUN")?
            || enclosing.introducer() != Some(owner)
            || !b.equal(enclosing.kind(), "BLOCK")?
            || node.parent != Some(enclosing.cst_index())
            || body.parent() != Some(membership)
        {
            return Err(LocalBindingError::UnsupportedContext);
        }
    } else if node.parent != Some(0) || body.parent() != Some(membership) || membership != 0 {
        return Err(LocalBindingError::UnsupportedContext);
    }
    Ok(TypeFunction {
        ordinal: function_ordinal,
        function,
        body_ordinal,
        body,
    })
}

// No host recursion or copied scope/name tables. Every function in the closed
// root is authenticated, including uncalled and unrelated descendants.
// ponytail: bounded declaration/region rescans; index only if measured query
// throughput needs it, retaining the same aggregate work/storage ceilings.
fn type_root<'a>(
    directory: &'a RegionDirectory<'a>,
    parsed: &'a Parsed,
    owner: Option<usize>,
    b: &mut ReferenceBudget,
) -> Result<TypeFunction<'a>> {
    let mut root = type_function(directory, parsed, owner, b)?;
    while let Some(parent) = root.function.owner() {
        root = type_function(directory, parsed, Some(parent), b)?;
    }
    let end = b.element(parsed, root.function.cst_index())?.subtree_end;
    for (ordinal, declaration) in directory.lowered().declarations().iter().enumerate() {
        b.charge(1)?;
        if declaration.cst_index() < root.function.cst_index() || declaration.cst_index() >= end {
            continue;
        }
        if b.equal(declaration.kind(), "FUN")? {
            let scope = type_function(directory, parsed, Some(ordinal), b)?;
            crate::call_syntax::validate_type_namespace(
                directory.lowered(),
                parsed,
                declaration,
                scope.body.cst_index(),
                b,
            )?;
        } else if b.equal(declaration.kind(), "TYPE_PARAMETER")? {
            let scope = type_function(directory, parsed, declaration.owner(), b)?;
            let list = b.element(
                parsed,
                declaration
                    .cst_parent_index()
                    .ok_or(LocalBindingError::InvalidOrigin)?,
            )?;
            b.charge(1)?;
            if declaration.incomplete()
                || list.token
                || !b.equal(&list.kind, "TYPE_PARAMETER_LIST")?
                || list.parent != Some(scope.function.cst_index())
                || directory
                    .memberships()
                    .get(ordinal)
                    .ok_or(LocalBindingError::InvalidOrigin)?
                    .region()
                    != directory
                        .memberships()
                        .get(scope.ordinal)
                        .ok_or(LocalBindingError::InvalidOrigin)?
                        .region()
            {
                return Err(LocalBindingError::UnsupportedContext);
            }
            let name = b
                .spelling(declaration.raw_name())?
                .ok_or(LocalBindingError::UnsupportedSpelling)?;
            for previous in &directory.lowered().declarations()[..ordinal] {
                b.charge(1)?;
                if previous.owner() == declaration.owner()
                    && b.equal(previous.kind(), "TYPE_PARAMETER")?
                {
                    let previous_name = b
                        .spelling(previous.raw_name())?
                        .ok_or(LocalBindingError::UnsupportedSpelling)?;
                    if b.equal(name, previous_name)? {
                        return Err(LocalBindingError::AmbiguousTypeParameter);
                    }
                }
            }
        }
    }
    // Visit original annotation references once, in CST/source order. Scanning
    // a Function subtree as one annotation would duplicate all descendants.
    for index in root.function.cst_index()..end {
        let element = b.element(parsed, index)?;
        if !element.token && b.equal(&element.kind, "REFERENCE_EXPRESSION")? {
            let parent = b.element(
                parsed,
                element.parent.ok_or(LocalBindingError::InvalidOrigin)?,
            )?;
            if b.equal(&parent.kind, "USER_TYPE")? {
                let (ordinal, annotation) = type_annotation(directory, parsed, index, b)?;
                let owner = annotation_owner(ordinal, &annotation, b)?;
                let scope = type_function(directory, parsed, owner, b)?;
                check_type_annotation(directory, parsed, ordinal, &annotation, &scope, b)?;
                let name = annotation_spelling(parsed, &annotation, b)?;
                type_target(directory, &scope, name, None, b)?;
            }
        }
    }
    Ok(root)
}

fn check_type_annotation(
    directory: &RegionDirectory<'_>,
    parsed: &Parsed,
    ordinal: usize,
    annotation: &TypeParameterAnnotation<'_>,
    context: &TypeFunction<'_>,
    b: &mut ReferenceBudget,
) -> Result<()> {
    let annotated = annotation.annotated;
    if annotated.incomplete() {
        return Err(LocalBindingError::UnsupportedContext);
    }
    let parameter = b.equal(annotated.kind(), "VALUE_PARAMETER")?;
    let parent_index = annotated
        .cst_parent_index()
        .ok_or(LocalBindingError::UnsupportedContext)?;
    let parent = b.element(parsed, parent_index)?;
    b.charge(2)?;
    let expected_region = if b.equal(annotated.kind(), "FUN")? {
        let user = b.element(
            parsed,
            annotation.reference.parent.ok_or(LocalBindingError::InvalidOrigin)?,
        )?;
        let header = b.element(parsed, user.parent.ok_or(LocalBindingError::InvalidOrigin)?)?;
        // Complete-root namespace validation proved the ordered post-parameter
        // colon/annotation pair. Authenticate this exact original direct header,
        // not a descendant annotation or a range geometrically in our body.
        if ordinal != context.ordinal
            || !std::ptr::eq(annotated, context.function)
            || header.parent != Some(annotated.cst_index())
            || header.start <= annotated.name_range().end
            || header.end > context.body.range().start
        {
            return Err(LocalBindingError::UnsupportedContext);
        }
        directory
            .memberships()
            .get(context.ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?
            .region()
    } else if parameter {
        let owner_ordinal = annotated
            .owner()
            .ok_or(LocalBindingError::UnsupportedContext)?;
        b.charge(1)?;
        let owner = directory
            .lowered()
            .declarations()
            .get(owner_ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        if owner_ordinal != context.ordinal || !std::ptr::eq(owner, context.function) {
            return Err(LocalBindingError::UnsupportedContext);
        }
        if parent.token
            || !b.equal(&parent.kind, "VALUE_PARAMETER_LIST")?
            || parent.parent != Some(owner.cst_index())
        {
            return Err(LocalBindingError::UnsupportedContext);
        }
        directory
            .memberships()
            .get(owner_ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?
            .region()
    } else {
        if annotated.owner() != Some(context.ordinal)
            || !b.equal(annotated.kind(), "PROPERTY")?
            || parent_index != context.body.cst_index()
        {
            return Err(LocalBindingError::UnsupportedContext);
        }
        context.body_ordinal
    };
    if directory
        .memberships()
        .get(ordinal)
        .ok_or(LocalBindingError::InvalidOrigin)?
        .region()
        != expected_region
    {
        return Err(LocalBindingError::UnsupportedContext);
    }
    Ok(())
}

fn annotation_spelling<'a>(
    parsed: &'a Parsed,
    annotation: &TypeParameterAnnotation<'_>,
    b: &mut ReferenceBudget,
) -> Result<&'a str> {
    b.charge(2)?;
    let raw = parsed
        .source()
        .get(annotation.range())
        .ok_or(LocalBindingError::UnsupportedContext)?;
    b.spelling(raw)?
        .ok_or(LocalBindingError::UnsupportedSpelling)
}

fn type_target<'a>(
    directory: &'a RegionDirectory<'a>,
    context: &TypeFunction<'a>,
    spelling: &str,
    renamed: Option<(&Declaration<'_>, &str)>,
    b: &mut ReferenceBudget,
) -> Result<Option<&'a Declaration<'a>>> {
    let mut owner = Some(context.ordinal);
    while let Some(ordinal) = owner {
        b.charge(1)?;
        let function = directory
            .lowered()
            .declarations()
            .get(ordinal)
            .ok_or(LocalBindingError::InvalidOrigin)?;
        let mut target = None;
        for declaration in directory.lowered().declarations() {
            b.charge(1)?;
            if declaration.owner() != Some(ordinal)
                || !b.equal(declaration.kind(), "TYPE_PARAMETER")?
            {
                continue;
            }
            let name = if let Some((selected, name)) = renamed
                && std::ptr::eq(selected, declaration)
            {
                name
            } else {
                b.spelling(declaration.raw_name())?
                    .ok_or(LocalBindingError::UnsupportedSpelling)?
            };
            if b.equal(name, spelling)? && target.replace(declaration).is_some() {
                return Err(LocalBindingError::AmbiguousTypeParameter);
            }
        }
        if target.is_some() {
            return Ok(target);
        }
        owner = function.owner();
    }
    Ok(None)
}

/// Positive type-role anchors; value initializers and callees are not fallbacks.
#[derive(Debug, Clone, Copy)]
pub enum TypeParameterAnchor<'a> {
    Declaration(&'a Declaration<'a>),
    AnnotationReference(usize),
}

#[derive(Debug)]
pub struct TypeParameterOccurrences<'a> {
    directory: &'a RegionDirectory<'a>,
    target: &'a Declaration<'a>,
    function: &'a Declaration<'a>,
    body: &'a Region<'a>,
    occurrences: Vec<TypeParameterAnnotation<'a>>,
}
impl<'a> TypeParameterOccurrences<'a> {
    pub fn directory(&self) -> &'a RegionDirectory<'a> {
        self.directory
    }
    pub fn target(&self) -> &'a Declaration<'a> {
        self.target
    }
    pub fn function(&self) -> &'a Declaration<'a> {
        self.function
    }
    pub fn body(&self) -> &'a Region<'a> {
        self.body
    }
    /// Original bound simple annotations in source order, excluding declaration.
    pub fn occurrences(&self) -> &[TypeParameterAnnotation<'a>] {
        &self.occurrences
    }
}

/// Collect the complete original-identity type partition under one shared budget.
pub fn type_parameter_occurrences<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: TypeParameterAnchor<'a>,
    current: &SourceSnapshot,
) -> Result<TypeParameterOccurrences<'a>> {
    type_occurrences_with_budget(
        directory,
        result,
        anchor,
        current,
        None,
        &mut ReferenceBudget::default(),
    )
}

/// Prepare or rename an original function type parameter without capturing any
/// unedited annotation. Values and proved local functions are separate namespaces.
pub fn guarded_type_parameter_rename<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: TypeParameterAnchor<'a>,
    current: &SourceSnapshot,
    new_name: Option<&str>,
) -> Result<TypeParameterOccurrences<'a>> {
    type_rename_with_budget(
        directory,
        result,
        anchor,
        current,
        new_name,
        &mut ReferenceBudget::default(),
    )
}

pub(super) fn type_rename_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: TypeParameterAnchor<'a>,
    current: &SourceSnapshot,
    new_name: Option<&str>,
    b: &mut ReferenceBudget,
) -> Result<TypeParameterOccurrences<'a>> {
    let found = type_occurrences_with_budget(directory, result, anchor, current, new_name, b)?;
    rename_boundary(current.source(), found.target.name_range(), b)?;
    for annotation in &found.occurrences {
        rename_boundary(current.source(), annotation.range(), b)?;
    }
    b.charge(4)?;
    Ok(found)
}

fn type_annotation_storage(count: usize) -> Result<()> {
    if count > MAX_OCCURRENCES
        || count
            .checked_mul(std::mem::size_of::<TypeParameterAnnotation<'_>>())
            .ok_or(LocalBindingError::Resource)?
            > OCCURRENCE_BYTES
    {
        return Err(LocalBindingError::Resource);
    }
    Ok(())
}
fn rename_boundary(
    source: &str,
    range: std::ops::Range<usize>,
    b: &mut ReferenceBudget,
) -> Result<()> {
    b.charge(4)?;
    let bytes = source.as_bytes();
    let before = range.start.checked_sub(1).and_then(|i| bytes.get(i));
    let after = bytes.get(range.end);
    if [before, after]
        .into_iter()
        .flatten()
        .any(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || !byte.is_ascii())
    {
        return Err(LocalBindingError::UnsupportedContext);
    }
    Ok(())
}

pub(super) fn type_occurrences_with_budget<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    anchor: TypeParameterAnchor<'a>,
    current: &SourceSnapshot,
    new_name: Option<&str>,
    b: &mut ReferenceBudget,
) -> Result<TypeParameterOccurrences<'a>> {
    if let Some(name) = new_name {
        b.spelling(name)?;
        // Native output admission for BOTH declaration and USER_TYPE positions:
        // where is generic recovery, dynamic is DYNAMIC_TYPE, suspend a type
        // modifier. Other soft names (including out/reified) remain identifiers
        // before our admitted delimiters. Check BEFORE same-name quote removal.
        b.charge(19)?;
        if !valid_local_rename_name(name) || matches!(name, "where" | "dynamic" | "suspend") {
            return Err(LocalBindingError::UnsupportedSpelling);
        }
    }
    let parsed = type_tree(directory, result, current, b)?;
    let (declaration, mut selected_annotation) = match anchor {
        TypeParameterAnchor::Declaration(target) => {
            let mut ordinal = None;
            for (i, original) in directory.lowered().declarations().iter().enumerate() {
                b.charge(1)?;
                if std::ptr::eq(original, target) {
                    ordinal = Some(i);
                }
            }
            ordinal.ok_or(LocalBindingError::InvalidOrigin)?;
            if !b.equal(target.kind(), "TYPE_PARAMETER")? {
                return Err(LocalBindingError::UnsupportedContext);
            }
            (Some(target), None)
        }
        TypeParameterAnchor::AnnotationReference(index) => {
            (None, Some(type_annotation(directory, parsed, index, b)?))
        }
    };
    let owner = if let Some(declaration) = declaration {
        declaration.owner()
    } else {
        let (ordinal, annotation) = selected_annotation.as_ref().unwrap();
        annotation_owner(*ordinal, annotation, b)?
    };
    let root = type_root(directory, parsed, owner, b)?;
    let context = type_function(directory, parsed, owner, b)?;
    let spelling = if let Some(target) = declaration {
        b.spelling(target.raw_name())?
            .ok_or(LocalBindingError::UnsupportedSpelling)?
    } else {
        let (ordinal, annotation) = selected_annotation.as_ref().unwrap();
        check_type_annotation(directory, parsed, *ordinal, annotation, &context, b)?;
        annotation_spelling(parsed, annotation, b)?
    };
    let target = type_target(directory, &context, spelling, None, b)?
        .ok_or(LocalBindingError::NoEligibleTypeParameter)?;
    if let Some(name) = new_name {
        // Duplicate same-list binders conflict even when neither has a use.
        for declaration in directory.lowered().declarations() {
            b.charge(1)?;
            if declaration.owner() == target.owner()
                && b.equal(declaration.kind(), "TYPE_PARAMETER")?
                && !std::ptr::eq(declaration, target)
            {
                let other = b
                    .spelling(declaration.raw_name())?
                    .ok_or(LocalBindingError::UnsupportedSpelling)?;
                if b.equal(other, name)? {
                    return Err(LocalBindingError::AmbiguousTypeParameter);
                }
            }
        }
    }
    if declaration.is_some_and(|original| !std::ptr::eq(original, target)) {
        return Err(LocalBindingError::InvalidOrigin);
    }
    // Count ALL annotations before reserving actual borrowed annotation records,
    // including parameters before the body and unrelated external-like types.
    let node = b.element(parsed, root.function.cst_index())?;
    let mut count = 0usize;
    for index in root.function.cst_index()..node.subtree_end {
        let element = b.element(parsed, index)?;
        if !element.token
            && b.equal(&element.kind, "REFERENCE_EXPRESSION")?
            && let Some(parent) = element.parent
        {
            let parent = b.element(parsed, parent)?;
            if b.equal(&parent.kind, "USER_TYPE")? {
                count = count.checked_add(1).ok_or(LocalBindingError::Resource)?;
            }
        }
    }
    type_annotation_storage(count)?;
    let mut occurrences = Vec::new();
    b.reserve(&mut occurrences, count)?;
    type_annotation_storage(occurrences.capacity())?;
    for index in root.function.cst_index()..node.subtree_end {
        let element = b.element(parsed, index)?;
        if element.token || !b.equal(&element.kind, "REFERENCE_EXPRESSION")? {
            continue;
        }
        let parent = b.element(
            parsed,
            element.parent.ok_or(LocalBindingError::InvalidOrigin)?,
        )?;
        if !b.equal(&parent.kind, "USER_TYPE")? {
            continue;
        }
        let selected = selected_annotation
            .as_ref()
            .is_some_and(|(_, annotation)| annotation.reference_index == index);
        let (ordinal, annotation) = if selected {
            selected_annotation.take().unwrap()
        } else {
            type_annotation(directory, parsed, index, b)?
        };
        let owner = annotation_owner(ordinal, &annotation, b)?;
        let annotation_context = type_function(directory, parsed, owner, b)?;
        check_type_annotation(directory, parsed, ordinal, &annotation, &annotation_context, b)?;
            let name = if selected {
                spelling
            } else {
                annotation_spelling(parsed, &annotation, b)?
            };
            let original = type_target(directory, &annotation_context, name, None, b)?;
            let matched = original.is_some_and(|original| std::ptr::eq(original, target));
            if let Some(new_name) = new_name {
                let hypothetical = type_target(
                    directory,
                    &annotation_context,
                    if matched { new_name } else { name },
                    Some((target, new_name)),
                    b,
                )?;
                if original.map(|d| d as *const _) != hypothetical.map(|d| d as *const _) {
                    return Err(LocalBindingError::AmbiguousTypeParameter);
                }
            }
            if matched {
                b.charge(1)?;
                if occurrences.len() >= count {
                    return Err(LocalBindingError::Resource);
                }
                occurrences.push(annotation);
            }
    }
    // Final emission also shares the query budget; no successful prefix escapes.
    b.charge(4)?;
    let context = type_function(directory, parsed, target.owner(), b)?;
    Ok(TypeParameterOccurrences {
        directory,
        target,
        function: context.function,
        body: context.body,
        occurrences,
    })
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
        directory,
        selected: None,
        selected_target: None,
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
                || !(b.equal(declaration.kind(), "PROPERTY")?
                    || b.equal(declaration.kind(), "FUN")?)
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
            if !is_parameter && !b.equal(declaration.kind(), "PROPERTY")? {
                // Also protects renamed values from capturing proved local calls,
                // including conservative conflicts with future local functions.
                return Err(LocalBindingError::AmbiguousLocal);
            }
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
    if !parameter && !b.equal(declaration.kind(), "PROPERTY")? {
        return Ok(false);
    }
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
        directory,
        selected: None,
        selected_target: None,
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
        // Removing quotes from val`x` would produce valfresh. Preparation and
        // unchanged semantic names must also preserve every original boundary.
        rename_boundary(current.source(), range, b)?;
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
            || non_value_reference(parsed, element, b)?
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
        directory,
        selected: None,
        selected_target: None,
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
            && !non_value_reference(parsed, element, b)?
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
            || non_value_reference(parsed, element, b)?
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
        type_annotation_storage(0).unwrap();
        type_annotation_storage(MAX_OCCURRENCES).unwrap();
        for count in [MAX_OCCURRENCES + 1, usize::MAX] {
            assert_eq!(
                type_annotation_storage(count),
                Err(LocalBindingError::Resource)
            );
        }
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
