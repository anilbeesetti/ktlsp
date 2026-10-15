# Source-only admission; runtime validation pending

Root admitted the exact seventeen paths at77bd0ac8b4e74dadd037a3404b82dba7e004c395
(tree91e836ff6855273d08fc0d536d41767043eca1f9, sole parent6b965cfa).
The following frozen historical6b preparation contract is preserved unchanged.
Publication and native runtime remain ungranted. Final pinned format and first
compile-only locked all-target Clippy passed; source review pending.

# TypeParameter inline upper-bound composition — TEMP contract v1

Preparation only, 2026-10-15. Immutable source: 6b965cfa8008df51deba5ff4a56be31e9b30d16b,
tree d6c1fa7ab838488f9dd8283d9cfb19c0cecb4025, sole parent efbfb991014f926ab2031d489a53a12ef386ed7b.
Owned clean worktree /Users/anil/.codex/worktrees/t018-type-parameter-bounds/ktlsp,
branch codex/t018-type-parameter-bounds. No product source, public docs, exports,
issue posting, publication, CI, builds or native runtime admitted in this phase.

## Concrete output and consumer

One separately bounded call accepts ONE caller-selected unframed TypeParameter body
for supported mv=[2,3,0]. Caller selects header version/kind/container separately.
It returns required signed id/name, raw optional reified/variance presence, ordered
signed bound IDs, and every field5/wire2 inline upper-bound occurrence, including
empty and duplicate bodies, as span plus existing TypeHeader. The only consumer
in this child is new integration tests; no production caller is introduced.

Proposed API (documentation only; no Rust file written):

```rust
pub use super::string_table::Limits;
pub use super::type_parameter::{Error, ErrorKind};
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedUpperBound<'a> {
    pub span: std::ops::Range<usize>, // body-only, TypeParameter-relative
    pub header: super::type_header::TypeHeader<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct TypeParameterWithBounds<'a> {
    pub raw: &'a [u8],
    pub id: i32,
    pub name: i32,
    pub reified: Option<bool>,
    pub variance: Option<super::type_parameter::Variance>,
    pub upper_bound: Vec<SelectedUpperBound<'a>>,
    pub upper_bound_id: Vec<i32>,
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeParameterWithBounds<'_>, Error>;
```

No field_number (always 5), repeated raw field (header.raw already borrows the
body), parent wrapper or second retained opaque-bound directory. Existing public
TypeParameter, Limits, Error/Kind and decode behavior remain exactly unchanged.
No effective-default getter, ID validation/lookup, bound synthesis, recursive
Type/Argument/Annotation initialization, merging, substitution, graph, generic
semantics, symbols, providers or full metadata/LSP parity. Type IDs and deeper
messages remain opaque. No semantic preference between inline bounds and IDs.

## Minimal extraction and ownership

Future existing type_parameter.rs change is MECHANICAL ONLY:
1. Keep public decode's byte precheck and string_table::Budget construction exactly.
2. Public decode then calls a new pub(super) decode_with_budget(bytes, &mut Budget).
3. Move existing `let mut pos = 0` through final Ok(TypeParameter { ... }) into that
   helper unchanged, replacing `&mut budget.wire` with the equivalent borrow as
   needed. No node charge, extra check, new mode/callback or public API in helper.
The new helper takes the WHOLE existing string_table::Budget, because parent
opaque bounds and integer IDs already reserve through that budget. It must not
construct/reset/refund a budget or call public child decode.

New composition checks bytes first, constructs ONE existing aggregate Budget,
charges node(1,0), calls the private parent helper, then destructures/consumes its
output. All parent fields/tail/packed IDs and MissingId then MissingName checks
complete before any child node/parse/typed reservation. Move upper_bound_id once;
consume opaque upper_bound in order. For each existing bound span: node(2,start),
TypeHeader::decode_with_budget(bound.raw, &mut budget.wire), map only its wire
error by offset += bound.span.start ONCE, then budget.push typed entry at BODY
START (not parent tag). node and typed push already use parent offsets; never
rebase them. Parent helper errors are already parent-relative; never rebase them.
No second parent scan. No caller spans are trusted: spans come from wire reader.
Valid spans imply start+child_error.offset<=end<=input.len; no arithmetic overflow.

Result retains ONE typed-bound directory and the moved ID vector. During
conversion the consumed opaque staging allocation and typed allocation overlap.
BOTH are charged conservatively to the same budget, even after staging is freed;
no refund. This is deliberate minimal reuse, not a claim of zero temporary
allocation or a final-output-size-only budget. The old public raw API remains
available to callers needing only opaque bodies. No ABI/layout redesign.

## Scalars and framing inherited unchanged

Parent id/name/IDs take low32 signed int32. Recognized scalars last-win;
reified uses full deterministic raw64!=0. Variance 0/In,1/Out,2/Inv; unknown enum
preserves previous known value. Wrong wires neither set nor clear selected data.
Bound IDs append across packed/unpacked fields in encounter order, empty packed
segments append nothing. Every recognized field5/wire2 body is selected regardless
of IDs. TypeHeader returns all ten independent optional scalars in declaration
order; classifiers may conflict, bool full raw64, wrong wires remain raw.

Unknown parent/child bytes and deeper bodies are borrowed unchanged. Nonminimal
varints, low32 tags/lengths, groups unsupported, negative lengths, field0 before
wire-type validation and raw64 slow-path limitations remain wire behavior.
Use one shortcut comment: `shortcut: inline bounds are unmerged data; upgrade when Type interpretation is implemented.`
No upstream parser/compiler/runtime source is shipped or executed.

## Aggregate accounting and fault precedence

Existing string_table::Limits bytes/records/nodes/depth/work/owned_bytes is reused.
Hard maxima: bytes=owned_bytes=16MiB, records=nodes=65,535, depth=2,
work=64Mi units. Caller may tighten, never raise. Root node1 always charged after
byte check; each selected inline bound (even empty) node2 charged after complete
parent success. No node for deeper opaque data. Child wire and parent packed IDs
share the same wire budget. No downstream per-child/public reset.

Let F=parent wire fields, N=parent bytes, P=packed-ID varint count, B=packed-ID
payload bytes reread, U=unpacked-ID count, n=selected bound occurrences,
C=sum(child body lengths), H=sum(child wire-field counts), m=ID count.
Let cap(0)=0; cap(k>0)=smallest power of two >=k, S=size_of::<raw UpperBound>(),
T=size_of::<SelectedUpperBound>(), I=size_of::<i32>()=4. Symbolic sizes are used;
no host ABI is presumed or measured. Successful exact accounting:

- records = F + P + H
- nodes = 1 + n; required depth=1 if n=0, otherwise2
- owned = cap(n)*S + cap(n)*T + cap(m)*I
- work = N + 2F + B + 2P + U + C + 2H + (1+n) + owned

Budget::push charges deterministic requested slots1,1,2,4,... at lengths0,1,2,4
and existing reserve_exact behavior; excludes allocator surplus/churn, stack,
RSS, input ownership and all upstream pipeline budgets. Allocation refusal maps
to Wire(Limit(allocation)) at existing staging tag/ID offset or new typed body
start after owned/work debit; no forced allocator/OOM experiment. Runtime not run.

Byte ceiling wins at0 before node. Parent budget/framing faults win before
required checks; MissingId at EOF wins MissingName; either required fault wins
all child errors. Child node(depth then nodes then work) wins child framing;
child framing/dispatch wins typed reservation. Iterate children in occurrence
order and stop at first fault. Parent raw API still accepts malformed opaque
bound contents; composition intentionally rejects malformed selected Type body
framing. No partial success. Small work/storage cuts may win structural faults
according to the unchanged shared helper call order, not an invented precedence.

Header/d1/payload/table/Class/flat-wire/resolver operations remain separately
budgeted. Composition is not a whole-pipeline CPU/memory/RSS bound.

## Pinned sources and evidence

Use existing committed source pins only; research is complete, no new artifact
hunt. Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f:
metadata.proto Type121–175 and TypeParameter177–197 SHA256
 a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2;
ProtoBuf.java Type4896 onward/TypeParameter7569 onward SHA256
 da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716;
patched CodedInputStream readBool382 SHA256
 7fbb3aeebd3c3d5414d7171bc3ba2785de50b40863a4f381c4b43986e15784fe.
Pins are inherited provenance, not fresh external-source verification. Preserve
existing Kotlin/protobuf notices byte-identically in prospective fixture directory.
All positive inline-bound fixtures below are independently authored before code.

Held genuine Pair.class.bin is unchanged and has NO inline-bound positive. Its
285-byte decoded d1 from existing PAIR_EXTRACTION.json has Class body47..285 and
TypeParameter bodies54..60 (`08 00 10 01 20 01`) and62..68
(`08 01 10 02 20 01`), Class-relative7..13 and15..21. Future existing pipeline can
assert exact tuples (0,1,None,Some(Out),[],[]) and
(1,2,None,Some(Out),[],[]), pointer identity into caller slices, zero typed bounds
and zero IDs. This is genuine ZERO-inline-bound coverage, not a positive. Compiler
provenance remains UNVERIFIED despite distribution2.3.20-release-208. Class hash
8fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736;
JAR hash0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
No new genuine artifact duplicated; future test reuses existing constructor Pair
fixture and full header/d1/Class wire pipeline with separate budgets.

## Reach / exact prospective scope

See INVENTORY.md for every exact path/caller/test/export. No current repository
writes are authorized. Only after root separately grants source: new module,
integration test, exact fixture files, own SPEC and REVIEW; mechanical parent
helper extraction needs explicit additional grant. metadata/mod.rs one additive
`pub mod type_parameter_bounds;` needs coordinated release. No Cargo/lock/dependency,
config/workflow, lib root, wire, string_table, TypeHeader, parser/server/vendor or
public-status edits. Existing TypeParameter tests must stay byte-for-byte and pass.
No existing consumer needs migration; new output/API is additive.

## Subsequent phases are ungranted

Retain SAME3 architect, correctness/functionality and adversarial reviewers for
actual source, changes and completed original hosted Linux CI. Future pinned
commands unchanged: cargo fmt --all -- --check;
cargo clippy --locked --workspace --all-targets -- -D warnings;
cargo test --locked --workspace. No local runtime under #96. No CI reruns/output
expectation tuning/settings changes. Own push/open+ATTACH PR requires separate
grant; ONE exclusive root expected-head/base lease, actual standalone strictly
read-only assertion guard EXIT0/full matching COMPLETEPASS, separate ordinary
expected-head squash, OWNER+ROOT original exact-main delivery/proof/release.
No bypass/admin/force-main. Contract readiness does not claim source/runtime/CI.

Native server/core/build/default tests remain Rust-only. No bundled Java/JVM/JDK,
compiler/JNI/C/C++ analysis/tree-sitter C or oracle. T004 data-only; T008/T008a
HOLD, broader C13/B3/B4 GAP, full B11 UNVERIFIED; #7/#83/#96 OPEN and Darwin
UNKNOWN. #18/#68 remain open; no semantics, provider, parity or performance promotion.
