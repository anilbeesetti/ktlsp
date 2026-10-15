# Class TypeParameter composition — source admitted

Root admitted the accepted21 exclusive paths and coordinated additive module
export on f9146027c072fdcc3d3ec1c34726678d6b5c5c8c after actual PR275 OWNER+ROOT
delivery. Delivered helper SHA256aeec4edd9f84f994369caf1e0a1cc99fc7f0c03fecf902ce4fb6a020e7a21ce2
matches the accepted compatibility delta89968127b6caf1541010f736b571c7ad181202f863decf302192f82dfc3ccf55.
Historical prerequisite HOLD text below records preparation-time state; root has
now explicitly admitted source. Publication/CI/merge remain separate gates.
No local native runtime; hosted pinned Linux format/Clippy/workspace checks are
required after actual source reviews. This source does not promote semantics,
providers, full initialization, performance or full Kotlin LSP parity.

The unchanged preimplementation contract follows. Its literal expected data is
tests/fixtures/metadata_class_type_parameters/EXPECTATIONS.md.
That copied data now documents the one-byte nonminimal authored-length erratum
found by original PR285 CI. Expected values/spans/budgets remain unchanged;
the original sealed preparation packet is preserved separately.

# T018 Class selected TypeParameter composition — preparation v1

Status: CONTRACT CANDIDATE; SOURCE_HOLD. Prepared 2026-10-15. No source,
public documentation, dependency, build, native execution, publication or CI.
Owner worktree /Users/anil/.codex/worktrees/t018-class-type-parameter-contract/ktlsp,
branch codex/t018-class-type-parameter-contract. Input HEAD
46fd28ddba16ddd6d000588cae56548a7af3d732, tree
c06f6dc81b8063bfb3c0e1881860ac61d47b6e96, sole parent
66d2a99c752f901dbe7a46a7fc5254cdd8a90484. Root251 delivery-clear input
/private/tmp/ktlsp-root-251-owner-delivery-clear.json SHA256
c5cee42167d7852f68407bc50dd939a69cb5b554066c5247447e98f4bf7af72a.

## Prerequisite and ownership

At this exact base, ClassHeader has an internal shared-wire-Budget helper;
TypeParameter only exposes decode, which creates a fresh string_table::Budget.
The existing TypeParameterBounds owner 01a13bc4-0553-7d01-9dbe-4ec8450623a9
owns the whole-Budget helper and has a particular publication DENIAL HOLD.
This task does not copy, redesign, implement or publish that helper. No proxy
publication or workaround is authorized. Root must resolve helper ownership,
delivery and publication restrictions, inspect the actual accepted helper's
compatibility, and grant exact source paths/schema before implementation.
CONTRACT_READY never means SOURCE_READY. If helper behavior changes this contract,
renew the affected literal freeze and SAME2/architect reviews before source.

## Smallest future API (proposal for root acceptance)

New metadata::class_type_parameters, reusing string_table::Limits unchanged:

    SelectedTypeParameter<'a> { span: Range<usize>, parameter: TypeParameter<'a> }
    ClassWithTypeParameters<'a> { header: ClassHeader<'a>, type_parameters: Vec<SelectedTypeParameter<'a>> }
    ErrorKind { Wire(wire::ErrorKind), Class(class_header::ErrorKind), TypeParameter(type_parameter::ErrorKind) }
    Error { offset: usize, kind: ErrorKind }
    decode(bytes: &[u8], limits: Limits) -> Result<ClassWithTypeParameters<'_>, Error>

Structs Debug/PartialEq/Eq; error Copy/Clone/Debug/PartialEq/Eq + Display/Error.
Limits re-export existing string_table::Limits, including nodes/depth; no new
configuration/helper/dependency. Parent errors use Class (including parent Wire),
child helper errors use TypeParameter (including child Wire), container byte/node/
selection-wire/storage errors use Wire. Phase is therefore observable and errors
cannot confuse parent MissingFqName with child MissingId/MissingName.

Caller selects supported k=1, mv=[2,3,0] and supplies ONE unframed Class body.
Bytes-only API cannot check header/version/container. Preserve ClassHeader exact
raw/flags/fq_name and existing last-recognized/wrong-wire/signed behavior. First
perform byte preflight, create ONE aggregate Budget, charge parent node(depth1,0),
then finish existing ClassHeader validation INCLUDING required fq_name before any
child work. Next perform an explicit charged selection scan over the whole Class.
Each field pays wire::field + dispatch work1. Select ONLY (field5,wire2), in encoded
order, including duplicate/equal bodies. Child node(depth2,body_start) precedes
the owner-delivered raw TypeParameter helper on the SAME whole Budget; successful
child validation precedes outer Budget::push at OUTER TAG START. No directory,
deduplication, merging, name resolution, speculative capacity or reset/refund.

SelectedTypeParameter.span is body-only and Class-relative, derived from validated
field end minus borrowed body length. parameter.raw borrows exactly that body;
header.raw borrows the complete Class. Existing parameter.upper_bound spans stay
TYPEPARAMETER-RELATIVE and bodies remain opaque/borrowed. Child faults rebase once
by selected body_start; no nested span rebasing or second error rebase. Input
bounded by MAX_BYTES makes this addition representable and within selected body.
Required empty child fails MissingId at its end; it cannot return an empty valid
parameter. MissingId precedes MissingName; malformed child wire precedes both.
Missing fq_name/parent malformed tail precedes earlier invalid selected children.

IDs/names/bound IDs remain raw signed indices; no UTF-16 lookup/string allocation,
index validation, type reconstruction, semantics, variance/substitution validation,
full nested initialization, Annotation/extension scan, server consumer/provider,
capability or performance/full-parity claim. Deeper bound bodies remain opaque.
Function TypeParameter lane is deferred; no genuine Function positive established.

## Aggregate accounting frozen from existing Budget behavior

Hard ceilings unchanged: bytes/owned 16MiB, records/nodes65,535, depth2,
work64Mi units; caller usize::MAX cannot raise ceilings. Header/d1/framing/name
resolution remain separate operations; this is only one Class operation.

Let N/F=Class byte/field counts, K=selected children, n_i/f_i=child byte/field
counts, p_i/l_i=packed bound-ID count/bytes, u_i=unpacked bound-ID count.
Let S=size_of::<SelectedTypeParameter>(), B=size_of::<UpperBound>(), and C be
ALL successful logical vector growth bytes (outer vector + each child's two raw
vectors). Successful requirements are:

    records = 2F + sum(f_i+p_i)
    nodes = 1+K; depth = 1 when K=0, otherwise 2
    work = 1 + 2(N+2F) + K + sum(n_i+2f_i+l_i+2p_i+u_i) + C
    owned_bytes = C

Parent node contributes1 work; each child node1. No raw helper node charges may
be added without renewal: raw standalone TypeParameter has none today.
Logical growth: len0 adds1 slot, positive power-of-two len adds len slots;
capacity for 1/2/3/4/5 occurrences charges1/2/4/4/8 times S. B and S are
platform layout expressions, not observed allocator capacities. Child bound IDs
use sizeof(i32)=4, with same growth rule. No claim about surplus, churn, stack,
RSS or allocation failure observability. No allocation forcing/OOM execution.

Failure order is bytes → parent node → complete parent wire/dispatch/required
checks → second scan field/dispatch → child node → child scan/required checks →
outer push. node order: depth check → nodes deduction → work deduction.
record order: records deduction → work deduction → bytes. push order: checked
slot arithmetic → owned deduction → growth-work deduction → try_reserve_exact
→ append. Child packed IDs use existing record/varint/interpretation/push rules.
Failed checked_sub leaves THAT counter unchanged; all earlier successful charges
persist, even when child allocation/validation or later outer growth fails.
No refund/reset and no claim that an unsuccessful attempted debit consumed its
full amount. Tests can assert private Budget transitions only in already-owned
helper tests; this task cannot extend helper ownership to add instrumentation.

## Future reach and admission gate

Exact prospective allowlist is FUTURE_PATHS.txt, proposals only. One new provider,
one focused integration test, ticket-local fixtures/spec/review plus coordinated
mod.rs additive export. Existing ClassHeader/TypeParameter sources/APIs, wire,
Budget, Cargo/workflows/status/docs and consumers are readonly/outside ownership.
Call-site search finds no existing composed Class TypeParameter caller to migrate.
Potential break: changed raw helper semantics would break existing raw consumers;
therefore preserve standalone decode and all raw API/opaque-span behavior exactly.

Future genuine test must traverse existing licensed Pair.class.bin through native
parse_class → annotations::decode → metadata::decode → payload::decode → this API;
expected literals originate in PAIR_WITNESS.md, never implementation output.
No executing this pipeline now. Future source must first pass SAME2 + architect;
root then decides CI/publication and merge separately. Current contract reviews
do not authorize source or count as implementation/CI reviews.

T008/T008a HOLD; C13/B3/B4 GAP; full B11 UNVERIFIED; #7/#83/#96 OPEN, Darwin
UNKNOWN; full official semantic parity unfinished. No assessment cutoff advanced.
