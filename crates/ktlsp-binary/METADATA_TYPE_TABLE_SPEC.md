# Bounded Kotlin TypeTable directory

Root admitted this finite contract at base
cff36d17cd5cd1386059ecc91572013fdb417b48. Contract-only architect,
correctness/functionality and adversarial reviewers approved the independent
literal/API/budget/offset freeze before implementation. Source/runtime approval
is recorded separately in METADATA_TYPE_TABLE_REVIEW.md.

## API and selected data

`metadata::type_table::decode(bytes, Limits)` returns
`TypeTable { raw: &[u8], types: Vec<&[u8]>, first_nullable: Option<i32> }`.
It reuses wire::Error/ErrorKind, with table-relative offsets. Limits supplies
bytes, records, work and owned_bytes; callers can tighten defaults only.
The caller selects one unframed supported mv=[2,3,0] TypeTable body; this
bytes-only API cannot select or enforce version/kind/container.

Pinned Kotlin v2.3.20 metadata.proto lines279..286 and ProtoBuf.java TypeTable
parser tags10/16 establish repeated Type field1 and optional int32 field2 with
default-1. Exact reference hashes and retained notices are in the fixture README.
Field1/wire2 appends borrowed opaque bodies in order, including empty/duplicate
bodies. Field2/wire0 retains low32 signed i32, last recognized scalar wins.
Absent None remains distinct from explicit Some(-1). Negative and out-of-range
thresholds remain raw; they are not checked against the vector or applied.
Wrong-wire/unselected fields stay in complete borrowed raw bytes only and never
erase selected values. Types are not parsed, initialized, expanded or linked.
No name resolution, nullability application, semantic/provider consumer or full
metadata parity follows. Public caller-constructed structs have no validated
raw/typed-equivalence guarantee.

## One aggregate operation budget

Hard defaults: 16MiB input,65535 fields,64Mi work units,16MiB requested retained
logical capacity. Empty input with all limits0 succeeds. Bytes admission runs
first and reports Limit(bytes)@0. One existing wire::Budget spans the whole scan;
each field attempt debits record1 then work1 before reads. Each read/skipped byte
costs work1; every completed field pays work1 dispatch at tag start before any
selected-data action. Unknown/wrong-wire/late fields are scanned too. Opaque
bodies are charged once without nested reads. No early stop/refund/partial result.

Let S=size_of::<&[u8]>(). Before append at len0 or a power of two, checked
slots=max(len,1), len+slots and slots*S precede aggregate owned subtraction,
work(slots*S) and try_reserve_exact(slots), in that order. Only then retain the
borrowed body. The deterministic logical capacity is C=next_power_of_two(k),
C0=0. Successful N-byte,F-field scan costs N+2F+C*S work and C*S owned capacity.
Failure labels are owned_bytes/work/allocation at field tag start. Allocation
surplus, churn, RSS, CPU/walltime and whole-pipeline budgets are not bounded by
these charges. This separate operation does not aggregate header/d1,composition,
Class/Function/name-resolver or caller scans. No new depth/node limit is needed
because there is no nested parsing or recursion.

Precedence: bytes,wire record,record work,reads/structural validation,dispatch,
owned admission,capacity work,allocation. Wire rules and errors are unchanged:
nonminimal low32 tags/lengths and low64 slow-path values; ten continuations fail;
negative low32 lengths reject; field0 precedes wire validity; groups3/4 unsupported,
wires6/7 invalid. Truncation reports supplied end after failed-read work charge;
negative length reports length-varint start; malformed varint reports tenth
continuation; other structural errors report tag start. Complete Type bytes may
be malformed internally and still succeed as this directory's opaque data.

## Frozen validation and scope

`tests/fixtures/metadata_type_table/EXPECTATIONS.md` preserves preimplementation
literals,54 prefix cuts,scalar conversions,growth1..5,exact limits and error
offsets. Integration tests check borrowed pointer identity,ordered/empty/malformed
bodies,scalar presence/duplicates/wrong-wire,unknown fields,structural errors,
aggregate limits/hard clamps and deterministic hostile bytes. Allocation failure
mapping has static review only.

Owned changes are one new module,integration test,fixture directory,this spec
and its review record; metadata/mod.rs adds only `pub mod type_table;` in the
root-serialized slot. No shared helper,Cargo,dependency,workflow,server,parser,
worker or root-status changes. No existing callers migrate. No local native
runtime while #96 remains OPEN/UNKNOWN; admitted local checks are formatting and
compile-only locked workspace Clippy. Runtime evidence must come from original
unchanged Linux Native Rust CI. #68/#18 remain open; T008/T008a HOLD and broader
C13/B3/B4 GAP/B11 UNVERIFIED are unchanged. No semantic or performance claim.
