//! Complete exact-raw-name groups along one snapshot-local structural ancestor chain.
//!
//! These are structural candidates, without Kotlin visibility, shadowing, kind,
//! reference-position, escaped-name equivalence or resolved-symbol semantics.
//! No query heap or recursion; fixed result size does not bound peak stack/RSS/time.
//! Callers must synchronize authoritative document identity before publication.
use crate::{
    declaration_name_index::{DeclarationNameIndex, NameGroup},
    structural_regions::{Region, RegionDirectory},
};

const NAME_BYTES: usize = 65_536;
const DECLARATIONS: usize = 1_024;
const REGIONS: usize = 8_192;
const GROUPS: usize = 1_024;
const ANCESTORS: usize = 128;
const WORK: usize = 2_097_152;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AncestorLimit {
    NameBytes,
    Declarations,
    Regions,
    Groups,
    Ancestors,
    Work,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AncestorError {
    InvalidRegion,
    InvalidAssociation,
    Limit(AncestorLimit),
}
impl std::fmt::Display for AncestorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ancestor candidates rejected request: {self:?}")
    }
}
impl std::error::Error for AncestorError {}
type Result<T> = std::result::Result<T, AncestorError>;

fn cap(n: usize, maximum: usize, which: AncestorLimit) -> Result<()> {
    if n > maximum {
        Err(AncestorError::Limit(which))
    } else {
        Ok(())
    }
}
fn add(n: &mut usize, amount: usize, maximum: usize, which: AncestorLimit) -> Result<()> {
    let next = n.checked_add(amount).ok_or(AncestorError::Limit(which))?;
    cap(next, maximum, which)?;
    *n = next;
    Ok(())
}
fn group_count(declarations: usize, groups: usize) -> Result<()> {
    cap(groups, GROUPS, AncestorLimit::Groups)?;
    if groups > declarations {
        return Err(AncestorError::InvalidAssociation);
    }
    Ok(())
}
fn parent_ordinal(current: usize, parent: Option<usize>, regions: usize) -> Result<Option<usize>> {
    if current >= regions
        || match parent {
            Some(p) => p >= regions || p >= current,
            None => current != 0,
        }
    {
        return Err(AncestorError::InvalidAssociation);
    }
    Ok(parent)
}

#[derive(Default)]
struct Budget {
    work: usize,
    #[cfg(test)]
    emitted: usize,
}
impl Budget {
    fn work(&mut self, n: usize) -> Result<()> {
        add(&mut self.work, n, WORK, AncestorLimit::Work)
    }
    fn equal(&mut self, a: &str, b: &str) -> Result<bool> {
        self.work(a.len())?;
        self.work(b.len())?;
        Ok(a == b)
    }
}

/// Complete borrowed groups, nearest structural region first, with no semantic winner.
///
/// The result cannot outlive its index:
/// ```compile_fail
/// use ktlsp_parser::{ancestor_candidates::{ancestor_candidates, AncestorCandidates}, declaration_name_index::build_name_index, structural_regions::RegionDirectory};
/// fn leak<'d, 'i>(directory: &'d RegionDirectory<'i>) -> AncestorCandidates<'d, 'i> {
///     let index = build_name_index(directory).unwrap();
///     ancestor_candidates(&index, &directory.regions()[0], "x").unwrap()
/// }
/// ```
/// Nor can it outlive the directory:
/// ```compile_fail
/// use ktlsp_parser::{ancestor_candidates::{ancestor_candidates, AncestorCandidates}, declaration_name_index::DeclarationNameIndex};
/// fn leak(index: &DeclarationNameIndex<'_, '_>) -> AncestorCandidates<'static, 'static> {
///     ancestor_candidates(index, &index.directory().regions()[0], "x").unwrap()
/// }
/// ```
#[derive(Debug)]
pub struct AncestorCandidates<'a, 'i> {
    directory: &'a RegionDirectory<'i>,
    groups: [Option<NameGroup<'a, 'i>>; ANCESTORS],
    len: usize,
}
const _: () = assert!(std::mem::size_of::<AncestorCandidates<'static, 'static>>() <= 8_192);

impl<'a, 'i> AncestorCandidates<'a, 'i> {
    pub fn directory(&self) -> &'a RegionDirectory<'i> {
        self.directory
    }
    /// Groups preserve complete increasing declaration ordinals, not global source order.
    pub fn groups(&self) -> impl Iterator<Item = &NameGroup<'a, 'i>> {
        self.groups[..self.len].iter().flatten()
    }
}

/// All exact raw-name groups from the identical starting Region through file region.
///
/// Empty regions are traversed, outer matches remain after near hits, and failures
/// expose no partial result. Query/name bytes retain backticks, case and Unicode.
/// The query and start-reference binding need not survive this call:
/// ```no_run
/// use ktlsp_parser::{ancestor_candidates::{ancestor_candidates, AncestorCandidates}, declaration_name_index::DeclarationNameIndex};
/// fn lookup<'a, 'i>(index: &'a DeclarationNameIndex<'_, 'i>) -> AncestorCandidates<'a, 'i> {
///     let query = String::from("x");
///     let start = &index.directory().regions()[0];
///     ancestor_candidates(index, start, &query).unwrap()
/// }
/// ```
/// Returned ordinal slices still cannot outlive the borrowed index storage:
/// ```compile_fail
/// use ktlsp_parser::{ancestor_candidates::ancestor_candidates, declaration_name_index::DeclarationNameIndex};
/// fn leak(index: &DeclarationNameIndex<'_, '_>) -> &'static [usize] {
///     let result = ancestor_candidates(index, &index.directory().regions()[0], "x").unwrap();
///     result.groups().next().unwrap().declaration_ordinals()
/// }
/// ```
pub fn ancestor_candidates<'a, 'i>(
    index: &'a DeclarationNameIndex<'_, 'i>,
    start: &Region<'_>,
    raw_name: &str,
) -> Result<AncestorCandidates<'a, 'i>> {
    candidates_with_budget(index, start, raw_name, &mut Budget::default())
}
fn candidates_with_budget<'a, 'i>(
    index: &'a DeclarationNameIndex<'_, 'i>,
    start: &Region<'_>,
    raw_name: &str,
    b: &mut Budget,
) -> Result<AncestorCandidates<'a, 'i>> {
    b.work(10)?;
    cap(raw_name.len(), NAME_BYTES, AncestorLimit::NameBytes)?;
    let directory = index.directory();
    let regions = directory.regions();
    let declarations = directory.lowered().declarations().len();
    let groups = index.groups().len();
    cap(declarations, DECLARATIONS, AncestorLimit::Declarations)?;
    cap(regions.len(), REGIONS, AncestorLimit::Regions)?;
    group_count(declarations, groups)?;
    let mut origin = None;
    for (ordinal, candidate) in regions.iter().enumerate() {
        b.work(2)?;
        if std::ptr::eq(candidate, start) {
            origin = Some(ordinal);
            break;
        }
    }
    let mut current = origin.ok_or(AncestorError::InvalidRegion)?;
    b.work(ANCESTORS)?;
    let mut out = AncestorCandidates {
        directory,
        groups: std::array::from_fn(|_| None),
        len: 0,
    };
    let mut visited = 0;
    let mut ordinals = 0;
    loop {
        b.work(4)?;
        add(&mut visited, 1, ANCESTORS, AncestorLimit::Ancestors)?;
        let region = regions
            .get(current)
            .ok_or(AncestorError::InvalidAssociation)?;
        let parent = parent_ordinal(current, region.parent(), regions.len())?;
        let mut hit = false;
        let mut iter = index.groups();
        for _ in 0..groups {
            b.work(4)?; // Charge the lazy view's indexed reads before next().
            let group = iter.next().ok_or(AncestorError::InvalidAssociation)?;
            if std::ptr::eq(group.region(), region) && b.equal(group.raw_name(), raw_name)? {
                b.work(6)?;
                let count = group.declaration_ordinals().len();
                b.work(count)?;
                add(
                    &mut ordinals,
                    count,
                    DECLARATIONS,
                    AncestorLimit::Declarations,
                )?;
                let slot = out
                    .groups
                    .get_mut(out.len)
                    .ok_or(AncestorError::InvalidAssociation)?;
                if hit || slot.is_some() {
                    return Err(AncestorError::InvalidAssociation);
                }
                *slot = Some(group);
                out.len = out
                    .len
                    .checked_add(1)
                    .ok_or(AncestorError::InvalidAssociation)?;
                hit = true;
                #[cfg(test)]
                {
                    b.emitted += 1;
                }
            }
        }
        match parent {
            Some(parent) => current = parent,
            None => return Ok(out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Diagnostic, Element, FileKind, Parsed, SnapshotResult, SourceSnapshot,
        declaration_name_index::build_name_index, lowering::lower_declarations,
        structural_regions::build_regions,
    };
    use std::sync::Arc;

    // Same private valid CST pattern as the upstream consumer tests.
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
            self.elements.push(Element {
                kind: kind.into(),
                start: self.source.len(),
                end: 0,
                parent: self.stack.last().copied(),
                subtree_end: 0,
                token: false,
            });
            self.stack.push(self.elements.len() - 1);
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
        fn named(&mut self, name: &str, incomplete: bool) {
            self.fun(name);
            if incomplete {
                self.open("ERROR_ELEMENT");
                self.close();
                self.diagnostics.push(Diagnostic {
                    start: self.source.len(),
                    end: self.source.len(),
                    message: "missing".into(),
                });
            }
            self.close();
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
                    positions: std::sync::OnceLock::new(),
                    elements: self.elements,
                    diagnostics: self.diagnostics,
                },
            }
        }
    }
    fn sample() -> SnapshotResult {
        let mut t = Tree::new();
        t.named("x", true);
        t.named("y", false);
        t.named("x", false);
        t.finish()
    }
    fn nested() -> SnapshotResult {
        let mut t = Tree::new();
        t.named("x", false);
        t.fun("host");
        t.open("BLOCK");
        t.named("x", false);
        t.close();
        t.close();
        t.finish()
    }
    fn limit<T: std::fmt::Debug>(result: Result<T>, which: AncestorLimit) {
        assert_eq!(result.unwrap_err(), AncestorError::Limit(which));
    }

    #[test]
    fn inclusive_scalar_guards_reject_overflow_and_invalid_parents() {
        for (max, which) in [
            (NAME_BYTES, AncestorLimit::NameBytes),
            (DECLARATIONS, AncestorLimit::Declarations),
            (REGIONS, AncestorLimit::Regions),
            (GROUPS, AncestorLimit::Groups),
            (ANCESTORS, AncestorLimit::Ancestors),
            (WORK, AncestorLimit::Work),
        ] {
            assert!(cap(max, max, which).is_ok());
            limit(cap(max + 1, max, which), which);
            let mut n = max - 1;
            add(&mut n, 1, max, which).unwrap();
            limit(add(&mut n, 1, max, which), which);
            assert_eq!(n, max);
            let mut n = usize::MAX;
            limit(add(&mut n, 1, max, which), which);
            assert_eq!(n, usize::MAX);
        }
        assert!(group_count(0, 0).is_ok());
        assert!(group_count(DECLARATIONS, GROUPS).is_ok());
        assert_eq!(group_count(0, 1), Err(AncestorError::InvalidAssociation));
        limit(group_count(DECLARATIONS, GROUPS + 1), AncestorLimit::Groups);
        assert_eq!(parent_ordinal(0, None, 1), Ok(None));
        assert_eq!(parent_ordinal(2, Some(1), 3), Ok(Some(1)));
        for (current, parent, regions) in [
            (0, None, 0),
            (1, None, 2),
            (1, Some(1), 2),
            (1, Some(2), 3),
            (1, Some(2), 2),
            (0, Some(0), 1),
        ] {
            assert_eq!(
                parent_ordinal(current, parent, regions),
                Err(AncestorError::InvalidAssociation)
            );
        }
    }

    #[test]
    fn comparisons_precharge_both_complete_strings() {
        for (a, b, equal) in [("ab", "ab", true), ("ab", "zz", false), ("ab", "z", false)] {
            let mut meter = Budget::default();
            assert_eq!(meter.equal(a, b).unwrap(), equal);
            assert_eq!(meter.work, a.len() + b.len());
            let mut short = Budget {
                work: WORK - a.len() - b.len() + 1,
                ..Budget::default()
            };
            limit(short.equal(a, b), AncestorLimit::Work);
        }
        let long = "a".repeat(NAME_BYTES);
        let mut meter = Budget::default();
        for _ in 0..16 {
            assert!(meter.equal(&long, &long).unwrap());
        }
        assert_eq!(meter.work, WORK);
        limit(meter.equal(&long, "a"), AncestorLimit::Work);
    }

    #[test]
    fn duplicate_ordinals_incomplete_first_and_original_borrows_survive() {
        let result = sample();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        let candidates = {
            let query = String::from("x");
            let start = &directory.regions()[0];
            ancestor_candidates(&index, start, &query).unwrap()
        };
        assert!(std::ptr::eq(candidates.directory(), &directory));
        assert_eq!(candidates.groups().count(), 1);
        let group = candidates.groups().next().unwrap();
        let original = index.find(&directory.regions()[0], "x").unwrap().unwrap();
        assert!(std::ptr::eq(group.region(), original.region()));
        assert!(std::ptr::eq(group.raw_name(), original.raw_name()));
        assert!(std::ptr::eq(
            group.declaration_ordinals(),
            original.declaration_ordinals()
        ));
        assert_eq!(group.declaration_ordinals(), [0, 2]);
        assert_eq!(
            group
                .declaration_ordinals()
                .iter()
                .map(|&i| lower.declarations()[i].incomplete())
                .collect::<Vec<_>>(),
            [true, false]
        );
        assert_eq!(
            (
                lower.errors()[0].range(),
                lower.errors()[0].owner(),
                lower.errors()[0].message()
            ),
            (5..5, Some(0), "missing")
        );
    }

    #[test]
    fn raw_case_backticks_and_unicode_are_not_equivalent() {
        let names = ["x", "`x`", "X", "`é`", "`e\u{301}`"];
        let mut t = Tree::new();
        for name in names {
            t.named(name, false);
        }
        let result = t.finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        for (ordinal, name) in names.iter().enumerate() {
            let found = ancestor_candidates(&index, &directory.regions()[0], name).unwrap();
            assert_eq!(
                found.groups().next().unwrap().declaration_ordinals(),
                [ordinal]
            );
            assert_eq!(found.groups().count(), 1);
        }
        for name in ["", "\0", "x\0", "é", "e\u{301}", "`X`"] {
            assert_eq!(
                ancestor_candidates(&index, &directory.regions()[0], name)
                    .unwrap()
                    .groups()
                    .count(),
                0
            );
        }
    }

    #[test]
    fn all_ancestor_hits_follow_structural_order_and_one_exact_meter() {
        let result = nested();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        let mut meter = Budget::default();
        let candidates =
            candidates_with_budget(&index, &directory.regions()[1], "x", &mut meter).unwrap();
        assert_eq!(
            candidates
                .groups()
                .map(|g| g.declaration_ordinals())
                .collect::<Vec<_>>(),
            [&[2][..], &[0][..]]
        );
        // 10 metadata + 4 origin + 128 slots + 8 ancestors + 24 views + 9 name bytes + 14 hit units.
        assert_eq!(meter.work, 197);
        assert_eq!(meter.emitted, 2);
        let mut exact = Budget {
            work: WORK - 197,
            ..Budget::default()
        };
        assert!(candidates_with_budget(&index, &directory.regions()[1], "x", &mut exact).is_ok());
        assert_eq!(exact.work, WORK);
        let mut short = Budget {
            work: WORK - 197 + 1,
            ..Budget::default()
        };
        limit(
            candidates_with_budget(&index, &directory.regions()[1], "x", &mut short),
            AncestorLimit::Work,
        );
        assert_eq!(short.emitted, 2);
        let mut during_outer = Budget {
            work: WORK - 171,
            ..Budget::default()
        };
        limit(
            candidates_with_budget(&index, &directory.regions()[1], "x", &mut during_outer),
            AncestorLimit::Work,
        );
        assert_eq!(during_outer.emitted, 1);
        assert_eq!(
            ancestor_candidates(&index, &directory.regions()[1], "x")
                .unwrap()
                .groups()
                .count(),
            2
        );
        let mut absent = Budget::default();
        assert_eq!(
            candidates_with_budget(&index, &directory.regions()[1], "z", &mut absent)
                .unwrap()
                .groups()
                .count(),
            0
        );
        assert_eq!(absent.work, 183);
        let file = ancestor_candidates(&index, &directory.regions()[0], "x").unwrap();
        assert_eq!(file.groups().next().unwrap().declaration_ordinals(), [0]);
        assert_eq!(file.groups().count(), 1);
    }

    #[test]
    fn empty_indexes_still_validate_origin_traverse_and_apply_query_precedence() {
        let result = Tree::new().finish();
        let other_result = Tree::new().finish();
        assert_eq!(result.snapshot(), other_result.snapshot());
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let other_lower = lower_declarations(&other_result, other_result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let same_inputs = build_regions(&lower, &result, result.snapshot()).unwrap();
        let other = build_regions(&other_lower, &other_result, other_result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        let second = build_name_index(&directory).unwrap();
        for own in [&index, &second] {
            let mut meter = Budget::default();
            assert_eq!(
                candidates_with_budget(own, &directory.regions()[0], "", &mut meter)
                    .unwrap()
                    .groups()
                    .count(),
                0
            );
            assert_eq!(meter.work, 144);
            let mut short = Budget {
                work: WORK - 143,
                ..Budget::default()
            };
            limit(
                candidates_with_budget(own, &directory.regions()[0], "", &mut short),
                AncestorLimit::Work,
            );
            for foreign in [&same_inputs.regions()[0], &other.regions()[0]] {
                let mut meter = Budget::default();
                assert_eq!(
                    candidates_with_budget(own, foreign, "x", &mut meter).unwrap_err(),
                    AncestorError::InvalidRegion
                );
                assert_eq!(meter.work, 12);
                limit(
                    ancestor_candidates(own, foreign, &"a".repeat(NAME_BYTES + 1)),
                    AncestorLimit::NameBytes,
                );
            }
            assert_eq!(
                ancestor_candidates(own, &directory.regions()[0], &"a".repeat(NAME_BYTES))
                    .unwrap()
                    .groups()
                    .count(),
                0
            );
        }
    }

    #[test]
    fn failure_in_empty_outer_region_discards_the_inner_hit() {
        let mut t = Tree::new();
        t.fun("host");
        t.open("BLOCK");
        t.fun("inner");
        t.open("BLOCK");
        t.named("x", false);
        t.close();
        t.close();
        t.close();
        t.close();
        let result = t.finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        let mut meter = Budget {
            work: WORK - 177,
            ..Budget::default()
        };
        limit(
            candidates_with_budget(&index, &directory.regions()[2], "x", &mut meter),
            AncestorLimit::Work,
        );
        assert_eq!(meter.emitted, 1);
        let found = ancestor_candidates(&index, &directory.regions()[2], "x").unwrap();
        assert_eq!(found.groups().next().unwrap().declaration_ordinals(), [2]);
        assert_eq!(found.groups().count(), 1);
    }
}
