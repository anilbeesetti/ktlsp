//! Finite, borrowed declaration/error projection of an already validated source snapshot.
//!
//! Unsupported syntax rejects the whole operation. Owners are result-local ordinals,
//! not scopes or symbols. Only new output capacities and traversal work/storage are
//! bounded here; borrowed input retention, allocator overhead, RSS and time are not.
//! Callers must recheck snapshot identity against their document store before publication.
use crate::{Diagnostic, Element, FileKind, Parsed, SnapshotResult, SourceSnapshot};
use std::{mem::size_of, ops::Range};

const DOCUMENT: usize = 8_192;
const SOURCE: usize = 65_536;
const ELEMENTS: usize = 16_384;
const NODES: usize = 8_192;
const TOKENS: usize = 8_192;
const KIND_BYTES: usize = 64;
const DECLARATIONS: usize = 1_024;
const ERRORS: usize = 256;
const MESSAGE_BYTES: usize = 4_096;
const DEPTH: usize = 128;
const WORK: usize = 2_097_152;
const REQUESTED: usize = 131_072;
const RETAINED: usize = 131_072;
const STACK_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringLimit {
    Document,
    Source,
    Elements,
    Nodes,
    Tokens,
    KindBytes,
    Declarations,
    Errors,
    MessageBytes,
    Depth,
    Work,
    RequestedHeap,
    RetainedCapacity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringError {
    InvalidIdentity,
    StaleSnapshot,
    UnsupportedFileKind,
    Limit(LoweringLimit),
    InvalidCst { element: Option<usize> },
    Unsupported { element: usize },
    Allocation,
}
type Result<T> = std::result::Result<T, LoweringError>;
impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "declaration lowering rejected request: {self:?}")
    }
}
impl std::error::Error for LoweringError {}
fn invalid(i: usize) -> LoweringError {
    LoweringError::InvalidCst { element: Some(i) }
}
fn unsupported(i: usize) -> LoweringError {
    LoweringError::Unsupported { element: i }
}
fn cap(n: usize, maximum: usize, limit: LoweringLimit) -> Result<()> {
    if n > maximum {
        Err(LoweringError::Limit(limit))
    } else {
        Ok(())
    }
}
fn add(n: &mut usize, amount: usize, maximum: usize, limit: LoweringLimit) -> Result<()> {
    let next = n.checked_add(amount).ok_or(LoweringError::Limit(limit))?;
    cap(next, maximum, limit)?;
    *n = next;
    Ok(())
}
fn bytes(count: usize, width: usize, limit: LoweringLimit) -> Result<usize> {
    count.checked_mul(width).ok_or(LoweringError::Limit(limit))
}
#[derive(Default)]
struct Budget {
    work: usize,
    requested: usize,
    retained: usize,
    #[cfg(test)]
    reservations: usize,
    #[cfg(test)]
    fail_reservation: Option<usize>,
    #[cfg(test)]
    extra_capacity: [usize; 2],
    #[cfg(test)]
    emitted: usize,
}
impl Budget {
    fn work(&mut self, n: usize) -> Result<()> {
        add(&mut self.work, n, WORK, LoweringLimit::Work)
    }
    fn equal(&mut self, a: &str, b: &str) -> Result<bool> {
        self.work(a.len())?;
        self.work(b.len())?;
        Ok(a == b)
    }
    fn one_of(&mut self, text: &str, choices: &[&str]) -> Result<bool> {
        for choice in choices {
            self.work(text.len())?;
            self.work(choice.len())?;
        }
        Ok(choices.contains(&text))
    }
    #[cfg(test)]
    fn reservation_count(&mut self, count: usize) -> Result<usize> {
        let site = self.reservations;
        self.reservations += 1;
        if self.fail_reservation == Some(site) {
            return Ok(usize::MAX);
        }
        count
            .checked_add(self.extra_capacity[site])
            .ok_or(LoweringError::Allocation)
    }
}

#[derive(Debug)]
pub struct LoweredDeclarations<'a> {
    snapshot: &'a SourceSnapshot,
    parsed: &'a Parsed,
    declarations: Vec<Declaration<'a>>,
    errors: Vec<LoweredSyntaxError<'a>>,
}
impl<'a> LoweredDeclarations<'a> {
    pub fn snapshot(&self) -> &SourceSnapshot {
        self.snapshot
    }
    pub fn declarations(&self) -> &[Declaration<'a>] {
        &self.declarations
    }
    pub fn errors(&self) -> &[LoweredSyntaxError<'a>] {
        &self.errors
    }
}
#[derive(Debug)]
pub struct Declaration<'a> {
    cst_index: usize,
    node: &'a Element,
    name: &'a Element,
    raw_name: &'a str,
    parent: &'a Element,
    owner: Option<usize>,
    incomplete: bool,
}
impl Declaration<'_> {
    pub fn cst_index(&self) -> usize {
        self.cst_index
    }
    pub fn kind(&self) -> &str {
        &self.node.kind
    }
    pub fn range(&self) -> Range<usize> {
        self.node.start..self.node.end
    }
    pub fn raw_name(&self) -> &str {
        self.raw_name
    }
    /// Borrowed spelling for supported bare and backtick names, without resolution.
    /// `None` means unsupported or malformed: compare only two successful spellings.
    /// Raw spelling/ranges, declaration identity and incompleteness remain unchanged.
    // shortcut: non-ASCII bare names are unsupported; upgrade when Unicode-category validation is admitted.
    pub fn semantic_spelling(&self) -> Option<&str> {
        semantic_spelling(self.raw_name)
    }
    pub fn name_range(&self) -> Range<usize> {
        self.name.start..self.name.end
    }
    pub fn cst_parent_index(&self) -> Option<usize> {
        self.node.parent
    }
    pub fn cst_parent_kind(&self) -> &str {
        &self.parent.kind
    }
    pub fn owner(&self) -> Option<usize> {
        self.owner
    }
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }
}
fn semantic_spelling(raw: &str) -> Option<&str> {
    if raw.len() > SOURCE {
        return None;
    }
    if let Some(body) = raw.strip_prefix('`') {
        let body = body.strip_suffix('`')?;
        return (!body.is_empty() && !body.bytes().any(|b| matches!(b, b'`' | b'\r' | b'\n' | 0)))
            .then_some(body);
    }
    let first = raw.as_bytes().first()?;
    if !first.is_ascii_alphabetic() && *first != b'_' {
        return None;
    }
    let mut named = false;
    for b in raw.bytes() {
        if !b.is_ascii_alphanumeric() && b != b'_' {
            return None;
        }
        named |= b != b'_';
    }
    if !named
        || matches!(
            raw,
            "as" | "break"
                | "class"
                | "continue"
                | "do"
                | "else"
                | "false"
                | "for"
                | "fun"
                | "if"
                | "in"
                | "interface"
                | "is"
                | "null"
                | "object"
                | "package"
                | "return"
                | "super"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typealias"
                | "typeof"
                | "val"
                | "var"
                | "when"
                | "while"
        )
    {
        None
    } else {
        Some(raw)
    }
}

#[derive(Debug)]
pub struct LoweredSyntaxError<'a> {
    cst_index: usize,
    diagnostic: &'a Diagnostic,
    owner: Option<usize>,
}
impl LoweredSyntaxError<'_> {
    pub fn cst_index(&self) -> usize {
        self.cst_index
    }
    pub fn range(&self) -> Range<usize> {
        self.diagnostic.start..self.diagnostic.end
    }
    pub fn message(&self) -> &str {
        &self.diagnostic.message
    }
    pub fn owner(&self) -> Option<usize> {
        self.owner
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
    Root,
    Package,
    Imports,
    Class,
    Fun,
    Property,
    ClassBody,
    Block,
    Parameters,
    Integer,
    Reference,
    Error,
}
impl Tag {
    fn selected(self) -> bool {
        matches!(self, Self::Class | Self::Fun | Self::Property)
    }
    fn keyword(self) -> Option<&'static str> {
        match self {
            Self::Class => Some("CLASS_KEYWORD"),
            Self::Fun => Some("FUN_KEYWORD"),
            Self::Property => Some("VAL_KEYWORD"),
            _ => None,
        }
    }
}
fn tag(text: &str, i: usize, b: &mut Budget) -> Result<Tag> {
    const TAGS: [(&str, Tag); 12] = [
        ("KT_FILE", Tag::Root),
        ("PACKAGE_DIRECTIVE", Tag::Package),
        ("IMPORT_LIST", Tag::Imports),
        ("CLASS", Tag::Class),
        ("FUN", Tag::Fun),
        ("PROPERTY", Tag::Property),
        ("CLASS_BODY", Tag::ClassBody),
        ("BLOCK", Tag::Block),
        ("VALUE_PARAMETER_LIST", Tag::Parameters),
        ("INTEGER_CONSTANT", Tag::Integer),
        ("REFERENCE_EXPRESSION", Tag::Reference),
        ("ERROR_ELEMENT", Tag::Error),
    ];
    for (word, _) in TAGS {
        b.work(text.len())?;
        b.work(word.len())?;
    }
    for (word, value) in TAGS {
        if text == word {
            return Ok(value);
        }
    }
    Err(unsupported(i))
}
#[derive(Clone, Copy)]
struct Frame<'a> {
    node: Option<&'a Element>,
    index: usize,
    child_end: usize,
    tag: Tag,
    owner_frame: Option<usize>,
    ordinal: Option<usize>,
    outer_owner: Option<usize>,
    core: bool,
    name: bool,
    integer: bool,
    owned_error: bool,
    emitted: bool,
    plain: u8,
    reference: bool,
}
impl Frame<'_> {
    const EMPTY: Self = Self {
        node: None,
        index: 0,
        child_end: 0,
        tag: Tag::Root,
        owner_frame: None,
        ordinal: None,
        outer_owner: None,
        core: false,
        name: false,
        integer: false,
        owned_error: false,
        emitted: false,
        plain: 0,
        reference: false,
    };
}
const _: () = assert!(size_of::<Frame<'static>>() <= 128);
const _: () = assert!(size_of::<[Frame<'static>; DEPTH]>() <= STACK_BYTES);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Structure,
    Count,
    Emit,
}
#[derive(Debug, Default)]
struct Counts {
    nodes: usize,
    tokens: usize,
    declarations: usize,
    errors: usize,
}
fn element<'a>(p: &'a Parsed, i: usize, b: &mut Budget) -> Result<&'a Element> {
    b.work(1)?;
    p.elements().get(i).ok_or(invalid(i))
}
fn frame<'a>(s: &[Frame<'a>; DEPTH], i: usize, b: &mut Budget) -> Result<Frame<'a>> {
    b.work(1)?;
    s.get(i).copied().ok_or(invalid(i))
}
fn put_frame<'a>(
    s: &mut [Frame<'a>; DEPTH],
    i: usize,
    value: Frame<'a>,
    b: &mut Budget,
) -> Result<()> {
    b.work(1)?;
    *s.get_mut(i).ok_or(invalid(i))? = value;
    Ok(())
}

/// Lower the closed CLASS/FUN/PROPERTY subset in CST preorder, preserving raw names
/// and original UTF-8 byte ranges. No parsing, name resolution or publication occurs.
pub fn lower_declarations<'a>(
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
) -> Result<LoweredDeclarations<'a>> {
    lower_with_budget(result, current, &mut Budget::default())
}

fn lower_with_budget<'a>(
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    b: &mut Budget,
) -> Result<LoweredDeclarations<'a>> {
    let snapshot = result.snapshot();
    for key in [snapshot.document(), current.document()] {
        cap(key.len(), DOCUMENT, LoweringLimit::Document)?;
    }
    for source in [snapshot.source(), current.source()] {
        cap(source.len(), SOURCE, LoweringLimit::Source)?;
    }
    for key in [snapshot.document(), current.document()] {
        if key.is_empty() {
            return Err(LoweringError::InvalidIdentity);
        }
        for i in 0..key.len() {
            b.work(2)?; // inspected byte plus indexed input access, before access
            if key.as_bytes().get(i) == Some(&0) {
                return Err(LoweringError::InvalidIdentity);
            }
        }
    }
    for n in [
        snapshot.document().len(),
        current.document().len(),
        snapshot.source().len(),
        current.source().len(),
        4,
    ] {
        b.work(n)?;
    }
    let parsed = result
        .tree_for(current)
        .map_err(|_| LoweringError::StaleSnapshot)?;
    if snapshot.kind() != FileKind::Source {
        return Err(LoweringError::UnsupportedFileKind);
    }
    cap(parsed.source().len(), SOURCE, LoweringLimit::Source)?;
    b.work(1)?;
    if !b.equal(parsed.source(), snapshot.source())? {
        return Err(LoweringError::InvalidCst { element: None });
    }
    let mut out = LoweredDeclarations {
        snapshot,
        parsed,
        declarations: Vec::new(),
        errors: Vec::new(),
    };
    let parsed = out.parsed;
    b.work(DEPTH)?; // initialize the one fixed array, reused across every pass
    let mut stack = [Frame::EMPTY; DEPTH];
    walk(parsed, Pass::Structure, &mut stack, b, &mut out)?;
    let counts = walk(parsed, Pass::Count, &mut stack, b, &mut out)?;
    if counts.declarations != 0 {
        let n = bytes(
            counts.declarations,
            size_of::<Declaration<'a>>(),
            LoweringLimit::RequestedHeap,
        )?;
        add(&mut b.requested, n, REQUESTED, LoweringLimit::RequestedHeap)?;
        let reserve = counts.declarations;
        #[cfg(test)]
        let reserve = b.reservation_count(reserve)?;
        out.declarations
            .try_reserve_exact(reserve)
            .map_err(|_| LoweringError::Allocation)?;
        let n = bytes(
            out.declarations.capacity(),
            size_of::<Declaration<'a>>(),
            LoweringLimit::RetainedCapacity,
        )?;
        add(
            &mut b.retained,
            n,
            RETAINED,
            LoweringLimit::RetainedCapacity,
        )?;
    }
    if counts.errors != 0 {
        let n = bytes(
            counts.errors,
            size_of::<LoweredSyntaxError<'a>>(),
            LoweringLimit::RequestedHeap,
        )?;
        add(&mut b.requested, n, REQUESTED, LoweringLimit::RequestedHeap)?;
        let reserve = counts.errors;
        #[cfg(test)]
        let reserve = b.reservation_count(reserve)?;
        out.errors
            .try_reserve_exact(reserve)
            .map_err(|_| LoweringError::Allocation)?;
        let n = bytes(
            out.errors.capacity(),
            size_of::<LoweredSyntaxError<'a>>(),
            LoweringLimit::RetainedCapacity,
        )?;
        add(
            &mut b.retained,
            n,
            RETAINED,
            LoweringLimit::RetainedCapacity,
        )?;
    }
    let emitted = walk(parsed, Pass::Emit, &mut stack, b, &mut out)?;
    if emitted.declarations != counts.declarations
        || emitted.errors != counts.errors
        || out.declarations.len() != counts.declarations
        || out.errors.len() != counts.errors
    {
        return Err(LoweringError::InvalidCst { element: None });
    }
    Ok(out)
}

// Strict ordered states are enforced only for reference-bearing functions.
fn plain_step(f: &mut Frame<'_>, from: u8, to: u8, b: &mut Budget) -> Result<()> {
    b.work(1)?;
    f.plain = if f.plain == from { to } else { u8::MAX };
    Ok(())
}

fn plain_child(f: &mut Frame<'_>, child: Tag, b: &mut Budget) -> Result<()> {
    b.work(1)?;
    match (f.tag, child) {
        (Tag::Fun, Tag::Parameters) => plain_step(f, 2, 3, b),
        (Tag::Fun, Tag::Block) => plain_step(f, 3, 4, b),
        (Tag::Block, Tag::Property) => plain_step(f, 1, 1, b),
        (Tag::Property, Tag::Integer | Tag::Reference) => plain_step(f, 3, 4, b),
        _ => {
            f.plain = u8::MAX;
            Ok(())
        }
    }
}

fn close_frames<'a>(
    s: &mut [Frame<'a>; DEPTH],
    depth: &mut usize,
    at: usize,
    pass: Pass,
    b: &mut Budget,
) -> Result<()> {
    while *depth != 0 {
        let f = frame(s, *depth - 1, b)?;
        let node = f.node.ok_or(invalid(f.index))?;
        if node.subtree_end > at {
            break;
        }
        if node.subtree_end != at {
            return Err(invalid(f.index));
        }
        if pass == Pass::Structure {
            if f.child_end != node.end {
                return Err(invalid(f.index));
            }
        } else if (f.tag.selected() && (!f.core || !f.name))
            || (f.tag == Tag::Integer && !f.integer)
        {
            return Err(unsupported(f.index));
        }
        if pass != Pass::Structure {
            b.work(1)?;
            let complete = f.plain
                == match f.tag {
                    Tag::Fun | Tag::Property => 4,
                    Tag::Block | Tag::Parameters => 2,
                    Tag::Integer | Tag::Reference => 1,
                    _ => u8::MAX,
                }
                && f.plain != u8::MAX;
            if f.tag == Tag::Fun && f.reference && !complete {
                return Err(unsupported(f.index));
            }
            if *depth > 1 {
                let mut parent = frame(s, *depth - 2, b)?;
                if !complete {
                    parent.plain = u8::MAX;
                }
                parent.reference |= f.reference;
                put_frame(s, *depth - 2, parent, b)?;
            }
        }
        b.work(1)?;
        *depth -= 1;
    }
    Ok(())
}

fn context(t: Tag, parent: Option<Frame<'_>>, e: &Element, i: usize) -> Result<()> {
    let p = parent.map(|f| f.tag);
    let allowed = match t {
        Tag::Root => i == 0 && p.is_none(),
        Tag::Package | Tag::Imports => {
            p == Some(Tag::Root) && e.start == e.end && e.subtree_end == i + 1
        }
        Tag::Class | Tag::Fun | Tag::Property => {
            matches!(p, Some(Tag::Root | Tag::ClassBody | Tag::Block))
        }
        Tag::ClassBody => p == Some(Tag::Class),
        Tag::Block | Tag::Parameters => p == Some(Tag::Fun),
        Tag::Integer => matches!(p, Some(Tag::Fun | Tag::Property)),
        Tag::Reference => p == Some(Tag::Property),
        Tag::Error => p.is_some() && !matches!(p, Some(Tag::Package | Tag::Imports)),
    };
    if !allowed {
        return Err(unsupported(i));
    }
    if matches!(
        t,
        Tag::ClassBody | Tag::Block | Tag::Parameters | Tag::Integer
    ) && parent.is_some_and(|p| p.tag.selected() && !p.name)
    {
        return Err(unsupported(i));
    }
    Ok(())
}

fn mark_error<'a>(
    owner: Option<usize>,
    s: &mut [Frame<'a>; DEPTH],
    pass: Pass,
    b: &mut Budget,
    out: &mut LoweredDeclarations<'a>,
    i: usize,
) -> Result<Option<usize>> {
    let Some(slot) = owner else {
        return Ok(None);
    };
    let mut f = frame(s, slot, b)?;
    let ordinal = f.ordinal.ok_or(invalid(i))?;
    f.owned_error = true;
    if pass == Pass::Emit && f.emitted {
        b.work(1)?;
        out.declarations
            .get_mut(ordinal)
            .ok_or(invalid(i))?
            .incomplete = true;
    }
    put_frame(s, slot, f, b)?;
    Ok(Some(ordinal))
}

fn token<'a>(
    p: &'a Parsed,
    e: &'a Element,
    i: usize,
    mut f: Frame<'a>,
    pass: Pass,
    b: &mut Budget,
    out: &mut LoweredDeclarations<'a>,
) -> Result<Frame<'a>> {
    if f.tag == Tag::Reference {
        let node = f.node.ok_or(invalid(i))?;
        if !b.equal(&e.kind, "IDENTIFIER")? || e.start != node.start || e.end != node.end {
            return Err(unsupported(i));
        }
        plain_step(&mut f, 0, 1, b)?;
        if f.plain != 1 {
            return Err(unsupported(i));
        }
        return Ok(f);
    }
    if b.one_of(&e.kind, &["WHITE_SPACE", "EOL_COMMENT", "BLOCK_COMMENT"])? {
        return Ok(f);
    }
    if let Some(keyword) = f.tag.keyword() {
        if b.equal(&e.kind, keyword)? {
            if f.core || f.name {
                return Err(unsupported(i));
            }
            f.core = true;
            plain_step(&mut f, 0, 1, b)?;
        } else if b.equal(&e.kind, "IDENTIFIER")? {
            if !f.core || f.name {
                return Err(unsupported(i));
            }
            f.name = true;
            plain_step(&mut f, 1, 2, b)?;
            if pass == Pass::Emit {
                let ordinal = f.ordinal.ok_or(invalid(i))?;
                let node = f.node.ok_or(invalid(i))?;
                let parent_index = node.parent.ok_or(invalid(i))?;
                let parent = element(p, parent_index, b)?;
                b.work(5)?; // slice access plus worst-case UTF-8 boundary byte/access checks
                let raw_name = p.source().get(e.start..e.end).ok_or(invalid(i))?;
                if out.declarations.len() != ordinal
                    || out.declarations.len() >= out.declarations.capacity()
                {
                    return Err(invalid(i));
                }
                b.work(2)?; // record emission plus output slot write; push must not grow
                out.declarations.push(Declaration {
                    cst_index: f.index,
                    node,
                    name: e,
                    raw_name,
                    parent,
                    owner: f.outer_owner,
                    incomplete: f.owned_error,
                });
                #[cfg(test)]
                {
                    b.emitted += 1;
                }
                f.emitted = true;
            }
        } else if !matches!(f.tag, Tag::Fun | Tag::Property)
            || !b.one_of(&e.kind, &["EQ", "SEMICOLON"])?
        {
            return Err(unsupported(i));
        } else if b.equal(&e.kind, "EQ")? {
            plain_step(&mut f, 2, 3, b)?;
        } else {
            f.plain = u8::MAX;
        }
    } else {
        let allowed = match f.tag {
            Tag::Root => b.equal(&e.kind, "SEMICOLON")?,
            Tag::ClassBody | Tag::Block => {
                let allowed = b.one_of(&e.kind, &["LBRACE", "RBRACE", "SEMICOLON"])?;
                match e.kind.as_str() {
                    "LBRACE" => plain_step(&mut f, 0, 1, b)?,
                    "RBRACE" => plain_step(&mut f, 1, 2, b)?,
                    _ => plain_step(&mut f, 1, 1, b)?,
                }
                allowed
            }
            Tag::Parameters => {
                let allowed = b.one_of(&e.kind, &["LPAR", "RPAR"])?;
                if e.kind == "LPAR" {
                    plain_step(&mut f, 0, 1, b)?;
                } else {
                    plain_step(&mut f, 1, 2, b)?;
                }
                allowed
            }
            Tag::Integer => {
                if !b.equal(&e.kind, "INTEGER_LITERAL")? || f.integer {
                    return Err(unsupported(i));
                }
                f.integer = true;
                plain_step(&mut f, 0, 1, b)?;
                true
            }
            Tag::Error => true, // retain existing validated error tokens opaquely
            _ => false,
        };
        if !allowed {
            return Err(unsupported(i));
        }
    }
    Ok(f)
}

fn walk<'a>(
    p: &'a Parsed,
    pass: Pass,
    s: &mut [Frame<'a>; DEPTH],
    b: &mut Budget,
    out: &mut LoweredDeclarations<'a>,
) -> Result<Counts> {
    let len = p.elements().len();
    if pass == Pass::Structure {
        cap(len, ELEMENTS, LoweringLimit::Elements)?;
        cap(p.diagnostics().len(), ERRORS, LoweringLimit::Errors)?;
        if len == 0 {
            return Err(LoweringError::InvalidCst { element: None });
        }
    }
    let mut counts = Counts::default();
    let mut depth = 0usize;
    let mut cursor = 0usize;
    for i in 0..len {
        close_frames(s, &mut depth, i, pass, b)?;
        b.work(1)?;
        let e = element(p, i, b)?;
        if pass == Pass::Structure {
            if e.token {
                add(&mut counts.tokens, 1, TOKENS, LoweringLimit::Tokens)?;
            } else {
                add(&mut counts.nodes, 1, NODES, LoweringLimit::Nodes)?;
            }
            cap(e.kind.len(), KIND_BYTES, LoweringLimit::KindBytes)?;
        }
        let parent = if depth == 0 {
            None
        } else {
            Some(frame(s, depth - 1, b)?)
        };
        if pass == Pass::Structure {
            b.work(4)?; // two UTF-8 boundary bytes and their indexed accesses
            if e.start > e.end
                || e.end > p.source().len()
                || !p.source().is_char_boundary(e.start)
                || !p.source().is_char_boundary(e.end)
                || e.subtree_end <= i
                || e.subtree_end > len
                || e.parent != parent.map(|f| f.index)
                || (i != 0 && parent.is_none())
            {
                return Err(invalid(i));
            }
            if i == 0
                && (e.token
                    || e.start != 0
                    || e.end != p.source().len()
                    || e.subtree_end != len
                    || !b.equal(&e.kind, "KT_FILE")?)
            {
                return Err(invalid(i));
            }
            if let Some(mut f) = parent {
                let node = f.node.ok_or(invalid(i))?;
                if e.start != f.child_end || e.end > node.end || e.subtree_end > node.subtree_end {
                    return Err(invalid(i));
                }
                f.child_end = e.end;
                put_frame(s, depth - 1, f, b)?;
            }
        }
        let next_depth = depth
            .checked_add(1)
            .ok_or(LoweringError::Limit(LoweringLimit::Depth))?;
        cap(next_depth, DEPTH, LoweringLimit::Depth)?; // tokens/empty nodes count too
        if e.token {
            if pass == Pass::Structure {
                if e.subtree_end != i + 1 || e.start != cursor || e.start == e.end {
                    return Err(invalid(i));
                }
                cursor = e.end;
            } else {
                let f = parent.ok_or(invalid(i))?;
                let f = token(p, e, i, f, pass, b, out)?;
                put_frame(s, depth - 1, f, b)?;
            }
            continue;
        }
        let t = if pass == Pass::Structure {
            Tag::Root
        } else {
            tag(&e.kind, i, b)?
        };
        if pass != Pass::Structure {
            context(t, parent, e, i)?;
            if t == Tag::Reference {
                if depth != 4
                    || frame(s, 0, b)?.tag != Tag::Root
                    || frame(s, 1, b)?.tag != Tag::Fun
                    || frame(s, 2, b)?.tag != Tag::Block
                    || frame(s, 3, b)?.tag != Tag::Property
                    || e.subtree_end != i + 2
                {
                    return Err(unsupported(i));
                }
            }
            if let Some(mut f) = parent {
                plain_child(&mut f, t, b)?;
                put_frame(s, depth - 1, f, b)?;
            }
        }
        let inherited_owner = parent.and_then(|f| f.owner_frame);
        let outer_owner = if let Some(slot) = inherited_owner {
            frame(s, slot, b)?.ordinal
        } else {
            None
        };
        let ordinal = if pass != Pass::Structure && t.selected() {
            let ordinal = counts.declarations;
            add(
                &mut counts.declarations,
                1,
                DECLARATIONS,
                LoweringLimit::Declarations,
            )?;
            Some(ordinal)
        } else {
            None
        };
        let owner_frame = if ordinal.is_some() {
            Some(depth)
        } else {
            inherited_owner
        };
        let is_error = if pass == Pass::Structure {
            b.equal(&e.kind, "ERROR_ELEMENT")?
        } else {
            t == Tag::Error
        };
        if is_error {
            let diagnostic_index = counts.errors;
            add(&mut counts.errors, 1, ERRORS, LoweringLimit::Errors)?;
            if pass == Pass::Structure || pass == Pass::Emit {
                b.work(2)?; // diagnostic visit and indexed input access
                let d = p.diagnostics().get(diagnostic_index).ok_or(invalid(i))?;
                if pass == Pass::Structure {
                    cap(d.message.len(), MESSAGE_BYTES, LoweringLimit::MessageBytes)?;
                    if d.message.is_empty() || d.start != e.start || d.end != e.end {
                        return Err(invalid(i));
                    }
                } else {
                    let owner = mark_error(inherited_owner, s, pass, b, out, i)?;
                    if out.errors.len() != diagnostic_index
                        || out.errors.len() >= out.errors.capacity()
                    {
                        return Err(invalid(i));
                    }
                    b.work(2)?;
                    out.errors.push(LoweredSyntaxError {
                        cst_index: i,
                        diagnostic: d,
                        owner,
                    });
                    #[cfg(test)]
                    {
                        b.emitted += 1;
                    }
                }
            } else {
                mark_error(inherited_owner, s, pass, b, out, i)?;
            }
        }
        b.work(1)?; // stack push, separately from indexed write
        put_frame(
            s,
            depth,
            Frame {
                node: Some(e),
                index: i,
                child_end: e.start,
                tag: t,
                owner_frame,
                ordinal,
                outer_owner,
                reference: t == Tag::Reference,
                ..Frame::EMPTY
            },
            b,
        )?;
        depth = next_depth;
    }
    close_frames(s, &mut depth, len, pass, b)?;
    if depth != 0
        || counts.errors != p.diagnostics().len()
        || (pass == Pass::Structure && cursor != p.source().len())
    {
        return Err(LoweringError::InvalidCst { element: None });
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Private synthetic shapes exercise the defensive boundary, not production parsing.
    struct Tree {
        source: String,
        elements: Vec<Element>,
        diagnostics: Vec<Diagnostic>,
        stack: Vec<usize>,
    }
    impl Tree {
        fn new() -> Self {
            let mut tree = Self {
                source: String::new(),
                elements: vec![],
                diagnostics: vec![],
                stack: vec![],
            };
            tree.open("KT_FILE");
            tree
        }
        fn open(&mut self, kind: &str) {
            let i = self.elements.len();
            self.elements.push(Element {
                kind: kind.into(),
                start: self.source.len(),
                end: 0,
                parent: self.stack.last().copied(),
                subtree_end: 0,
                token: false,
            });
            self.stack.push(i);
        }
        fn close(&mut self) {
            let i = self.stack.pop().unwrap();
            self.elements[i].end = self.source.len();
            self.elements[i].subtree_end = self.elements.len();
        }
        fn token(&mut self, kind: &str, text: &str) {
            let start = self.source.len();
            self.source.push_str(text);
            self.elements.push(Element {
                kind: kind.into(),
                start,
                end: self.source.len(),
                parent: self.stack.last().copied(),
                subtree_end: self.elements.len() + 1,
                token: true,
            });
        }
        fn error(&mut self, message: &str) {
            self.open("ERROR_ELEMENT");
            self.close();
            self.diagnostics.push(Diagnostic {
                start: self.source.len(),
                end: self.source.len(),
                message: message.into(),
            });
        }
        fn header(&mut self, name: &str) {
            self.open("FUN");
            self.token("FUN_KEYWORD", "fun");
            self.token("WHITE_SPACE", " ");
            self.token("IDENTIFIER", name);
        }
        fn finish(mut self) -> SnapshotResult {
            self.close();
            assert!(self.stack.is_empty());
            let source: Arc<str> = self.source.into();
            SnapshotResult {
                snapshot: SourceSnapshot {
                    document: Arc::from("synthetic"),
                    revision: 1,
                    kind: FileKind::Source,
                    source: source.clone(),
                },
                parsed: Parsed {
                    source,
                    elements: self.elements,
                    diagnostics: self.diagnostics,
                },
            }
        }
    }
    fn lower(result: &SnapshotResult) -> Result<LoweredDeclarations<'_>> {
        lower_declarations(result, result.snapshot())
    }
    fn with_error() -> SnapshotResult {
        let mut tree = Tree::new();
        tree.header("f");
        tree.error("missing");
        tree.close();
        tree.finish()
    }
    fn reference_body() -> Tree {
        let mut tree = Tree::new();
        tree.header("f");
        tree.open("VALUE_PARAMETER_LIST");
        tree.token("LPAR", "(");
        tree.token("RPAR", ")");
        tree.close();
        tree.open("BLOCK");
        tree.token("LBRACE", "{");
        tree
    }
    fn local_val(tree: &mut Tree, initializer: &str) {
        tree.open("PROPERTY");
        tree.token("VAL_KEYWORD", "val");
        tree.token("WHITE_SPACE", " ");
        tree.token("IDENTIFIER", "y");
        tree.token("EQ", "=");
        tree.open(initializer);
        if initializer == "REFERENCE_EXPRESSION" {
            tree.token("IDENTIFIER", "x");
        } else {
            tree.token("INTEGER_LITERAL", "1");
        }
        tree.close();
        tree.close();
    }
    fn finish_body(mut tree: Tree) -> SnapshotResult {
        tree.token("RBRACE", "}");
        tree.close();
        tree.close();
        tree.finish()
    }
    fn reference_result() -> SnapshotResult {
        let mut tree = reference_body();
        local_val(&mut tree, "REFERENCE_EXPRESSION");
        finish_body(tree)
    }

    fn structure(result: &SnapshotResult) -> Result<Counts> {
        let mut out = LoweredDeclarations {
            snapshot: result.snapshot(),
            parsed: &result.parsed,
            declarations: vec![],
            errors: vec![],
        };
        walk(
            &result.parsed,
            Pass::Structure,
            &mut [Frame::EMPTY; DEPTH],
            &mut Budget::default(),
            &mut out,
        )
    }
    fn limit<T: std::fmt::Debug>(result: Result<T>, expected: LoweringLimit) {
        assert_eq!(result.unwrap_err(), LoweringError::Limit(expected));
    }

    #[test]
    fn semantic_spelling_literals_keep_source_borrows_and_declaration_data() {
        let cases = [
            ("x", Some("x")),
            ("`x`", Some("x")),
            ("X", Some("X")),
            ("`é`", Some("é")),
            ("`e\u{301}`", Some("e\u{301}")),
            ("é", None),
            ("`local 🦀e\u{301}`", Some("local 🦀e\u{301}")),
            ("`if`", Some("if")),
            ("if", None),
            ("field", Some("field")),
            ("`field`", Some("field")),
            ("_x1", Some("_x1")),
            ("`_x1`", Some("_x1")),
            ("___", None),
            ("`___`", Some("___")),
            ("`a\\u0061`", Some("a\\u0061")),
            ("aa", Some("aa")),
            ("`a.b`", Some("a.b")),
            ("``", None),
            ("`x", None),
            ("x`", None),
            ("`x`y`", None),
            ("`a\nb`", None),
            ("`a\rb`", None),
            ("`a\0b`", None),
            ("9x", None),
            ("x y", None),
            ("x.y", None),
            ("$x", None),
            ("e\u{301}", None),
            ("broken", Some("broken")),
            ("`broken`", Some("broken")),
            ("`a\tb`", Some("a\tb")),
            ("x", Some("x")),
        ];
        let mut tree = Tree::new();
        for (raw, _) in cases {
            tree.header(raw);
            if raw == "broken" {
                tree.error("missing");
            }
            tree.close();
        }
        let result = tree.finish();
        let out = lower(&result).unwrap();
        assert_eq!(out.declarations().len(), cases.len());
        for (record, (raw, expected)) in out.declarations().iter().zip(cases) {
            assert_eq!(record.raw_name(), raw);
            assert_eq!(&result.parsed.source()[record.name_range()], raw);
            assert_eq!(record.semantic_spelling(), expected);
            if let Some(spelling) = record.semantic_spelling() {
                let start = record.name_range().start + usize::from(raw.starts_with('`'));
                assert_eq!(spelling.as_ptr(), result.parsed.source()[start..].as_ptr());
            }
            assert_eq!(record.owner(), None);
            assert_eq!(record.incomplete(), raw == "broken");
        }
        let spelling = |i: usize| out.declarations()[i].semantic_spelling().unwrap();
        assert_eq!(spelling(0), spelling(1));
        assert_ne!(spelling(0), spelling(2));
        assert_ne!(spelling(3), spelling(4));
        assert_ne!(spelling(15), spelling(16));
        assert_eq!(spelling(30), spelling(31));
        assert_eq!(spelling(0), spelling(33));
        assert_ne!(
            out.declarations()[0].name_range(),
            out.declarations()[33].name_range()
        );
        assert_eq!(out.errors().len(), 1);
        assert_eq!(out.errors()[0].owner(), Some(30));
        assert_eq!(out.errors()[0].message(), "missing");
        assert!(semantic_spelling("").is_none());
    }

    #[test]
    fn semantic_spelling_refuses_hard_keywords_but_keeps_contextual_names() {
        for raw in [
            "as",
            "break",
            "class",
            "continue",
            "do",
            "else",
            "false",
            "for",
            "fun",
            "if",
            "in",
            "interface",
            "is",
            "null",
            "object",
            "package",
            "return",
            "super",
            "this",
            "throw",
            "true",
            "try",
            "typealias",
            "typeof",
            "val",
            "var",
            "when",
            "while",
        ] {
            assert!(semantic_spelling(raw).is_none(), "{raw}");
        }
        for raw in ["field", "import", "const", "suspend", "_1"] {
            assert_eq!(semantic_spelling(raw), Some(raw));
        }
        for raw in ["_", "`", "a\0b", "a\rb", "a\nb", "🦀"] {
            assert!(semantic_spelling(raw).is_none(), "{raw:?}");
        }
    }

    #[test]
    fn semantic_spelling_byte_cap_is_inclusive_for_bare_and_multibyte_quoted_names() {
        let bare = "x".repeat(SOURCE);
        assert_eq!(semantic_spelling(&bare), Some(bare.as_str()));
        assert!(semantic_spelling(&(bare + "x")).is_none());
        let quoted = format!("`{}`", "é".repeat((SOURCE - 2) / 2));
        assert_eq!(quoted.len(), SOURCE);
        assert_eq!(semantic_spelling(&quoted), Some(&quoted[1..SOURCE - 1]));
        let over = format!("`{}x`", &quoted[1..SOURCE - 1]);
        assert_eq!(over.len(), SOURCE + 1);
        assert!(semantic_spelling(&over).is_none());
    }

    #[test]
    fn duplicate_raw_unicode_names_remain_separate_borrows() {
        let mut tree = Tree::new();
        for _ in 0..2 {
            tree.header("`🦀e\u{301}`");
            tree.close();
        }
        let result = tree.finish();
        let out = lower(&result).unwrap();
        assert_eq!(out.declarations().len(), 2);
        for record in out.declarations() {
            assert_eq!(record.raw_name(), "`🦀e\u{301}`");
            assert_eq!(
                record.raw_name().as_ptr(),
                result.parsed.source()[record.name_range()].as_ptr()
            );
            assert_eq!(record.owner(), None);
            assert!(!record.incomplete());
        }
        assert!(out.declarations()[0].cst_index() < out.declarations()[1].cst_index());
        assert_ne!(
            out.declarations()[0].name_range(),
            out.declarations()[1].name_range()
        );
    }

    #[test]
    fn pending_emitted_nested_and_root_errors_preserve_ancestry_and_multiplicity() {
        let mut tree = Tree::new();
        tree.error("root");
        tree.open("FUN");
        tree.token("FUN_KEYWORD", "fun");
        tree.token("WHITE_SPACE", " ");
        tree.error("before name");
        tree.token("IDENTIFIER", "outer");
        tree.open("BLOCK");
        tree.token("LBRACE", "{");
        tree.header("inner");
        tree.error("same");
        tree.error("same");
        tree.close();
        tree.token("RBRACE", "}");
        tree.close();
        tree.close();
        let result = tree.finish();
        let out = lower(&result).unwrap();
        assert_eq!(
            out.declarations()
                .iter()
                .map(|d| (d.raw_name(), d.owner(), d.incomplete()))
                .collect::<Vec<_>>(),
            [("outer", None, true), ("inner", Some(0), true)]
        );
        assert_eq!(out.declarations()[1].cst_parent_kind(), "BLOCK");
        assert_eq!(
            out.errors().iter().map(|e| e.owner()).collect::<Vec<_>>(),
            [None, Some(0), Some(1), Some(1)]
        );
        assert_eq!(out.errors()[2].range(), out.errors()[3].range());
        assert_eq!(out.errors()[2].message(), out.errors()[3].message());
        assert_ne!(out.errors()[2].cst_index(), out.errors()[3].cst_index());
        // A nested error alone does not make its outer selected owner incomplete.
        let mut tree = Tree::new();
        tree.header("outer");
        tree.open("BLOCK");
        tree.header("inner");
        tree.error("inner only");
        tree.close();
        tree.close();
        tree.close();
        let result = tree.finish();
        let out = lower(&result).unwrap();
        assert!(!out.declarations()[0].incomplete());
        assert!(out.declarations()[1].incomplete());
    }

    #[test]
    fn identity_precedence_and_same_revision_staleness() {
        let result = with_error();
        for current in [
            SourceSnapshot::new("other", 1, FileKind::Source, result.snapshot.source()).unwrap(),
            SourceSnapshot::new("synthetic", 2, FileKind::Source, result.snapshot.source())
                .unwrap(),
            SourceSnapshot::new("synthetic", 1, FileKind::Script, result.snapshot.source())
                .unwrap(),
            SourceSnapshot::new("synthetic", 1, FileKind::Source, "fun changed").unwrap(),
        ] {
            assert_eq!(
                lower_declarations(&result, &current).unwrap_err(),
                LoweringError::StaleSnapshot
            );
        }
        for key in ["", "a\0b"] {
            let current = SourceSnapshot::new(key, 1, FileKind::Source, "").unwrap();
            assert_eq!(
                lower_declarations(&result, &current).unwrap_err(),
                LoweringError::InvalidIdentity
            );
        }
        let mut script = Tree::new().finish();
        script.snapshot.kind = FileKind::Script;
        assert_eq!(
            lower(&script).unwrap_err(),
            LoweringError::UnsupportedFileKind
        );
        let mut mismatch = Tree::new().finish();
        mismatch.parsed.source = Arc::from("x");
        assert_eq!(
            lower(&mismatch).unwrap_err(),
            LoweringError::InvalidCst { element: None }
        );
    }

    #[test]
    fn document_and_source_exact_and_over_limits() {
        let mut result = Tree::new().finish();
        result.snapshot.document = Arc::from("d".repeat(DOCUMENT));
        assert!(lower(&result).is_ok());
        let current =
            SourceSnapshot::new("d".repeat(DOCUMENT + 1), 1, FileKind::Source, "").unwrap();
        limit(
            lower_declarations(&result, &current),
            LoweringLimit::Document,
        );
        result.snapshot.document = current.document.clone();
        limit(lower(&result), LoweringLimit::Document);
        let mut tree = Tree::new();
        tree.token("WHITE_SPACE", &" ".repeat(SOURCE));
        let mut result = tree.finish();
        assert!(lower(&result).is_ok());
        result.snapshot.source = Arc::from(" ".repeat(SOURCE + 1));
        limit(lower(&result), LoweringLimit::Source);
        let result = Tree::new().finish();
        let mut current = result.snapshot.clone();
        current.source = Arc::from(" ".repeat(SOURCE + 1));
        limit(lower_declarations(&result, &current), LoweringLimit::Source);
        let mut result = Tree::new().finish();
        result.parsed.source = current.source;
        limit(lower(&result), LoweringLimit::Source);
    }

    #[test]
    fn structural_element_node_token_and_depth_caps_are_inclusive() {
        let mut tree = Tree::new();
        for _ in 1..NODES {
            tree.open("PACKAGE_DIRECTIVE");
            tree.close();
        }
        for _ in 0..TOKENS {
            tree.token("WHITE_SPACE", " ");
        }
        let mut result = tree.finish();
        let counts = structure(&result).unwrap();
        assert_eq!(
            (result.parsed.elements.len(), counts.nodes, counts.tokens),
            (ELEMENTS, NODES, TOKENS)
        );
        result
            .parsed
            .elements
            .push(result.parsed.elements[1].clone());
        limit(structure(&result), LoweringLimit::Elements);
        let mut tree = Tree::new();
        for _ in 0..NODES {
            tree.open("PACKAGE_DIRECTIVE");
            tree.close();
        }
        limit(structure(&tree.finish()), LoweringLimit::Nodes);
        let mut tree = Tree::new();
        for _ in 0..=TOKENS {
            tree.token("WHITE_SPACE", " ");
        }
        limit(structure(&tree.finish()), LoweringLimit::Tokens);
        for depth in [DEPTH, DEPTH + 1] {
            let mut tree = Tree::new();
            for _ in 1..depth {
                tree.open("BLOCK");
            }
            for _ in 1..depth {
                tree.close();
            }
            let result = tree.finish();
            if depth == DEPTH {
                assert!(structure(&result).is_ok());
            } else {
                limit(structure(&result), LoweringLimit::Depth);
            }
        }
        // A leaf token also occupies a depth level.
        let mut tree = Tree::new();
        for _ in 1..DEPTH {
            tree.open("BLOCK");
        }
        tree.token("WHITE_SPACE", " ");
        for _ in 1..DEPTH {
            tree.close();
        }
        limit(structure(&tree.finish()), LoweringLimit::Depth);
    }

    #[test]
    fn kind_message_error_and_declaration_caps_are_inclusive() {
        let mut tree = Tree::new();
        tree.open(&"x".repeat(KIND_BYTES));
        tree.close();
        let mut result = tree.finish();
        assert!(structure(&result).is_ok());
        result.parsed.elements[1].kind.push('x');
        limit(structure(&result), LoweringLimit::KindBytes);
        let mut tree = Tree::new();
        for _ in 0..ERRORS {
            tree.error(&"e".repeat(MESSAGE_BYTES));
        }
        let mut result = tree.finish();
        assert_eq!(lower(&result).unwrap().errors.len(), ERRORS);
        result.parsed.diagnostics[0].message.push('e');
        limit(lower(&result), LoweringLimit::MessageBytes);
        result
            .parsed
            .diagnostics
            .push(result.parsed.diagnostics[0].clone());
        limit(lower(&result), LoweringLimit::Errors);
        for count in [DECLARATIONS, DECLARATIONS + 1] {
            let mut tree = Tree::new();
            for _ in 0..count {
                tree.header("f");
                tree.close();
            }
            let result = tree.finish();
            if count == DECLARATIONS {
                assert_eq!(lower(&result).unwrap().declarations.len(), DECLARATIONS);
            } else {
                limit(lower(&result), LoweringLimit::Declarations);
            }
        }
    }

    #[test]
    fn defensive_shapes_ranges_and_diagnostics_reject_atomically() {
        for change in 0..12 {
            let mut result = with_error();
            match change {
                0 => result.parsed.elements.clear(),
                1 => result.parsed.elements[0].parent = Some(0),
                2 => result.parsed.elements[1].parent = None,
                3 => result.parsed.elements[2].subtree_end = 4,
                4 => result.parsed.elements[1].end -= 1,
                5 => result.parsed.elements[2].start = 1,
                6 => result.parsed.elements[2].end = usize::MAX,
                7 => result.parsed.elements[1].subtree_end = usize::MAX,
                8 => result.parsed.diagnostics.clear(),
                9 => result.parsed.diagnostics[0].start -= 1,
                10 => result.parsed.diagnostics[0].message.clear(),
                11 => result.parsed.elements[2].end = 0,
                _ => unreachable!(),
            }
            assert!(
                matches!(lower(&result), Err(LoweringError::InvalidCst { .. })),
                "change {change}"
            );
        }
        let mut tree = Tree::new();
        tree.header("🦀");
        tree.close();
        let mut result = tree.finish();
        result.parsed.elements[4].start += 1;
        assert!(matches!(
            lower(&result),
            Err(LoweringError::InvalidCst { .. })
        ));
    }

    #[test]
    fn closed_subset_rejects_unsupported_owners_names_and_lazy_tokens() {
        for kind in [
            "OBJECT_DECLARATION",
            "TYPEALIAS",
            "VALUE_PARAMETER",
            "REFERENCE_EXPRESSION",
            "MODIFIER_LIST",
        ] {
            let mut tree = Tree::new();
            tree.open(kind);
            tree.header("hidden");
            tree.close();
            tree.close();
            assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(1));
        }
        for kind in ["PACKAGE_DIRECTIVE", "IMPORT_LIST"] {
            let mut tree = Tree::new();
            tree.open(kind);
            tree.token("IDENTIFIER", "x");
            tree.close();
            assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(1));
        }
        for token in ["INTERFACE_KEYWORD", "VAR_KEYWORD", "WHITESPACE"] {
            let mut tree = Tree::new();
            tree.open("CLASS");
            tree.token(token, "x");
            tree.close();
            assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(2));
        }
        let mut tree = Tree::new();
        tree.header("f");
        tree.token("IDENTIFIER", "g");
        tree.close();
        assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(5));
        let mut tree = Tree::new();
        tree.header("f");
        tree.open("BLOCK");
        tree.token("FUN_KEYWORD", "fun");
        tree.close();
        tree.close();
        assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(6));
        let mut tree = Tree::new();
        tree.open("FUN");
        tree.open("VALUE_PARAMETER_LIST");
        tree.close();
        tree.close();
        assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(2));
    }

    #[test]
    fn traversal_event_and_eof_closing_precedence() {
        let mut tree = Tree::new();
        tree.open("FUN");
        tree.open("UNKNOWN");
        tree.close();
        tree.close();
        assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(2));
        let mut tree = Tree::new();
        tree.open("FUN");
        tree.close();
        assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(1));
        let mut tree = Tree::new();
        tree.header("f");
        tree.open("INTEGER_CONSTANT");
        tree.close();
        tree.close();
        assert_eq!(lower(&tree.finish()).unwrap_err(), unsupported(5));
        // Structure always wins over a later context failure, even if its index is larger.
        let mut tree = Tree::new();
        tree.open("UNKNOWN");
        tree.close();
        tree.token("WHITE_SPACE", " ");
        let mut result = tree.finish();
        result.parsed.elements[2].parent = None;
        assert_eq!(lower(&result).unwrap_err(), invalid(2));
    }

    #[test]
    fn reference_shapes_require_the_complete_ordered_function_and_property() {
        let baseline = reference_result();
        assert_eq!(lower(&baseline).unwrap().declarations().len(), 2);
        for change in 0..16 {
            let mut result = reference_result();
            let elements = &mut result.parsed.elements;
            let find = |kind: &str| elements.iter().position(|e| e.kind == kind).unwrap();
            let (parameters, block, property, reference) = (
                find("VALUE_PARAMETER_LIST"),
                find("BLOCK"),
                find("PROPERTY"),
                find("REFERENCE_EXPRESSION"),
            );
            let (lpar, rpar, lbrace, rbrace, eq) = (
                find("LPAR"),
                find("RPAR"),
                find("LBRACE"),
                find("RBRACE"),
                find("EQ"),
            );
            match change {
                0 => elements[eq].kind = "WHITE_SPACE".into(),
                1 => elements[property + 2].kind = "EQ".into(),
                2 => elements[property + 1].kind = "VAR_KEYWORD".into(),
                3 => elements[property + 3].kind = "WHITE_SPACE".into(),
                4 => elements[lpar].kind = "RPAR".into(),
                5 => elements[rpar].kind = "LPAR".into(),
                6 => elements[lbrace].kind = "RBRACE".into(),
                7 => elements[rbrace].kind = "LBRACE".into(),
                8 => elements[reference + 1].kind = "INTEGER_LITERAL".into(),
                9 => elements[reference + 1].kind = "WHITE_SPACE".into(),
                10 => elements[reference].end -= 1,
                11 => elements[reference + 1].parent = Some(property),
                12 => elements[reference].subtree_end = reference + 1,
                13 => elements[parameters].kind = "INTEGER_CONSTANT".into(),
                14 => elements[block].kind = "CLASS_BODY".into(),
                15 => elements[eq].kind = "SEMICOLON".into(),
                _ => unreachable!(),
            }
            assert!(lower(&result).is_err(), "change {change}");
        }
        for child in [
            None,
            Some("WHITE_SPACE"),
            Some("IDENTIFIER"),
            Some("INTEGER_CONSTANT"),
        ] {
            let mut tree = reference_body();
            tree.open("PROPERTY");
            tree.token("VAL_KEYWORD", "val");
            tree.token("IDENTIFIER", "y");
            tree.token("EQ", "=");
            tree.open("REFERENCE_EXPRESSION");
            if child.is_some() {
                tree.token("IDENTIFIER", "x");
            }
            match child {
                Some("INTEGER_CONSTANT") => {
                    tree.open("INTEGER_CONSTANT");
                    tree.token("INTEGER_LITERAL", "1");
                    tree.close();
                }
                Some(kind) => tree.token(kind, "x"),
                None => {}
            }
            tree.close();
            tree.close();
            assert!(lower(&finish_body(tree)).is_err(), "child {child:?}");
        }
    }

    #[test]
    fn reference_admission_refuses_wrong_ancestry_and_invalid_siblings_in_either_order() {
        for owner in ["KT_FILE", "FUN", "CLASS_BODY", "BLOCK"] {
            let mut tree = Tree::new();
            if owner == "FUN" {
                tree.header("f");
            } else if owner != "KT_FILE" {
                tree.open("CLASS");
                tree.token("CLASS_KEYWORD", "class");
                tree.token("IDENTIFIER", "C");
                tree.open("CLASS_BODY");
                if owner == "BLOCK" {
                    tree.header("f");
                    tree.open("BLOCK");
                }
            }
            local_val(&mut tree, "REFERENCE_EXPRESSION");
            while tree.stack.len() > 1 {
                tree.close();
            }
            assert!(lower(&tree.finish()).is_err(), "owner {owner}");
        }
        for before in [true, false] {
            for bad in 0..7 {
                let mut tree = reference_body();
                if !before {
                    local_val(&mut tree, "REFERENCE_EXPRESSION");
                }
                match bad {
                    0 => tree.error("incomplete"),
                    1 => {
                        tree.header("local");
                        tree.close();
                    }
                    2 => {
                        tree.open("CLASS");
                        tree.token("CLASS_KEYWORD", "class");
                        tree.token("IDENTIFIER", "Local");
                        tree.close();
                    }
                    _ => {
                        tree.open("PROPERTY");
                        tree.token("VAL_KEYWORD", "val");
                        tree.token("IDENTIFIER", "z");
                        if bad != 3 {
                            tree.token("EQ", "=");
                        }
                        if bad == 4 {
                            tree.token("EQ", "=");
                        }
                        for _ in 0..if bad == 5 { 2 } else { 1 } {
                            tree.open("INTEGER_CONSTANT");
                            tree.token("INTEGER_LITERAL", "1");
                            tree.close();
                        }
                        if bad == 6 {
                            tree.error("incomplete property");
                        }
                        tree.close();
                    }
                }
                if before {
                    local_val(&mut tree, "REFERENCE_EXPRESSION");
                }
                let result = finish_body(tree);
                let mut b = Budget::default();
                assert!(
                    lower_with_budget(&result, result.snapshot(), &mut b).is_err(),
                    "before {before} bad {bad}"
                );
                assert_eq!(b.reservations, 0);
            }
        }
        let mut tree = reference_body();
        local_val(&mut tree, "REFERENCE_EXPRESSION");
        tree.token("RBRACE", "}");
        tree.close();
        tree.close();
        tree.header("legacy");
        tree.error("old error");
        tree.close();
        let result = tree.finish();
        let out = lower(&result).unwrap();
        assert_eq!(out.declarations().len(), 3);
        assert!(out.declarations()[2].incomplete());
        assert_eq!(out.errors().len(), 1);
    }

    #[test]
    fn reference_ordering_and_repeated_bodies_refuse_without_rescans() {
        for change in 0..7 {
            let mut tree = Tree::new();
            tree.header("f");
            for _ in 0..if change == 0 {
                0
            } else if change == 1 {
                2
            } else {
                1
            } {
                tree.open("VALUE_PARAMETER_LIST");
                tree.token("LPAR", "(");
                tree.token("RPAR", ")");
                tree.close();
            }
            tree.open("BLOCK");
            tree.token("LBRACE", "{");
            tree.open("PROPERTY");
            tree.token("VAL_KEYWORD", "val");
            tree.token("IDENTIFIER", "y");
            if change != 2 {
                tree.token("EQ", "=");
            }
            if change == 3 {
                tree.open("INTEGER_CONSTANT");
                tree.token("INTEGER_LITERAL", "1");
                tree.close();
            }
            tree.open("REFERENCE_EXPRESSION");
            tree.token("IDENTIFIER", "x");
            tree.close();
            if change == 2 {
                tree.token("EQ", "=");
            }
            if change == 4 {
                tree.open("INTEGER_CONSTANT");
                tree.token("INTEGER_LITERAL", "1");
                tree.close();
            }
            if change == 5 {
                tree.open("REFERENCE_EXPRESSION");
                tree.token("IDENTIFIER", "x");
                tree.close();
            }
            tree.close();
            if change == 6 {
                tree.token("RBRACE", "}");
                tree.close();
                tree.open("BLOCK");
                tree.token("LBRACE", "{");
            }
            assert!(lower(&finish_body(tree)).is_err(), "change {change}");
        }
        let mut work = Vec::new();
        for count in [16, 32, 64] {
            let mut tree = reference_body();
            for _ in 0..count {
                local_val(&mut tree, "REFERENCE_EXPRESSION");
                tree.token("SEMICOLON", ";");
            }
            let result = finish_body(tree);
            let mut b = Budget::default();
            assert_eq!(
                lower_with_budget(&result, result.snapshot(), &mut b)
                    .unwrap()
                    .declarations()
                    .len(),
                count + 1
            );
            work.push(b.work);
        }
        assert_eq!(work[2] - work[1], 2 * (work[1] - work[0]));
    }

    #[test]
    fn reference_budget_stale_and_origin_failures_remain_atomic() {
        let result = reference_result();
        let mut measured = Budget::default();
        let lower = lower_with_budget(&result, result.snapshot(), &mut measured).unwrap();
        let other = reference_result();
        assert_eq!(
            crate::structural_regions::build_regions(&lower, &other, other.snapshot()).unwrap_err(),
            crate::structural_regions::RegionError::InvalidOrigin
        );
        let stale =
            SourceSnapshot::new("synthetic", 2, FileKind::Source, result.snapshot().source())
                .unwrap();
        assert_eq!(
            lower_declarations(&result, &stale).unwrap_err(),
            LoweringError::StaleSnapshot
        );
        let mut exact = Budget {
            work: WORK - measured.work,
            ..Budget::default()
        };
        assert!(lower_with_budget(&result, result.snapshot(), &mut exact).is_ok());
        assert_eq!(exact.work, WORK);
        let mut over = Budget {
            work: WORK - measured.work + 1,
            ..Budget::default()
        };
        limit(
            lower_with_budget(&result, result.snapshot(), &mut over),
            LoweringLimit::Work,
        );
        let mut fail = Budget {
            fail_reservation: Some(0),
            ..Budget::default()
        };
        assert_eq!(
            lower_with_budget(&result, result.snapshot(), &mut fail).unwrap_err(),
            LoweringError::Allocation
        );
        assert_eq!(fail.emitted, 0);
        let mut requested = Budget {
            requested: REQUESTED - measured.requested + 1,
            ..Budget::default()
        };
        limit(
            lower_with_budget(&result, result.snapshot(), &mut requested),
            LoweringLimit::RequestedHeap,
        );
        assert_eq!(requested.emitted, 0);
        let mut capacity = Budget::default();
        capacity.extra_capacity[0] = RETAINED;
        limit(
            lower_with_budget(&result, result.snapshot(), &mut capacity),
            LoweringLimit::RetainedCapacity,
        );
        assert_eq!(capacity.emitted, 0);
    }

    #[test]
    fn accounting_exact_over_overflow_and_dispatch_precharge() {
        for (maximum, which) in [
            (WORK, LoweringLimit::Work),
            (REQUESTED, LoweringLimit::RequestedHeap),
            (RETAINED, LoweringLimit::RetainedCapacity),
        ] {
            let mut n = maximum - 1;
            add(&mut n, 1, maximum, which).unwrap();
            limit(add(&mut n, 1, maximum, which), which);
            assert_eq!(n, maximum);
            let mut n = usize::MAX;
            limit(add(&mut n, 1, maximum, which), which);
            limit(bytes(usize::MAX, 2, which), which);
        }
        let mut b = Budget::default();
        assert!(b.one_of("a", &["a", "long"]).unwrap());
        assert_eq!(b.work, 7); // both candidate comparisons reserved, including the unexecuted second.
        let mut b = Budget {
            work: WORK - 6,
            ..Budget::default()
        };
        limit(b.one_of("a", &["a", "long"]), LoweringLimit::Work);
    }

    #[test]
    fn both_real_reservation_failures_and_capacity_excess_are_atomic() {
        let result = with_error();
        let decl = size_of::<Declaration<'_>>();
        let error = size_of::<LoweredSyntaxError<'_>>();
        let mut measured = Budget::default();
        assert!(lower_with_budget(&result, result.snapshot(), &mut measured).is_ok());
        for site in 0..2 {
            let mut b = Budget {
                fail_reservation: Some(site),
                ..Budget::default()
            };
            assert_eq!(
                lower_with_budget(&result, result.snapshot(), &mut b).unwrap_err(),
                LoweringError::Allocation
            );
            assert_eq!(b.reservations, site + 1);
            assert_eq!(b.emitted, 0);
            assert_eq!(b.requested, if site == 0 { decl } else { decl + error });
        }
        for site in 0..2 {
            let mut b = Budget::default();
            b.extra_capacity[site] = RETAINED;
            limit(
                lower_with_budget(&result, result.snapshot(), &mut b),
                LoweringLimit::RetainedCapacity,
            );
            assert_eq!(b.reservations, site + 1);
            assert_eq!(b.emitted, 0);
        }
        // Each individual vector fits, but their actual combined capacities do not.
        let mut b = Budget {
            retained: RETAINED - measured.retained + 1,
            ..Budget::default()
        };
        limit(
            lower_with_budget(&result, result.snapshot(), &mut b),
            LoweringLimit::RetainedCapacity,
        );
        assert_eq!(b.reservations, 2);
        assert_eq!(b.emitted, 0);
        let mut b = Budget {
            requested: REQUESTED - decl - error,
            retained: RETAINED - measured.retained,
            ..Budget::default()
        };
        assert!(lower_with_budget(&result, result.snapshot(), &mut b).is_ok());
        assert_eq!((b.requested, b.retained), (REQUESTED, RETAINED));
        let mut b = Budget {
            requested: REQUESTED - decl - error + 1,
            ..Budget::default()
        };
        limit(
            lower_with_budget(&result, result.snapshot(), &mut b),
            LoweringLimit::RequestedHeap,
        );
        assert_eq!(b.reservations, 1);
        assert_eq!(b.emitted, 0);
    }

    #[test]
    fn work_exhaustion_after_emission_returns_no_partial_result() {
        let result = with_error();
        let mut measured = Budget::default();
        assert!(lower_with_budget(&result, result.snapshot(), &mut measured).is_ok());
        assert_eq!(measured.emitted, 2);
        let mut exact = Budget {
            work: WORK - measured.work,
            ..Budget::default()
        };
        assert!(lower_with_budget(&result, result.snapshot(), &mut exact).is_ok());
        assert_eq!(exact.work, WORK);
        let mut over = Budget {
            work: WORK - measured.work + 1,
            ..Budget::default()
        };
        limit(
            lower_with_budget(&result, result.snapshot(), &mut over),
            LoweringLimit::Work,
        );
        assert_eq!(over.emitted, 2);
        assert_eq!(over.reservations, 2);
    }
}
