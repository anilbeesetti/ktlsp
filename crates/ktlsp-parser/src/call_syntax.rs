//! Authenticated original call/parameter syntax, deliberately without callable resolution.
use crate::{
    Element, Parsed, SnapshotResult, SourceSnapshot,
    lowering::{
        Declaration, LoweredDeclarations, ReferenceBudget, ReferenceSiteError, authenticated_tree,
    },
};

type Result<T, E = ReferenceSiteError> = std::result::Result<T, E>;
const UNSUPPORTED: ReferenceSiteError = ReferenceSiteError::UnsupportedContext;

/// A standalone direct call. Argument order is source order, never parameter order.
#[derive(Debug)]
pub struct CallSite<'a> {
    origin: &'a LoweredDeclarations<'a>,
    call_index: usize,
    call: &'a Element,
    callee: &'a Element,
    callee_name: &'a Element,
    argument_list: &'a Element,
    open_paren: &'a Element,
    close_paren: &'a Element,
    arguments: Vec<CallArgument<'a>>,
    function: &'a Declaration<'a>,
    body: &'a Element,
}
impl<'a> CallSite<'a> {
    pub fn origin(&self) -> &'a LoweredDeclarations<'a> {
        self.origin
    }
    pub fn cst_index(&self) -> usize {
        self.call_index
    }
    pub fn call(&self) -> &'a Element {
        self.call
    }
    pub fn callee(&self) -> &'a Element {
        self.callee
    }
    pub fn callee_name(&self) -> &'a Element {
        self.callee_name
    }
    pub fn argument_list(&self) -> &'a Element {
        self.argument_list
    }
    pub fn open_paren(&self) -> &'a Element {
        self.open_paren
    }
    pub fn close_paren(&self) -> &'a Element {
        self.close_paren
    }
    pub fn arguments(&self) -> &[CallArgument<'a>] {
        &self.arguments
    }
    pub fn function(&self) -> &'a Declaration<'a> {
        self.function
    }
    pub fn body(&self) -> &'a Element {
        self.body
    }
}

/// Original argument wrapper and its distinct label/value roles; no inferred type.
#[derive(Debug)]
pub struct CallArgument<'a> {
    ordinal: usize,
    node: &'a Element,
    expression: &'a Element,
    value_token: &'a Element,
    label: Option<ArgumentLabel<'a>>,
    comma: Option<&'a Element>,
}
impl<'a> CallArgument<'a> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn node(&self) -> &'a Element {
        self.node
    }
    pub fn expression(&self) -> &'a Element {
        self.expression
    }
    pub fn value_token(&self) -> &'a Element {
        self.value_token
    }
    pub fn label(&self) -> Option<&ArgumentLabel<'a>> {
        self.label.as_ref()
    }
    /// The following original comma, including a trailing comma on the last argument.
    pub fn comma(&self) -> Option<&'a Element> {
        self.comma
    }
}
#[derive(Debug)]
pub struct ArgumentLabel<'a> {
    node: &'a Element,
    reference: &'a Element,
    name: &'a Element,
    equals: &'a Element,
}
impl<'a> ArgumentLabel<'a> {
    pub fn node(&self) -> &'a Element {
        self.node
    }
    pub fn reference(&self) -> &'a Element {
        self.reference
    }
    pub fn name(&self) -> &'a Element {
        self.name
    }
    pub fn equals(&self) -> &'a Element {
        self.equals
    }
}

/// Parameters OF the explicitly selected original local function, not OF a call.
#[derive(Debug)]
pub struct CallableParameters<'a> {
    origin: &'a LoweredDeclarations<'a>,
    function: &'a Declaration<'a>,
    parameter_list: &'a Element,
    open_paren: &'a Element,
    close_paren: &'a Element,
    parameters: Vec<CallableParameter<'a>>,
    body: &'a Element,
}
impl<'a> CallableParameters<'a> {
    pub fn origin(&self) -> &'a LoweredDeclarations<'a> {
        self.origin
    }
    pub fn function(&self) -> &'a Declaration<'a> {
        self.function
    }
    pub fn parameter_list(&self) -> &'a Element {
        self.parameter_list
    }
    pub fn open_paren(&self) -> &'a Element {
        self.open_paren
    }
    pub fn close_paren(&self) -> &'a Element {
        self.close_paren
    }
    pub fn parameters(&self) -> &[CallableParameter<'a>] {
        &self.parameters
    }
    pub fn body(&self) -> &'a Element {
        self.body
    }
}
#[derive(Debug)]
pub struct CallableParameter<'a> {
    ordinal: usize,
    declaration: &'a Declaration<'a>,
    annotation: &'a Element,
    comma: Option<&'a Element>,
}
impl<'a> CallableParameter<'a> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn declaration(&self) -> &'a Declaration<'a> {
        self.declaration
    }
    /// Original simple annotation syntax. Its spelling does not resolve a type.
    pub fn annotation(&self) -> &'a Element {
        self.annotation
    }
    pub fn comma(&self) -> Option<&'a Element> {
        self.comma
    }
}

// Forward-only original CST cursor. Every inspection and comparison consumes the
// caller's single budget; returned accessors perform no later validating walk.
struct Cursor<'a, 'b> {
    parsed: &'a Parsed,
    at: usize,
    b: &'b mut ReferenceBudget,
}
impl<'a> Cursor<'a, '_> {
    fn trivia(&mut self, parent: usize, end: usize) -> Result<()> {
        while self.at < end {
            let e = self.b.element(self.parsed, self.at)?;
            if !e.token
                || e.parent != Some(parent)
                || !(self.b.equal(&e.kind, "WHITE_SPACE")?
                    || self.b.equal(&e.kind, "EOL_COMMENT")?
                    || self.b.equal(&e.kind, "BLOCK_COMMENT")?)
            {
                break;
            }
            self.at += 1;
        }
        Ok(())
    }
    fn is(&mut self, parent: usize, end: usize, kind: &str) -> Result<bool> {
        self.trivia(parent, end)?;
        if self.at >= end {
            return Ok(false);
        }
        let e = self.b.element(self.parsed, self.at)?;
        Ok(e.parent == Some(parent) && self.b.equal(&e.kind, kind)?)
    }
    fn expect(
        &mut self,
        parent: usize,
        end: usize,
        kind: &str,
        token: bool,
    ) -> Result<(usize, &'a Element)> {
        self.trivia(parent, end)?;
        if self.at >= end {
            return Err(UNSUPPORTED);
        }
        let index = self.at;
        let e = self.b.element(self.parsed, index)?;
        if e.parent != Some(parent) || e.token != token || !self.b.equal(&e.kind, kind)? {
            return Err(UNSUPPORTED);
        }
        self.at = self.at.checked_add(1).ok_or(ReferenceSiteError::Resource)?;
        Ok((index, e))
    }
    fn finish(&mut self, parent: usize, end: usize) -> Result<()> {
        self.trivia(parent, end)?;
        if self.at != end {
            return Err(UNSUPPORTED);
        }
        Ok(())
    }
    fn spelling(&mut self, token: &Element) -> Result<()> {
        self.b.charge(5)?;
        let raw = self
            .parsed
            .source()
            .get(token.start..token.end)
            .ok_or(UNSUPPORTED)?;
        if self.b.spelling(raw)?.is_none() {
            return Err(UNSUPPORTED);
        }
        Ok(())
    }
    fn reference(&mut self, parent: usize, end: usize) -> Result<(&'a Element, &'a Element)> {
        let (index, node) = self.expect(parent, end, "REFERENCE_EXPRESSION", false)?;
        let (_, name) = self.expect(index, node.subtree_end, "IDENTIFIER", true)?;
        if name.start != node.start || name.end != node.end {
            return Err(UNSUPPORTED);
        }
        self.spelling(name)?;
        self.finish(index, node.subtree_end)?;
        Ok((node, name))
    }
    fn argument(&mut self, list: usize, end: usize, ordinal: usize) -> Result<CallArgument<'a>> {
        let (index, node) = self.expect(list, end, "VALUE_ARGUMENT", false)?;
        let label = if self.is(index, node.subtree_end, "VALUE_ARGUMENT_NAME")? {
            let (label_index, label) =
                self.expect(index, node.subtree_end, "VALUE_ARGUMENT_NAME", false)?;
            let (reference, name) = self.reference(label_index, label.subtree_end)?;
            self.finish(label_index, label.subtree_end)?;
            let (_, equals) = self.expect(index, node.subtree_end, "EQ", true)?;
            Some(ArgumentLabel {
                node: label,
                reference,
                name,
                equals,
            })
        } else {
            None
        };
        let (expression, value_token) =
            if self.is(index, node.subtree_end, "REFERENCE_EXPRESSION")? {
                self.reference(index, node.subtree_end)?
            } else {
                let (expression_index, expression) =
                    self.expect(index, node.subtree_end, "INTEGER_CONSTANT", false)?;
                let (_, token) = self.expect(
                    expression_index,
                    expression.subtree_end,
                    "INTEGER_LITERAL",
                    true,
                )?;
                if token.start != expression.start || token.end != expression.end {
                    return Err(UNSUPPORTED);
                }
                self.finish(expression_index, expression.subtree_end)?;
                (expression, token)
            };
        self.finish(index, node.subtree_end)?;
        let comma = if self.is(list, end, "COMMA")? {
            Some(self.expect(list, end, "COMMA", true)?.1)
        } else {
            None
        };
        self.b.charge(7)?;
        Ok(CallArgument {
            ordinal,
            node,
            expression,
            value_token,
            label,
            comma,
        })
    }
}

fn declaration<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    index: usize,
    b: &mut ReferenceBudget,
) -> Result<(usize, &'a Declaration<'a>)> {
    let mut found = None;
    for (ordinal, d) in lowered.declarations().iter().enumerate() {
        b.charge(1)?;
        if d.cst_index() == index {
            found = Some((ordinal, d));
        }
    }
    let (ordinal, d) = found.ok_or(UNSUPPORTED)?;
    if d.incomplete() {
        return Err(UNSUPPORTED);
    }
    Ok((ordinal, d))
}

fn outer_body<'a>(
    parsed: &'a Parsed,
    index: usize,
    b: &mut ReferenceBudget,
) -> Result<(usize, &'a Element)> {
    let body = b.element(parsed, index)?;
    let function_index = body.parent.ok_or(UNSUPPORTED)?;
    let function = b.element(parsed, function_index)?;
    if body.token
        || !b.equal(&body.kind, "BLOCK")?
        || function.token
        || !b.equal(&function.kind, "FUN")?
        || function.parent != Some(0)
    {
        return Err(UNSUPPORTED);
    }
    Ok((function_index, body))
}

/// Select an original standalone CALL_EXPRESSION; unknown/duplicate names and
/// mismatched arity/types are syntax, not successful Kotlin invocations.
/// Recheck the authoritative document under its synchronization before use.
pub fn call_site<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    call_index: usize,
) -> Result<CallSite<'a>> {
    call_site_with_budget(
        lowered,
        result,
        current,
        call_index,
        &mut ReferenceBudget::default(),
    )
}
pub(crate) fn call_site_with_budget<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    call_index: usize,
    b: &mut ReferenceBudget,
) -> Result<CallSite<'a>> {
    let parsed = authenticated_tree(lowered, result, current, b)?;
    let call = b.element(parsed, call_index)?;
    if call.token || !b.equal(&call.kind, "CALL_EXPRESSION")? {
        return Err(UNSUPPORTED);
    }
    let (function_index, body) = outer_body(parsed, call.parent.ok_or(UNSUPPORTED)?, b)?;
    let (_, function) = declaration(lowered, function_index, b)?;
    let mut cursor = Cursor {
        parsed,
        at: call_index + 1,
        b,
    };
    let (callee, callee_name) = cursor.reference(call_index, call.subtree_end)?;
    let (list_index, list) =
        cursor.expect(call_index, call.subtree_end, "VALUE_ARGUMENT_LIST", false)?;
    let (_, open_paren) = cursor.expect(list_index, list.subtree_end, "LPAR", true)?;
    let start = cursor.at;
    let mut count = 0usize;
    while !cursor.is(list_index, list.subtree_end, "RPAR")? {
        let argument = cursor.argument(list_index, list.subtree_end, count)?;
        count = count.checked_add(1).ok_or(ReferenceSiteError::Resource)?;
        if argument.comma.is_none() {
            break;
        }
    }
    let (_, close_paren) = cursor.expect(list_index, list.subtree_end, "RPAR", true)?;
    cursor.finish(list_index, list.subtree_end)?;
    cursor.finish(call_index, call.subtree_end)?;
    let mut arguments = Vec::new();
    cursor.b.reserve(&mut arguments, count)?;
    cursor.at = start;
    for ordinal in 0..count {
        let argument = cursor.argument(list_index, list.subtree_end, ordinal)?;
        cursor.b.charge(1)?;
        if arguments.len() >= arguments.capacity() {
            return Err(ReferenceSiteError::Resource);
        }
        arguments.push(argument);
    }
    cursor.b.charge(11)?;
    Ok(CallSite {
        origin: lowered,
        call_index,
        call,
        callee,
        callee_name,
        argument_list: list,
        open_paren,
        close_paren,
        arguments,
        function,
        body,
    })
}

/// Select a Declaration by original borrowed identity, never by callee spelling.
/// Only direct local, ordinary non-generic functions with empty bodies qualify.
pub fn callable_parameters<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    function: &'a Declaration<'a>,
) -> Result<CallableParameters<'a>> {
    callable_parameters_with_budget(
        lowered,
        result,
        current,
        function,
        &mut ReferenceBudget::default(),
    )
}
pub(crate) fn callable_parameters_with_budget<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    function: &'a Declaration<'a>,
    b: &mut ReferenceBudget,
) -> Result<CallableParameters<'a>> {
    let parsed = authenticated_tree(lowered, result, current, b)?;
    let (owner_ordinal, original) = declaration(lowered, function.cst_index(), b)?;
    if !std::ptr::eq(original, function) {
        return Err(ReferenceSiteError::InvalidOrigin);
    }
    let node = b.element(parsed, function.cst_index())?;
    if node.token || !b.equal(&node.kind, "FUN")? {
        return Err(UNSUPPORTED);
    }
    let (outer_index, _) = outer_body(parsed, node.parent.ok_or(UNSUPPORTED)?, b)?;
    let (outer_ordinal, _) = declaration(lowered, outer_index, b)?;
    if function.owner() != Some(outer_ordinal) {
        return Err(UNSUPPORTED);
    }
    let mut cursor = Cursor {
        parsed,
        at: function.cst_index() + 1,
        b,
    };
    let (list_index, list, open_paren, close_paren, start, count, body) =
        local_parameter_layout(&mut cursor, lowered, owner_ordinal, function)?;
    let mut parameters = Vec::new();
    cursor.b.reserve(&mut parameters, count)?;
    cursor.at = start;
    for ordinal in 0..count {
        let parameter = parameter(
            &mut cursor,
            lowered,
            owner_ordinal,
            list_index,
            list.subtree_end,
            ordinal,
        )?;
        cursor.b.charge(1)?;
        if parameters.len() >= parameters.capacity() {
            return Err(ReferenceSiteError::Resource);
        }
        parameters.push(parameter);
    }
    cursor.b.charge(8)?;
    Ok(CallableParameters {
        origin: lowered,
        function,
        parameter_list: list,
        open_paren,
        close_paren,
        parameters,
        body,
    })
}

// Restricted public callable projection: nongeneric and empty-bodied.
fn local_parameter_layout<'a>(
    cursor: &mut Cursor<'a, '_>,
    lowered: &'a LoweredDeclarations<'a>,
    owner_ordinal: usize,
    function: &Declaration<'a>,
) -> Result<(
    usize,
    &'a Element,
    &'a Element,
    &'a Element,
    usize,
    usize,
    &'a Element,
)> {
    let node = cursor.b.element(cursor.parsed, function.cst_index())?;
    cursor.expect(function.cst_index(), node.subtree_end, "FUN_KEYWORD", true)?;
    let (_, name) = cursor.expect(function.cst_index(), node.subtree_end, "IDENTIFIER", true)?;
    cursor.spelling(name)?;
    let (list_index, list) = cursor.expect(
        function.cst_index(),
        node.subtree_end,
        "VALUE_PARAMETER_LIST",
        false,
    )?;
    let (_, open_paren) = cursor.expect(list_index, list.subtree_end, "LPAR", true)?;
    let start = cursor.at;
    let mut count = 0usize;
    while !cursor.is(list_index, list.subtree_end, "RPAR")? {
        let parameter = parameter(
            cursor,
            lowered,
            owner_ordinal,
            list_index,
            list.subtree_end,
            count,
        )?;
        count = count.checked_add(1).ok_or(ReferenceSiteError::Resource)?;
        if parameter.comma.is_none() {
            break;
        }
    }
    let (_, close_paren) = cursor.expect(list_index, list.subtree_end, "RPAR", true)?;
    cursor.finish(list_index, list.subtree_end)?;
    let (body_index, body) =
        cursor.expect(function.cst_index(), node.subtree_end, "BLOCK", false)?;
    cursor.expect(body_index, body.subtree_end, "LBRACE", true)?;
    cursor.expect(body_index, body.subtree_end, "RBRACE", true)?;
    cursor.finish(body_index, body.subtree_end)?;
    cursor.finish(function.cst_index(), node.subtree_end)?;
    Ok((
        list_index,
        list,
        open_paren,
        close_paren,
        start,
        count,
        body,
    ))
}

fn parameter<'a>(
    cursor: &mut Cursor<'a, '_>,
    lowered: &'a LoweredDeclarations<'a>,
    owner: usize,
    list: usize,
    end: usize,
    ordinal: usize,
) -> Result<CallableParameter<'a>> {
    let (index, node) = cursor.expect(list, end, "VALUE_PARAMETER", false)?;
    let (_, declaration) = declaration(lowered, index, cursor.b)?;
    if declaration.owner() != Some(owner) || declaration.cst_parent_index() != Some(list) {
        return Err(UNSUPPORTED);
    }
    let (_, name) = cursor.expect(index, node.subtree_end, "IDENTIFIER", true)?;
    cursor.spelling(name)?;
    cursor.expect(index, node.subtree_end, "COLON", true)?;
    let (annotation_index, annotation) =
        cursor.expect(index, node.subtree_end, "TYPE_REFERENCE", false)?;
    // Reuse the authenticated lowerer's simple-annotation grammar. This pass only
    // charges the original retained subtree; it does not reinterpret type names.
    cursor.b.charge(
        annotation
            .subtree_end
            .checked_sub(annotation_index + 1)
            .ok_or(UNSUPPORTED)?,
    )?;
    cursor.at = annotation.subtree_end;
    cursor.finish(index, node.subtree_end)?;
    let comma = if cursor.is(list, end, "COMMA")? {
        Some(cursor.expect(list, end, "COMMA", true)?.1)
    } else {
        None
    };
    cursor.b.charge(4)?;
    Ok(CallableParameter {
        ordinal,
        declaration,
        annotation,
        comma,
    })
}

/// Validate the entire ordinary outer type namespace, without allocating call or
/// parameter projections and without deciding whether any call is applicable.
pub(crate) fn validate_type_namespace<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    parsed: &'a Parsed,
    function: &'a Declaration<'a>,
    body_index: usize,
    b: &mut ReferenceBudget,
) -> Result<()> {
    b.charge(1)?;
    // Documentation-only prefix admission is not authority for semantic queries.
    if lowered.documentation_operation.is_some() {
        return Err(UNSUPPORTED);
    }
    let node = b.element(parsed, function.cst_index())?;
    if function.incomplete() {
        return Err(UNSUPPORTED);
    }
    let mut cursor = Cursor {
        parsed,
        at: function.cst_index() + 1,
        b,
    };
    cursor.expect(function.cst_index(), node.subtree_end, "FUN_KEYWORD", true)?;
    if cursor.is(
        function.cst_index(),
        node.subtree_end,
        "TYPE_PARAMETER_LIST",
    )? {
        let (index, list) = cursor.expect(
            function.cst_index(),
            node.subtree_end,
            "TYPE_PARAMETER_LIST",
            false,
        )?;
        cursor.b.charge(list.subtree_end - index)?;
        cursor.at = list.subtree_end;
    }
    cursor.expect(function.cst_index(), node.subtree_end, "IDENTIFIER", true)?;
    let (parameters, list) = cursor.expect(
        function.cst_index(),
        node.subtree_end,
        "VALUE_PARAMETER_LIST",
        false,
    )?;
    // The authenticated lowerer already proved every ordinary outer parameter.
    cursor.b.charge(list.subtree_end - parameters)?;
    cursor.at = list.subtree_end;
    if cursor.is(function.cst_index(), node.subtree_end, "COLON")? {
        cursor.expect(function.cst_index(), node.subtree_end, "COLON", true)?;
        let (index, annotation) =
            cursor.expect(function.cst_index(), node.subtree_end, "TYPE_REFERENCE", false)?;
        // The shared lowerer proved the same finite simple_annotation grammar.
        cursor.b.charge(annotation.subtree_end - index)?;
        cursor.at = annotation.subtree_end;
    }
    let (block, body) = cursor.expect(function.cst_index(), node.subtree_end, "BLOCK", false)?;
    if block != body_index {
        return Err(UNSUPPORTED);
    }
    cursor.expect(block, body.subtree_end, "LBRACE", true)?;
    while !cursor.is(block, body.subtree_end, "RBRACE")? {
        if cursor.is(block, body.subtree_end, "SEMICOLON")? {
            cursor.expect(block, body.subtree_end, "SEMICOLON", true)?;
        } else if cursor.is(block, body.subtree_end, "FUN")? {
            let (index, local) = cursor.expect(block, body.subtree_end, "FUN", false)?;
            let (_, local_declaration) = declaration(lowered, index, cursor.b)?;
            let (outer_ordinal, _) = declaration(lowered, function.cst_index(), cursor.b)?;
            if local_declaration.owner() != Some(outer_ordinal) {
                return Err(UNSUPPORTED);
            }
            // Each descendant is independently checked by the iterative
            // complete-root walk; do not widen local_parameter_layout.
            cursor.b.charge(local.subtree_end - index)?;
            cursor.at = local.subtree_end;
        } else if cursor.is(block, body.subtree_end, "CALL_EXPRESSION")? {
            let (index, call) = cursor.expect(block, body.subtree_end, "CALL_EXPRESSION", false)?;
            cursor.reference(index, call.subtree_end)?;
            let (arguments, list) =
                cursor.expect(index, call.subtree_end, "VALUE_ARGUMENT_LIST", false)?;
            cursor.expect(arguments, list.subtree_end, "LPAR", true)?;
            let mut count = 0usize;
            while !cursor.is(arguments, list.subtree_end, "RPAR")? {
                let argument = cursor.argument(arguments, list.subtree_end, count)?;
                count = count.checked_add(1).ok_or(ReferenceSiteError::Resource)?;
                if argument.comma.is_none() {
                    break;
                }
            }
            cursor.expect(arguments, list.subtree_end, "RPAR", true)?;
            cursor.finish(arguments, list.subtree_end)?;
            cursor.finish(index, call.subtree_end)?;
        } else {
            let (index, property) = cursor.expect(block, body.subtree_end, "PROPERTY", false)?;
            declaration(lowered, index, cursor.b)?;
            // Complete original immutable simple-initializer/annotation grammar
            // was validated by lowering, not inferred from a property's name.
            cursor.b.charge(property.subtree_end - index)?;
            cursor.at = property.subtree_end;
        }
    }
    cursor.expect(block, body.subtree_end, "RBRACE", true)?;
    cursor.finish(block, body.subtree_end)?;
    cursor.finish(function.cst_index(), node.subtree_end)
}
