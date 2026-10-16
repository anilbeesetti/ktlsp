//! Bounded exact-raw-name groups within one borrowed structural directory.
//!
//! Ordinals are snapshot-local declaration references through `directory()`, never
//! stable or resolved symbols. No visibility, shadowing, imports or ancestor lookup.
//! Bounds cover incremental work/capacities, not borrowed inputs, RSS or time.
//! Callers must synchronize current document identity before publishing results.
use crate::structural_regions::{Region, RegionDirectory};
use std::mem::size_of;

const DECLARATIONS: usize = 1_024;
const REGIONS: usize = 8_192;
const NAME_BYTES: usize = 65_536;
const WORK: usize = 2_097_152;
const REQUESTED: usize = 131_072;
const RETAINED: usize = 131_072;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameIndexLimit {
    Declarations,
    Regions,
    NameBytes,
    Work,
    RequestedHeap,
    RetainedCapacity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameIndexError {
    InvalidRegion,
    InvalidAssociation,
    Limit(NameIndexLimit),
    Allocation,
}
impl std::fmt::Display for NameIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "declaration name index rejected request: {self:?}")
    }
}
impl std::error::Error for NameIndexError {}
type Result<T> = std::result::Result<T, NameIndexError>;
fn cap(n: usize, maximum: usize, which: NameIndexLimit) -> Result<()> {
    if n > maximum {
        Err(NameIndexError::Limit(which))
    } else {
        Ok(())
    }
}
fn add(n: &mut usize, amount: usize, maximum: usize, which: NameIndexLimit) -> Result<()> {
    let next = n.checked_add(amount).ok_or(NameIndexError::Limit(which))?;
    cap(next, maximum, which)?;
    *n = next;
    Ok(())
}
fn bytes(count: usize, width: usize, which: NameIndexLimit) -> Result<usize> {
    count.checked_mul(width).ok_or(NameIndexError::Limit(which))
}
fn association_length(declarations: usize, memberships: usize) -> Result<()> {
    if declarations != memberships {
        Err(NameIndexError::InvalidAssociation)
    } else {
        Ok(())
    }
}
fn association_region(region: usize, regions: usize) -> Result<()> {
    if region >= regions {
        Err(NameIndexError::InvalidAssociation)
    } else {
        Ok(())
    }
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
        add(&mut self.work, n, WORK, NameIndexLimit::Work)
    }
    fn equal(&mut self, a: &str, b: &str) -> Result<bool> {
        self.work(a.len())?;
        self.work(b.len())?;
        Ok(a == b)
    }
    fn emission(&mut self) -> Result<()> {
        self.work(1)?;
        #[cfg(test)]
        {
            self.emitted += 1;
        }
        Ok(())
    }
}
fn reserve<T>(v: &mut Vec<T>, count: usize, b: &mut Budget) -> Result<()> {
    if count == 0 {
        return Ok(());
    }
    add(
        &mut b.requested,
        bytes(count, size_of::<T>(), NameIndexLimit::RequestedHeap)?,
        REQUESTED,
        NameIndexLimit::RequestedHeap,
    )?;
    #[cfg(not(test))]
    let requested = count;
    #[cfg(test)]
    let requested = {
        let site = b.reservations;
        b.reservations += 1;
        if b.fail_reservation == Some(site) {
            usize::MAX
        } else {
            count
                .checked_add(
                    *b.extra_capacity
                        .get(site)
                        .ok_or(NameIndexError::Allocation)?,
                )
                .ok_or(NameIndexError::Allocation)?
        }
    };
    v.try_reserve_exact(requested)
        .map_err(|_| NameIndexError::Allocation)?;
    add(
        &mut b.retained,
        bytes(
            v.capacity(),
            size_of::<T>(),
            NameIndexLimit::RetainedCapacity,
        )?,
        RETAINED,
        NameIndexLimit::RetainedCapacity,
    )
}

#[derive(Debug)]
struct Group<'a> {
    region: usize,
    raw_name: &'a str,
    start: usize,
    end: usize,
}
/// Immutable groups whose origin is the exact borrowed directory.
///
/// An index cannot outlive that directory:
/// ```compile_fail
/// use ktlsp_parser::{declaration_name_index::{build_name_index, DeclarationNameIndex}, structural_regions::RegionDirectory};
/// fn leak<'d, 'i>(d: &'d RegionDirectory<'i>) -> DeclarationNameIndex<'static, 'i> {
///     build_name_index(d).unwrap()
/// }
/// ```
#[derive(Debug)]
pub struct DeclarationNameIndex<'d, 'i> {
    directory: &'d RegionDirectory<'i>,
    groups: Vec<Group<'d>>,
    ordinals: Vec<usize>,
}
/// A borrowed group view, without a portable identity or independent storage.
///
/// The view cannot outlive its index:
/// ```compile_fail
/// use ktlsp_parser::{declaration_name_index::{build_name_index, NameGroup}, structural_regions::RegionDirectory};
/// fn leak<'d, 'i>(d: &'d RegionDirectory<'i>) -> NameGroup<'d, 'i> {
///     let index = build_name_index(d).unwrap();
///     index.groups().next().unwrap()
/// }
/// ```
/// Nor can its ordinal slice:
/// ```compile_fail
/// use ktlsp_parser::declaration_name_index::DeclarationNameIndex;
/// fn leak(index: &DeclarationNameIndex<'_, '_>) -> &'static [usize] {
///     index.groups().next().unwrap().declaration_ordinals()
/// }
/// ```
#[derive(Debug)]
pub struct NameGroup<'a, 'i> {
    region: &'a Region<'i>,
    raw_name: &'a str,
    ordinals: &'a [usize],
}
impl<'a, 'i> NameGroup<'a, 'i> {
    pub fn region(&self) -> &'a Region<'i> {
        self.region
    }
    pub fn raw_name(&self) -> &'a str {
        self.raw_name
    }
    /// Increasing ordinals in `index.directory().lowered().declarations()`.
    pub fn declaration_ordinals(&self) -> &'a [usize] {
        self.ordinals
    }
}
impl<'d, 'i> DeclarationNameIndex<'d, 'i> {
    pub fn directory(&self) -> &'d RegionDirectory<'i> {
        self.directory
    }
    fn view<'a>(&'a self, group: &Group<'d>) -> NameGroup<'a, 'i> {
        NameGroup {
            region: &self.directory.regions()[group.region],
            raw_name: group.raw_name,
            ordinals: &self.ordinals[group.start..group.end],
        }
    }
    /// Keys in first-declaration order. Flattening groups is not global declaration order.
    pub fn groups(&self) -> impl ExactSizeIterator<Item = NameGroup<'_, 'i>> {
        self.groups.iter().map(|g| self.view(g))
    }
    /// Exact spelling in the identical region object; no ancestor or completeness filtering.
    /// The query's storage need not outlive the returned view:
    /// ```no_run
    /// use ktlsp_parser::{declaration_name_index::{DeclarationNameIndex, NameGroup}, structural_regions::Region};
    /// fn lookup<'a, 'd, 'i>(index: &'a DeclarationNameIndex<'d, 'i>, region: &Region<'_>) -> Option<NameGroup<'a, 'i>> {
    ///     let query = String::from("x");
    ///     index.find(region, &query).unwrap()
    /// }
    /// ```
    pub fn find(&self, region: &Region<'_>, raw_name: &str) -> Result<Option<NameGroup<'_, 'i>>> {
        self.find_with_budget(region, raw_name, &mut Budget::default())
    }
    fn find_with_budget(
        &self,
        region: &Region<'_>,
        raw_name: &str,
        b: &mut Budget,
    ) -> Result<Option<NameGroup<'_, 'i>>> {
        cap(raw_name.len(), NAME_BYTES, NameIndexLimit::NameBytes)?;
        let mut origin = None;
        for (ordinal, candidate) in self.directory.regions().iter().enumerate() {
            b.work(1)?;
            if std::ptr::eq(candidate, region) {
                origin = Some(ordinal);
                break;
            }
        }
        let origin = origin.ok_or(NameIndexError::InvalidRegion)?;
        for group in &self.groups {
            b.work(1)?;
            if group.region == origin && b.equal(group.raw_name, raw_name)? {
                b.work(2)?; // selected region and ordinal-slice reads in the view
                return Ok(Some(self.view(group)));
            }
        }
        Ok(None)
    }
}

/// Group only an already validated directory; failures never return partial output.
pub fn build_name_index<'d, 'i>(
    directory: &'d RegionDirectory<'i>,
) -> Result<DeclarationNameIndex<'d, 'i>> {
    build_with_budget(directory, &mut Budget::default())
}
fn build_with_budget<'d, 'i>(
    directory: &'d RegionDirectory<'i>,
    b: &mut Budget,
) -> Result<DeclarationNameIndex<'d, 'i>> {
    let declarations = directory.lowered().declarations();
    let memberships = directory.memberships();
    cap(
        declarations.len(),
        DECLARATIONS,
        NameIndexLimit::Declarations,
    )?;
    cap(directory.regions().len(), REGIONS, NameIndexLimit::Regions)?;
    association_length(declarations.len(), memberships.len())?;
    for (d, m) in declarations.iter().zip(memberships) {
        b.work(3)?; // row and two selected input reads
        association_region(m.region(), directory.regions().len())?;
        cap(d.raw_name().len(), NAME_BYTES, NameIndexLimit::NameBytes)?;
    }
    let mut out = DeclarationNameIndex {
        directory,
        groups: Vec::new(),
        ordinals: Vec::new(),
    };
    reserve(&mut out.groups, declarations.len(), b)?;
    reserve(&mut out.ordinals, declarations.len(), b)?;
    for (d, m) in declarations.iter().zip(memberships) {
        b.work(3)?;
        let mut found = false;
        for group in &out.groups {
            b.work(1)?;
            if group.region == m.region() && b.equal(group.raw_name, d.raw_name())? {
                found = true;
                break;
            }
        }
        if !found {
            if out.groups.len() >= out.groups.capacity() {
                return Err(NameIndexError::InvalidAssociation);
            }
            b.emission()?;
            out.groups.push(Group {
                region: m.region(),
                raw_name: d.raw_name(),
                start: 0,
                end: 0,
            });
        }
    }
    // Repeated funded scans keep each group's duplicates contiguous without scratch storage.
    for group in &mut out.groups {
        b.work(2)?; // group row and start update
        group.start = out.ordinals.len();
        for (ordinal, (d, m)) in declarations.iter().zip(memberships).enumerate() {
            b.work(3)?;
            if group.region == m.region() && b.equal(group.raw_name, d.raw_name())? {
                if out.ordinals.len() >= out.ordinals.capacity() {
                    return Err(NameIndexError::InvalidAssociation);
                }
                b.emission()?;
                out.ordinals.push(ordinal);
            }
        }
        b.work(1)?;
        group.end = out.ordinals.len();
        if group.start == group.end {
            return Err(NameIndexError::InvalidAssociation);
        }
    }
    if out.ordinals.len() != declarations.len() {
        return Err(NameIndexError::InvalidAssociation);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Diagnostic, Element, FileKind, Parsed, SnapshotResult, SourceSnapshot,
        lowering::lower_declarations, structural_regions::build_regions,
    };
    use std::sync::Arc;

    // Same valid private CST construction pattern as lowering/structural_regions tests.
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
        fn fun(&mut self, name: &str, incomplete: bool) {
            self.open("FUN");
            self.token("FUN_KEYWORD", "fun");
            self.token("WHITE_SPACE", " ");
            self.token("IDENTIFIER", name);
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
                    elements: self.elements,
                    diagnostics: self.diagnostics,
                },
            }
        }
    }
    fn sample() -> SnapshotResult {
        let mut t = Tree::new();
        t.fun("x", true);
        t.fun("y", false);
        t.fun("x", false);
        t.finish()
    }
    fn limit<T: std::fmt::Debug>(result: Result<T>, which: NameIndexLimit) {
        assert_eq!(result.unwrap_err(), NameIndexError::Limit(which));
    }

    #[test]
    fn real_association_guards_and_inclusive_counters_reject_overflow() {
        assert!(association_length(0, 0).is_ok());
        assert!(association_length(DECLARATIONS, DECLARATIONS).is_ok());
        assert_eq!(
            association_length(1, 0),
            Err(NameIndexError::InvalidAssociation)
        );
        assert_eq!(
            association_region(0, 0),
            Err(NameIndexError::InvalidAssociation)
        );
        assert!(association_region(REGIONS - 1, REGIONS).is_ok());
        assert_eq!(
            association_region(REGIONS, REGIONS),
            Err(NameIndexError::InvalidAssociation)
        );
        for (max, which) in [
            (DECLARATIONS, NameIndexLimit::Declarations),
            (REGIONS, NameIndexLimit::Regions),
            (NAME_BYTES, NameIndexLimit::NameBytes),
            (WORK, NameIndexLimit::Work),
            (REQUESTED, NameIndexLimit::RequestedHeap),
            (RETAINED, NameIndexLimit::RetainedCapacity),
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
            limit(bytes(usize::MAX, 2, which), which);
        }
    }

    #[test]
    fn full_operand_lengths_are_charged_before_any_byte_comparison() {
        for (a, b, same) in [("ab", "ab", true), ("ab", "zz", false), ("ab", "z", false)] {
            let mut meter = Budget::default();
            assert_eq!(meter.equal(a, b).unwrap(), same);
            assert_eq!(meter.work, a.len() + b.len());
            let mut short = Budget {
                work: WORK - a.len() - b.len() + 1,
                ..Budget::default()
            };
            limit(short.equal(a, b), NameIndexLimit::Work);
        }
    }

    #[test]
    fn noncontiguous_duplicates_keep_incomplete_first_and_original_borrows() {
        let result = sample();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        assert!(std::ptr::eq(index.directory(), &directory));
        assert_eq!(index.groups().len(), 2);
        let groups = index
            .groups()
            .map(|g| (g.raw_name(), g.declaration_ordinals()))
            .collect::<Vec<_>>();
        assert_eq!(groups, [("x", &[0, 2][..]), ("y", &[1][..])]);
        let group = {
            let query = String::from("x");
            index
                .find(&directory.regions()[0], &query)
                .unwrap()
                .unwrap()
        };
        assert!(std::ptr::eq(group.region(), &directory.regions()[0]));
        assert!(std::ptr::eq(
            group.raw_name(),
            lower.declarations()[0].raw_name()
        ));
        assert!(!std::ptr::eq(
            group.raw_name(),
            lower.declarations()[2].raw_name()
        ));
        assert_eq!(
            group
                .declaration_ordinals()
                .iter()
                .map(|&i| lower.declarations()[i].incomplete())
                .collect::<Vec<_>>(),
            [true, false]
        );
        assert_eq!(lower.errors().len(), 1);
        assert_eq!(lower.errors()[0].owner(), Some(0));
        assert_eq!(lower.errors()[0].range(), 5..5);
        assert_eq!(lower.errors()[0].message(), "missing");
    }

    #[test]
    fn escaped_case_and_unicode_spellings_are_five_distinct_raw_keys() {
        let names = ["x", "`x`", "X", "`é`", "`e\u{301}`"];
        let mut t = Tree::new();
        for name in names {
            t.fun(name, false);
        }
        let result = t.finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        assert_eq!(
            index.groups().map(|g| g.raw_name()).collect::<Vec<_>>(),
            names
        );
        for (ordinal, name) in names.iter().enumerate() {
            assert_eq!(
                index
                    .find(&directory.regions()[0], name)
                    .unwrap()
                    .unwrap()
                    .declaration_ordinals(),
                [ordinal]
            );
        }
        for absent in ["", "x\0", "é", "e\u{301}", "`X`"] {
            assert!(
                index
                    .find(&directory.regions()[0], absent)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn equal_snapshots_and_shared_lowerer_do_not_establish_region_origin() {
        let result = sample();
        let other_result = sample();
        assert_eq!(result.snapshot(), other_result.snapshot());
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let other_lower = lower_declarations(&other_result, other_result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let same_inputs = build_regions(&lower, &result, result.snapshot()).unwrap();
        let other = build_regions(&other_lower, &other_result, other_result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        let second = build_name_index(&directory).unwrap();
        for foreign in [&same_inputs.regions()[0], &other.regions()[0]] {
            let mut meter = Budget::default();
            assert_eq!(
                index
                    .find_with_budget(foreign, "x", &mut meter)
                    .unwrap_err(),
                NameIndexError::InvalidRegion
            );
            assert_eq!(meter.work, directory.regions().len()); // no name comparisons
        }
        assert_eq!(
            index
                .find(&directory.regions()[0], "x")
                .unwrap()
                .unwrap()
                .declaration_ordinals(),
            second
                .find(&directory.regions()[0], "x")
                .unwrap()
                .unwrap()
                .declaration_ordinals()
        );
        assert_eq!(
            build_name_index(&other)
                .unwrap()
                .find(&directory.regions()[0], "x")
                .unwrap_err(),
            NameIndexError::InvalidRegion
        );
    }

    #[test]
    fn empty_file_never_reserves_and_still_checks_foreign_origin() {
        let result = Tree::new().finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let foreign = build_regions(&lower, &result, result.snapshot()).unwrap();
        let mut meter = Budget::default();
        let index = build_with_budget(&directory, &mut meter).unwrap();
        assert_eq!(
            (
                meter.reservations,
                meter.requested,
                meter.retained,
                meter.emitted
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(index.groups().len(), 0);
        assert!(index.find(&directory.regions()[0], "x").unwrap().is_none());
        assert_eq!(
            index.find(&foreign.regions()[0], "x").unwrap_err(),
            NameIndexError::InvalidRegion
        );
        let over = "x".repeat(NAME_BYTES + 1);
        limit(
            index.find(&foreign.regions()[0], &over),
            NameIndexLimit::NameBytes,
        );
    }

    #[test]
    fn both_real_reservations_and_combined_capacity_limits_precede_emission() {
        let result = sample();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let mut measured = Budget::default();
        let index = build_with_budget(&directory, &mut measured).unwrap();
        assert_eq!(measured.reservations, 2);
        assert_eq!(
            measured.requested,
            3 * (size_of::<Group<'_>>() + size_of::<usize>())
        );
        assert_eq!(index.ordinals.len(), 3);
        for site in 0..2 {
            let mut meter = Budget {
                fail_reservation: Some(site),
                ..Budget::default()
            };
            assert_eq!(
                build_with_budget(&directory, &mut meter).unwrap_err(),
                NameIndexError::Allocation
            );
            assert_eq!(meter.reservations, site + 1);
            assert_eq!(meter.emitted, 0);
            let mut meter = Budget::default();
            meter.extra_capacity[site] = RETAINED;
            limit(
                build_with_budget(&directory, &mut meter),
                NameIndexLimit::RetainedCapacity,
            );
            assert_eq!(meter.reservations, site + 1);
            assert_eq!(meter.emitted, 0);
        }
        let mut exact = Budget {
            requested: REQUESTED - measured.requested,
            retained: RETAINED - measured.retained,
            ..Budget::default()
        };
        assert!(build_with_budget(&directory, &mut exact).is_ok());
        assert_eq!((exact.requested, exact.retained), (REQUESTED, RETAINED));
        let mut over = Budget {
            requested: REQUESTED - measured.requested + 1,
            ..Budget::default()
        };
        limit(
            build_with_budget(&directory, &mut over),
            NameIndexLimit::RequestedHeap,
        );
        assert_eq!((over.reservations, over.emitted), (1, 0));
        let mut over = Budget {
            retained: RETAINED - measured.retained + 1,
            ..Budget::default()
        };
        limit(
            build_with_budget(&directory, &mut over),
            NameIndexLimit::RetainedCapacity,
        );
        assert_eq!((over.reservations, over.emitted), (2, 0));
    }

    #[test]
    fn build_and_query_work_exhaustion_is_atomic_and_budgets_are_fresh() {
        let result = sample();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let mut measured = Budget::default();
        let index = build_with_budget(&directory, &mut measured).unwrap();
        let mut exact = Budget {
            work: WORK - measured.work,
            ..Budget::default()
        };
        assert!(build_with_budget(&directory, &mut exact).is_ok());
        assert_eq!(exact.work, WORK);
        let mut short = Budget {
            work: WORK - measured.work + 1,
            ..Budget::default()
        };
        limit(
            build_with_budget(&directory, &mut short),
            NameIndexLimit::Work,
        );
        assert!(short.emitted > 0);
        for query in ["y", "absent"] {
            let mut cost = Budget::default();
            let found = index
                .find_with_budget(&directory.regions()[0], query, &mut cost)
                .unwrap()
                .is_some();
            let mut exact = Budget {
                work: WORK - cost.work,
                ..Budget::default()
            };
            assert_eq!(
                index
                    .find_with_budget(&directory.regions()[0], query, &mut exact)
                    .unwrap()
                    .is_some(),
                found
            );
            assert_eq!(exact.work, WORK);
            let mut short = Budget {
                work: WORK - cost.work + 1,
                ..Budget::default()
            };
            limit(
                index.find_with_budget(&directory.regions()[0], query, &mut short),
                NameIndexLimit::Work,
            );
            assert_eq!(
                (short.requested, short.retained, short.reservations),
                (0, 0, 0)
            );
        }
        assert_eq!(
            index
                .find(&directory.regions()[0], "x")
                .unwrap()
                .unwrap()
                .declaration_ordinals(),
            [0, 2]
        );
        assert!(build_name_index(&directory).is_ok());
    }

    #[test]
    fn long_comparisons_and_exact_query_length_have_controlled_limits() {
        let mut t = Tree::new();
        for i in 0..48 {
            t.fun(&format!("`{}{i}`", "a".repeat(900)), false);
        }
        let result = t.finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        limit(build_name_index(&directory), NameIndexLimit::Work);
        let mut t = Tree::new();
        for i in 0..40 {
            t.fun(&format!("x{i}"), false);
        }
        let result = t.finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        let exact = "z".repeat(NAME_BYTES);
        limit(
            index.find(&directory.regions()[0], &exact),
            NameIndexLimit::Work,
        );
        let over = "z".repeat(NAME_BYTES + 1);
        limit(
            index.find(&directory.regions()[0], &over),
            NameIndexLimit::NameBytes,
        );
        assert_eq!(
            index
                .find(&directory.regions()[0], "x39")
                .unwrap()
                .unwrap()
                .declaration_ordinals(),
            [39]
        );
    }

    #[test]
    fn declaration_ceiling_preserves_every_duplicate_without_truncation() {
        let mut t = Tree::new();
        for _ in 0..DECLARATIONS {
            t.fun("x", false);
        }
        let result = t.finish();
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let index = build_name_index(&directory).unwrap();
        assert_eq!(index.groups().len(), 1);
        let group = index.groups().next().unwrap();
        assert_eq!(group.declaration_ordinals().len(), DECLARATIONS);
        assert!(
            group
                .declaration_ordinals()
                .iter()
                .copied()
                .eq(0..DECLARATIONS)
        );
    }
}
