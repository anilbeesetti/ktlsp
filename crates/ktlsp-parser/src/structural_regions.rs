//! Finite structural regions and ordered declaration membership, without lookup semantics.
//!
//! Requires successful lowering from the same immutable result. Bounds cover only
//! incremental consumer work/storage, not retained inputs, RSS, time or publication.
use crate::{
    Element, FileKind, Parsed, SnapshotResult, SourceSnapshot, lowering::LoweredDeclarations,
};
use std::{mem::size_of, ops::Range};

const DOCUMENT: usize = 8_192;
const SOURCE: usize = 65_536;
const ELEMENTS: usize = 16_384;
const NODES: usize = 8_192;
const TOKENS: usize = 8_192;
const KIND_BYTES: usize = 64;
const DECLARATIONS: usize = 1_024;
const REGIONS: usize = 8_192;
const DEPTH: usize = 128;
const WORK: usize = 2_097_152;
const REQUESTED: usize = 131_072;
const RETAINED: usize = 131_072;
const STACK_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionLimit {
    Document,
    Source,
    Elements,
    Nodes,
    Tokens,
    KindBytes,
    Declarations,
    Regions,
    Depth,
    Work,
    RequestedHeap,
    RetainedCapacity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionError {
    InvalidIdentity,
    InvalidOrigin,
    StaleSnapshot,
    UnsupportedFileKind,
    InvalidAssociation { element: Option<usize> },
    Limit(RegionLimit),
    Allocation,
}
impl std::fmt::Display for RegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "structural regions rejected request: {self:?}")
    }
}
impl std::error::Error for RegionError {}
type Result<T> = std::result::Result<T, RegionError>;
fn invalid(i: usize) -> RegionError {
    RegionError::InvalidAssociation { element: Some(i) }
}
fn cap(n: usize, max: usize, limit: RegionLimit) -> Result<()> {
    if n > max {
        Err(RegionError::Limit(limit))
    } else {
        Ok(())
    }
}
fn add(n: &mut usize, amount: usize, max: usize, limit: RegionLimit) -> Result<()> {
    let next = n.checked_add(amount).ok_or(RegionError::Limit(limit))?;
    cap(next, max, limit)?;
    *n = next;
    Ok(())
}
fn bytes(count: usize, width: usize, limit: RegionLimit) -> Result<usize> {
    count.checked_mul(width).ok_or(RegionError::Limit(limit))
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
    before_emit: usize,
    #[cfg(test)]
    first_emission: usize,
    #[cfg(test)]
    emitted: usize,
}
impl Budget {
    fn work(&mut self, n: usize) -> Result<()> {
        add(&mut self.work, n, WORK, RegionLimit::Work)
    }
    fn equal(&mut self, a: &str, b: &str) -> Result<bool> {
        self.work(a.len())?;
        self.work(b.len())?;
        Ok(a == b)
    }
    fn one_of(&mut self, text: &str, choices: &[&str]) -> Result<bool> {
        for word in choices {
            self.work(text.len())?;
            self.work(word.len())?;
        }
        Ok(choices.contains(&text))
    }
    fn emission(&mut self) -> Result<()> {
        self.work(2)?;
        #[cfg(test)]
        {
            if self.emitted == 0 {
                self.first_emission = self.work;
            }
            self.emitted += 1;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RegionDirectory<'a> {
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    parsed: &'a Parsed,
    regions: Vec<Region<'a>>,
    memberships: Vec<Membership>,
}
impl<'a> RegionDirectory<'a> {
    pub fn snapshot(&self) -> &SourceSnapshot {
        self.result.snapshot()
    }
    pub fn lowered(&self) -> &LoweredDeclarations<'a> {
        self.lowered
    }
    pub fn regions(&self) -> &[Region<'a>] {
        &self.regions
    }
    pub fn memberships(&self) -> &[Membership] {
        &self.memberships
    }
}
#[derive(Debug)]
pub struct Region<'a> {
    cst_index: usize,
    node: &'a Element,
    parent: Option<usize>,
    introducer: Option<usize>,
}
impl Region<'_> {
    pub fn cst_index(&self) -> usize {
        self.cst_index
    }
    pub fn kind(&self) -> &str {
        &self.node.kind
    }
    pub fn range(&self) -> Range<usize> {
        self.node.start..self.node.end
    }
    pub fn subtree_end(&self) -> usize {
        self.node.subtree_end
    }
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }
    pub fn introducer(&self) -> Option<usize> {
        self.introducer
    }
}
/// Vector position is the corresponding lowerer declaration ordinal.
#[derive(Debug)]
pub struct Membership {
    region: usize,
}
impl Membership {
    pub fn region(&self) -> usize {
        self.region
    }
}

#[derive(Clone, Copy)]
struct Frame {
    index: usize,
    subtree_end: usize,
    region: Option<usize>,
    selected: Option<usize>,
}
impl Frame {
    const EMPTY: Self = Self {
        index: 0,
        subtree_end: 0,
        region: None,
        selected: None,
    };
}
const _: () = assert!(size_of::<Frame>() <= 128);
const _: () = assert!(size_of::<[Frame; DEPTH]>() <= STACK_BYTES);
fn frame(s: &[Frame; DEPTH], i: usize, b: &mut Budget) -> Result<Frame> {
    b.work(1)?;
    s.get(i).copied().ok_or(invalid(i))
}
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    nodes: usize,
    tokens: usize,
    declarations: usize,
    regions: usize,
}

/// Build result-local structural membership. Recheck against the authoritative
/// document store, under its synchronization, before publishing any result.
pub fn build_regions<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
) -> Result<RegionDirectory<'a>> {
    build_with_budget(lowered, result, current, &mut Budget::default())
}
fn build_with_budget<'a>(
    lowered: &'a LoweredDeclarations<'a>,
    result: &'a SnapshotResult,
    current: &SourceSnapshot,
    b: &mut Budget,
) -> Result<RegionDirectory<'a>> {
    let snapshot = result.snapshot();
    for key in [snapshot.document(), current.document()] {
        cap(key.len(), DOCUMENT, RegionLimit::Document)?;
    }
    for source in [snapshot.source(), current.source()] {
        cap(source.len(), SOURCE, RegionLimit::Source)?;
    }
    for key in [snapshot.document(), current.document()] {
        if key.is_empty() {
            return Err(RegionError::InvalidIdentity);
        }
        for i in 0..key.len() {
            b.work(2)?;
            if key.as_bytes().get(i) == Some(&0) {
                return Err(RegionError::InvalidIdentity);
            }
        }
    }
    // Equal source bytes from a second parse do not establish structural origin.
    if !std::ptr::eq(lowered.snapshot(), snapshot) {
        return Err(RegionError::InvalidOrigin);
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
        .map_err(|_| RegionError::StaleSnapshot)?;
    if snapshot.kind() != FileKind::Source {
        return Err(RegionError::UnsupportedFileKind);
    }
    cap(parsed.source().len(), SOURCE, RegionLimit::Source)?;
    b.work(1)?;
    if !b.equal(parsed.source(), snapshot.source())? {
        return Err(RegionError::InvalidAssociation { element: None });
    }
    cap(parsed.elements().len(), ELEMENTS, RegionLimit::Elements)?;
    cap(
        lowered.declarations().len(),
        DECLARATIONS,
        RegionLimit::Declarations,
    )?;
    let mut out = RegionDirectory {
        lowered,
        result,
        parsed,
        regions: Vec::new(),
        memberships: Vec::new(),
    };
    b.work(DEPTH)?;
    let mut stack = [Frame::EMPTY; DEPTH];
    let counts = walk(&mut out, &mut stack, b, false)?;
    reserve(&mut out.regions, counts.regions, b)?;
    reserve(&mut out.memberships, counts.declarations, b)?;
    #[cfg(test)]
    {
        b.before_emit = b.work;
    }
    let emitted = walk(&mut out, &mut stack, b, true)?;
    if emitted != counts
        || out.regions.len() != counts.regions
        || out.memberships.len() != counts.declarations
    {
        return Err(RegionError::InvalidAssociation { element: None });
    }
    Ok(out)
}
fn reserve<T>(v: &mut Vec<T>, count: usize, b: &mut Budget) -> Result<()> {
    if count == 0 {
        return Ok(());
    }
    add(
        &mut b.requested,
        bytes(count, size_of::<T>(), RegionLimit::RequestedHeap)?,
        REQUESTED,
        RegionLimit::RequestedHeap,
    )?;
    #[cfg(not(test))]
    let reserve = count;
    #[cfg(test)]
    let reserve = {
        let site = b.reservations;
        b.reservations += 1;
        if b.fail_reservation == Some(site) {
            usize::MAX
        } else {
            count
                .checked_add(*b.extra_capacity.get(site).ok_or(RegionError::Allocation)?)
                .ok_or(RegionError::Allocation)?
        }
    };
    v.try_reserve_exact(reserve)
        .map_err(|_| RegionError::Allocation)?;
    add(
        &mut b.retained,
        bytes(v.capacity(), size_of::<T>(), RegionLimit::RetainedCapacity)?,
        RETAINED,
        RegionLimit::RetainedCapacity,
    )
}

fn close(s: &[Frame; DEPTH], depth: &mut usize, i: usize, b: &mut Budget) -> Result<()> {
    while *depth != 0 {
        let f = frame(s, *depth - 1, b)?;
        if f.subtree_end > i {
            break;
        }
        if f.subtree_end != i {
            return Err(invalid(f.index));
        }
        b.work(1)?;
        *depth -= 1;
    }
    Ok(())
}
fn body_parent(kind: &str, i: usize, b: &mut Budget) -> Result<&'static str> {
    for candidate in ["CLASS_BODY", "BLOCK"] {
        b.work(kind.len())?;
        b.work(candidate.len())?;
    }
    match kind {
        "CLASS_BODY" => Ok("CLASS"),
        "BLOCK" => Ok("FUN"),
        _ => Err(invalid(i)),
    }
}
fn walk<'a>(
    out: &mut RegionDirectory<'a>,
    s: &mut [Frame; DEPTH],
    b: &mut Budget,
    emit: bool,
) -> Result<Counts> {
    let p = out.parsed;
    let len = p.elements().len();
    if len == 0 {
        return Err(RegionError::InvalidAssociation { element: None });
    }
    let mut counts = Counts::default();
    let mut depth = 0usize;
    for i in 0..len {
        close(s, &mut depth, i, b)?;
        b.work(2)?;
        let e = p.elements().get(i).ok_or(invalid(i))?;
        if !emit {
            if e.token {
                add(&mut counts.tokens, 1, TOKENS, RegionLimit::Tokens)?;
            } else {
                add(&mut counts.nodes, 1, NODES, RegionLimit::Nodes)?;
            }
            cap(e.kind.len(), KIND_BYTES, RegionLimit::KindBytes)?;
        } else if e.token {
            counts.tokens = counts
                .tokens
                .checked_add(1)
                .ok_or(RegionError::Limit(RegionLimit::Tokens))?;
        } else {
            counts.nodes = counts
                .nodes
                .checked_add(1)
                .ok_or(RegionError::Limit(RegionLimit::Nodes))?;
        }
        let parent = if depth == 0 {
            None
        } else {
            Some(frame(s, depth - 1, b)?)
        };
        if e.subtree_end <= i
            || e.subtree_end > len
            || e.start > e.end
            || e.end > p.source().len()
            || e.parent != parent.map(|f| f.index)
            || (i != 0 && parent.is_none())
            || parent.is_some_and(|f| e.subtree_end > f.subtree_end)
        {
            return Err(invalid(i));
        }
        let next_depth = depth
            .checked_add(1)
            .ok_or(RegionError::Limit(RegionLimit::Depth))?;
        cap(next_depth, DEPTH, RegionLimit::Depth)?;
        if i == 0
            && (e.token
                || e.parent.is_some()
                || e.start != 0
                || e.end != p.source().len()
                || e.subtree_end != len
                || !b.equal(&e.kind, "KT_FILE")?)
        {
            return Err(invalid(i));
        }
        if e.token {
            if e.subtree_end != i + 1 {
                return Err(invalid(i));
            }
            continue;
        }
        let selected = b.one_of(
            &e.kind,
            &[
                "CLASS",
                "FUN",
                "PROPERTY",
                "VALUE_PARAMETER",
                "TYPE_PARAMETER",
            ],
        )?;
        let is_region = b.one_of(&e.kind, &["KT_FILE", "CLASS_BODY", "BLOCK"])?;
        let inherited = parent.and_then(|f| f.region);
        let declaration = if selected {
            let ordinal = counts.declarations;
            add(
                &mut counts.declarations,
                1,
                DECLARATIONS,
                RegionLimit::Declarations,
            )?;
            b.work(1)?;
            let d = out.lowered.declarations().get(ordinal).ok_or(invalid(i))?;
            if d.cst_index() != i
                || !b.equal(d.kind(), &e.kind)?
                || d.range() != (e.start..e.end)
                || d.cst_parent_index() != e.parent
            {
                return Err(invalid(i));
            }
            let region = inherited.ok_or(invalid(i))?;
            if region >= counts.regions {
                return Err(invalid(i));
            }
            let parent = parent.ok_or(invalid(i))?;
            b.work(1)?;
            let pe = p.elements().get(parent.index).ok_or(invalid(i))?;
            let value_parameter = b.equal(&e.kind, "VALUE_PARAMETER")?;
            if value_parameter || b.equal(&e.kind, "TYPE_PARAMETER")? {
                let function_index = pe.parent.ok_or(invalid(i))?;
                b.work(2)?;
                let function = p.elements().get(function_index).ok_or(invalid(i))?;
                let owner = d
                    .owner()
                    .filter(|owner| *owner < ordinal)
                    .and_then(|owner| out.lowered.declarations().get(owner))
                    .ok_or(invalid(i))?;
                let ordinary_owner = if function.parent == Some(0) {
                    true
                } else if value_parameter {
                    b.work(3)?;
                    let block = function.parent.and_then(|index| p.elements().get(index)).ok_or(invalid(i))?;
                    let outer = block.parent.and_then(|index| p.elements().get(index)).ok_or(invalid(i))?;
                    !block.token && b.equal(&block.kind, "BLOCK")?
                        && !outer.token && b.equal(&outer.kind, "FUN")? && outer.parent == Some(0)
                } else {
                    false
                };
                if pe.token
                    || !b.equal(
                        &pe.kind,
                        if value_parameter {
                            "VALUE_PARAMETER_LIST"
                        } else {
                            "TYPE_PARAMETER_LIST"
                        },
                    )?
                    || function.token
                    || !ordinary_owner
                    || !b.equal(&function.kind, "FUN")?
                    || owner.cst_index() != function_index
                    || !b.equal(owner.kind(), "FUN")?
                {
                    return Err(invalid(i));
                }
            } else if !b.one_of(&pe.kind, &["KT_FILE", "CLASS_BODY", "BLOCK"])? {
                return Err(invalid(i));
            }
            if emit {
                if out.memberships.len() != ordinal
                    || out.memberships.len() >= out.memberships.capacity()
                {
                    return Err(invalid(i));
                }
                b.emission()?;
                out.memberships.push(Membership { region });
            }
            Some(ordinal)
        } else {
            None
        };
        let region = if is_region {
            let ordinal = counts.regions;
            add(&mut counts.regions, 1, REGIONS, RegionLimit::Regions)?;
            let introducer = if i == 0 {
                None
            } else {
                let parent = parent.ok_or(invalid(i))?;
                b.work(1)?;
                let pe = p.elements().get(parent.index).ok_or(invalid(i))?;
                let keyword = body_parent(&e.kind, i, b)?;
                if !b.equal(&pe.kind, keyword)? || inherited.is_none_or(|r| r >= ordinal) {
                    return Err(invalid(i));
                }
                Some(parent.selected.ok_or(invalid(i))?)
            };
            if emit {
                if out.regions.len() != ordinal || out.regions.len() >= out.regions.capacity() {
                    return Err(invalid(i));
                }
                b.emission()?;
                out.regions.push(Region {
                    cst_index: i,
                    node: e,
                    parent: inherited,
                    introducer,
                });
            }
            Some(ordinal)
        } else {
            inherited
        };
        b.work(2)?;
        *s.get_mut(depth).ok_or(invalid(i))? = Frame {
            index: i,
            subtree_end: e.subtree_end,
            region,
            selected: declaration,
        };
        depth = next_depth;
    }
    close(s, &mut depth, len, b)?;
    if depth != 0 || counts.declarations != out.lowered.declarations().len() {
        return Err(RegionError::InvalidAssociation { element: None });
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagnostic, lowering::lower_declarations};
    use std::sync::Arc;

    // Private synthetic shapes exercise consumer boundaries, not production parsing.
    struct Tree {
        source: String,
        elements: Vec<Element>,
        diagnostics: Vec<Diagnostic>,
        stack: Vec<usize>,
    }
    impl Tree {
        fn new() -> Self {
            let mut t = Self {
                source: String::new(),
                elements: vec![],
                diagnostics: vec![],
                stack: vec![],
            };
            t.open("KT_FILE");
            t
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
        fn fun(&mut self, name: &str) {
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
                snapshot: SourceSnapshot::new("synthetic", 1, FileKind::Source, source.clone())
                    .unwrap(),
                parsed: Parsed {
                    source,
                    elements: self.elements,
                    diagnostics: self.diagnostics,
                },
            }
        }
    }
    fn nested() -> SnapshotResult {
        let mut t = Tree::new();
        t.fun("same");
        t.open("BLOCK");
        t.token("LBRACE", "{");
        t.fun("same");
        t.open("BLOCK");
        t.token("LBRACE", "{");
        t.token("RBRACE", "}");
        t.close();
        t.close();
        t.token("RBRACE", "}");
        t.close();
        t.close();
        t.finish()
    }
    fn clone_result(r: &SnapshotResult) -> SnapshotResult {
        SnapshotResult {
            snapshot: r.snapshot.clone(),
            parsed: Parsed {
                source: r.parsed.source.clone(),
                elements: r.parsed.elements.clone(),
                diagnostics: r.parsed.diagnostics.clone(),
            },
        }
    }
    fn limit<T: std::fmt::Debug>(r: Result<T>, expected: RegionLimit) {
        assert_eq!(r.unwrap_err(), RegionError::Limit(expected));
    }
    fn private_walk<'a>(
        parsed: &'a Parsed,
        lower: &'a LoweredDeclarations<'a>,
        result: &'a SnapshotResult,
    ) -> Result<Counts> {
        let mut out = RegionDirectory {
            lowered: lower,
            result,
            parsed,
            regions: vec![],
            memberships: vec![],
        };
        walk(
            &mut out,
            &mut [Frame::EMPTY; DEPTH],
            &mut Budget::default(),
            false,
        )
    }

    #[test]
    fn duplicate_memberships_and_empty_nested_regions_preserve_order() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let out = build_regions(&lower, &result, &result.snapshot.clone()).unwrap();
        assert!(std::ptr::eq(out.snapshot(), result.snapshot()));
        assert!(std::ptr::eq(out.lowered(), &lower));
        assert_eq!(
            out.regions()
                .iter()
                .map(|r| (r.kind(), r.parent(), r.introducer()))
                .collect::<Vec<_>>(),
            [
                ("KT_FILE", None, None),
                ("BLOCK", Some(0), Some(0)),
                ("BLOCK", Some(1), Some(1))
            ]
        );
        assert_eq!(
            out.memberships()
                .iter()
                .map(Membership::region)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(out.regions().get(3).is_none());
        assert!(out.memberships().get(2).is_none());
        assert_eq!(
            lower.declarations()[0].raw_name(),
            lower.declarations()[1].raw_name()
        );
        assert_eq!(out.regions()[2].range().len(), 2);
    }

    #[test]
    fn same_content_different_origin_and_stale_current_are_rejected() {
        let result = nested();
        let other = clone_result(&result);
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        assert_eq!(
            build_regions(&lower, &other, other.snapshot()).unwrap_err(),
            RegionError::InvalidOrigin
        );
        for current in [
            SourceSnapshot::new("other", 1, FileKind::Source, result.snapshot.source()).unwrap(),
            SourceSnapshot::new("synthetic", 2, FileKind::Source, result.snapshot.source())
                .unwrap(),
            SourceSnapshot::new("synthetic", 1, FileKind::Script, result.snapshot.source())
                .unwrap(),
            SourceSnapshot::new("synthetic", 1, FileKind::Source, "changed").unwrap(),
        ] {
            assert_eq!(
                build_regions(&lower, &result, &current).unwrap_err(),
                RegionError::StaleSnapshot
            );
        }
        for key in ["", "x\0y"] {
            let current = SourceSnapshot::new(key, 1, FileKind::Source, "").unwrap();
            assert_eq!(
                build_regions(&lower, &result, &current).unwrap_err(),
                RegionError::InvalidIdentity
            );
        }
    }

    #[test]
    fn every_counter_is_inclusive_and_overflow_rejects() {
        for (max, kind) in [
            (DOCUMENT, RegionLimit::Document),
            (SOURCE, RegionLimit::Source),
            (ELEMENTS, RegionLimit::Elements),
            (NODES, RegionLimit::Nodes),
            (TOKENS, RegionLimit::Tokens),
            (KIND_BYTES, RegionLimit::KindBytes),
            (DECLARATIONS, RegionLimit::Declarations),
            (REGIONS, RegionLimit::Regions),
            (DEPTH, RegionLimit::Depth),
            (WORK, RegionLimit::Work),
            (REQUESTED, RegionLimit::RequestedHeap),
            (RETAINED, RegionLimit::RetainedCapacity),
        ] {
            let mut n = max - 1;
            add(&mut n, 1, max, kind).unwrap();
            assert_eq!(n, max);
            limit(add(&mut n, 1, max, kind), kind);
            assert_eq!(n, max);
            let mut overflow = usize::MAX;
            limit(add(&mut overflow, 1, max, kind), kind);
        }
        limit(
            bytes(usize::MAX, 2, RegionLimit::RequestedHeap),
            RegionLimit::RequestedHeap,
        );
        limit(
            bytes(usize::MAX, 2, RegionLimit::RetainedCapacity),
            RegionLimit::RetainedCapacity,
        );
    }

    #[test]
    fn identity_caps_and_comparisons_are_funded_before_use() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let mut current = result.snapshot.clone();
        current.document = Arc::from("x".repeat(DOCUMENT + 1));
        limit(
            build_regions(&lower, &result, &current),
            RegionLimit::Document,
        );
        current = result.snapshot.clone();
        current.source = Arc::from("x".repeat(SOURCE + 1));
        limit(
            build_regions(&lower, &result, &current),
            RegionLimit::Source,
        );
        let mut b = Budget {
            work: WORK - 1,
            ..Budget::default()
        };
        limit(
            build_with_budget(&lower, &result, result.snapshot(), &mut b),
            RegionLimit::Work,
        );
        assert_eq!(b.reservations, 0);
        let mut b = Budget {
            work: WORK - 4,
            ..Budget::default()
        };
        limit(b.equal("abc", "de"), RegionLimit::Work);
        assert_eq!(b.work, WORK - 1);
        let mut b = Budget {
            work: WORK - 5,
            ..Budget::default()
        };
        assert!(!b.equal("abc", "de").unwrap());
        assert_eq!(b.work, WORK);
        for (kind, parent) in [("CLASS_BODY", "CLASS"), ("BLOCK", "FUN")] {
            let cost = 2 * kind.len() + "CLASS_BODY".len() + "BLOCK".len();
            let mut b = Budget {
                work: WORK - cost,
                ..Budget::default()
            };
            assert_eq!(body_parent(kind, 7, &mut b).unwrap(), parent);
            assert_eq!(b.work, WORK);
            let mut b = Budget {
                work: WORK - cost + 1,
                ..Budget::default()
            };
            limit(body_parent(kind, 7, &mut b), RegionLimit::Work);
        }
    }

    #[test]
    fn malformed_structure_and_missing_declarations_reject() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        for (index, mode) in [(1, 0), (1, 1), (1, 2), (0, 3), (0, 4)] {
            let mut other = clone_result(&result);
            match mode {
                0 => other.parsed.elements[index].parent = None,
                1 => other.parsed.elements[index].subtree_end = usize::MAX,
                2 => other.parsed.elements[index].start = usize::MAX,
                3 => other.parsed.elements[index].kind = "BLOCK".into(),
                _ => other.parsed.elements[index].token = true,
            }
            assert!(matches!(
                private_walk(&other.parsed, &lower, &result),
                Err(RegionError::InvalidAssociation { .. })
            ));
        }
        let empty = Tree::new().finish();
        assert!(matches!(
            private_walk(&empty.parsed, &lower, &result),
            Err(RegionError::InvalidAssociation { element: None })
        ));
        let empty_lower = lower_declarations(&empty, empty.snapshot()).unwrap();
        assert!(matches!(
            private_walk(&result.parsed, &empty_lower, &empty),
            Err(RegionError::InvalidAssociation { element: Some(1) })
        ));
    }

    #[test]
    fn token_and_empty_node_depth_boundaries_are_checked() {
        let empty = Tree::new().finish();
        let lower = lower_declarations(&empty, empty.snapshot()).unwrap();
        for token in [false, true] {
            for over in [false, true] {
                let mut t = Tree::new();
                let levels = DEPTH - 1 - usize::from(token) + usize::from(over);
                for _ in 0..levels {
                    t.open("ERROR_ELEMENT");
                }
                if token {
                    t.token("IDENTIFIER", "x");
                }
                for _ in 0..levels {
                    t.close();
                }
                let r = t.finish();
                let outcome = private_walk(&r.parsed, &lower, &empty);
                if over {
                    limit(outcome, RegionLimit::Depth);
                } else {
                    outcome.unwrap();
                }
            }
        }
    }

    #[test]
    fn count_kind_and_body_association_limits_reject() {
        let empty = Tree::new().finish();
        let lower = lower_declarations(&empty, empty.snapshot()).unwrap();
        let mut t = Tree::new();
        for _ in 0..NODES {
            t.open("ERROR_ELEMENT");
            t.close();
        }
        limit(
            private_walk(&t.finish().parsed, &lower, &empty),
            RegionLimit::Nodes,
        );
        let mut t = Tree::new();
        for _ in 0..=TOKENS {
            t.token("WHITE_SPACE", " ");
        }
        limit(
            private_walk(&t.finish().parsed, &lower, &empty),
            RegionLimit::Tokens,
        );
        let mut t = Tree::new();
        t.open(&"x".repeat(KIND_BYTES + 1));
        t.close();
        limit(
            private_walk(&t.finish().parsed, &lower, &empty),
            RegionLimit::KindBytes,
        );
        for kind in ["CLASS_BODY", "BLOCK", "KT_FILE"] {
            let mut t = Tree::new();
            t.open(kind);
            t.close();
            assert!(matches!(
                private_walk(&t.finish().parsed, &lower, &empty),
                Err(RegionError::InvalidAssociation { element: Some(1) })
            ));
        }
    }

    #[test]
    fn both_actual_reservation_failures_return_no_directory() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        for site in [0, 1] {
            let mut b = Budget {
                fail_reservation: Some(site),
                ..Budget::default()
            };
            assert_eq!(
                build_with_budget(&lower, &result, result.snapshot(), &mut b).unwrap_err(),
                RegionError::Allocation
            );
            assert_eq!(b.reservations, site + 1);
            assert_eq!(b.emitted, 0);
        }
        let empty = Tree::new().finish();
        let lower = lower_declarations(&empty, empty.snapshot()).unwrap();
        let mut b = Budget::default();
        assert!(
            build_with_budget(&lower, &empty, empty.snapshot(), &mut b)
                .unwrap()
                .memberships()
                .is_empty()
        );
        assert_eq!(b.reservations, 1);
    }

    #[test]
    fn requested_and_actual_capacity_limits_precede_emission() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        for site in [0, 1] {
            let mut b = Budget {
                requested: REQUESTED,
                ..Budget::default()
            };
            if site == 1 {
                b.requested -= 3 * size_of::<Region<'_>>();
            }
            limit(
                build_with_budget(&lower, &result, result.snapshot(), &mut b),
                RegionLimit::RequestedHeap,
            );
            assert_eq!(b.reservations, site);
            assert_eq!(b.emitted, 0);
            let mut b = Budget::default();
            b.extra_capacity[site] = RETAINED;
            limit(
                build_with_budget(&lower, &result, result.snapshot(), &mut b),
                RegionLimit::RetainedCapacity,
            );
            assert_eq!(b.reservations, site + 1);
            assert_eq!(b.emitted, 0);
        }
    }

    #[test]
    fn work_exhaustion_after_partial_emission_returns_only_error() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let mut full = Budget::default();
        build_with_budget(&lower, &result, result.snapshot(), &mut full).unwrap();
        assert!(full.first_emission > full.before_emit);
        let mut b = Budget {
            work: WORK - full.first_emission,
            ..Budget::default()
        };
        limit(
            build_with_budget(&lower, &result, result.snapshot(), &mut b),
            RegionLimit::Work,
        );
        assert_eq!(b.reservations, 2);
        assert_eq!(b.emitted, 1);
    }
}
