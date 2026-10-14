# Selected Kotlin Constructor data

`metadata::constructor::decode(bytes, Limits)` scans one caller-selected unframed
Constructor from independently selected supported mv=[2,3,0] Class field8/wire2.
Bytes alone cannot enforce header/version/kind/container. No production caller.

Returns complete borrowed `raw`, optional signed `flags`, ordered opaque
`value_parameters: Vec<ValueParameterBody { raw, span }>` and ordered signed
`version_requirements`. Parameter spans are body-only, Constructor-relative.
Recognized pairs:1/wire0 flags;2/wire2 parameter bodies;31/wire0 or2 IDs.
Flags/IDs interpret low32 as signed int32 without zigzag or semantic/index checks.
Flags last-recognized wins; absence=None has schema default6, distinct from
explicitSome6/0. IDs append across packed/unpacked occurrences without deduplication.
Parameters retain all empty/repeated/malformed bodies without nested scanning.
Wrong wires never set/clear selected fields. Full raw retains every exact unknown,
wrong-wire, annotation3, plugin32, extension, duplicate and cross-field ordering.
Public caller-constructed values have no validated raw/typed-equivalence guarantee.

Existing Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f schema
metadata.proto289..307 and generated ProtoBuf.java15053/15089..15153 establish
these fields; defaults15366..15372 and initialization15373..15403 establish that
this subset omits nested initialization. Hashes and retained Kotlin/protobuf
notices are in fixture README. Reference Java/schema is neither shipped nor run.
Existing wire nonminimal/low32/low64 slow-path policy, group rejection and negative
length handling are unchanged; no universal JVM/runtime equivalence claim.

## One aggregate operation budget

Limits bytes/records/work/owned_bytes default to existing16MiB/65535/64Mi/16MiB
ceilings; callers tighten only. Bytes admission first, Limit(bytes)@0. Reuse
unchanged string_table::Budget new/push/integers and wire helpers. No root/node/
depth charge. One records/work/owned counter spans full scan and both vectors.
Every field attempt record1/work1; wire bytes work1 each; completed field dispatch
work1 at tag start, including wrong/unknown fields. No early success or partial
result. Opaque bodies charged once, no per-parameter/reset/refund.

Unpacked ID conversion pays work1 then push at tag start. Packed outer field
already charges body bytes; each item reread pays record1/work1, varint-byte work,
conversion work1 and push at item start. Packed boundary is exact body end;
following fields cannot rescue truncated values. All errors abort whole call.

Let S=sizeof(ValueParameterBody),I=sizeof(i32),P body count,V ID count,K packed ID
count,L packed-body bytes; C(n)=0 for0 else next_power_of_two(n). Success N-byte,
F-field scan requires recordsF+K, workN+2F+L+K+V+C(P)S+C(V)I,
ownedC(P)S+C(V)I. Growth at len0/power-of2: slots=max(len,1), checked len+slots,
checked slots*sizeof(T), aggregate owned subtraction, work(bytes),
try_reserve_exact(slots), append. Labels owned_bytes/work/allocation at tag start
(parameters/unpacked IDs) or item start (packed IDs). Allocator surplus does not
reduce logical charges. These do not bound churn/transient buffers/stack/RSS/CPU/
walltime or the whole composition pipeline. Allocation failures static-review only.

Precedence: bytes, record, record-work, reads/structural validation, dispatch,
integer per-item charges, checked capacity/owned, capacity-work, allocation.
Failed short bulk read charges remaining+1 before Truncated@end; failed single
read charges1 before Truncated@end. NegativeLength at length start, malformed
varint at tenth continuation, other structural errors tag start. Empty all0
succeeds. No declared-length reservation, recursion or caller-trusted spans.

## Frozen validation and genuine composition

Fixture EXPECTATIONS SHA256
e3e2a6c07e755deadd06d645d621babaae75056c05407984de596f6c0cead6e8 freezes94 cuts,
raw/spans/flags/IDs, signed/duplicate/wrong-wire/packed/malformed/offset/precedence
and aggregate vector-growth expectations before implementation/runtime.
Finite hostile generation does not prove all-input safety or runtime compatibility.

Licensed unchanged Kotlin Pair.class2983 bytes SHA256
8fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736 has Class field8
76..103/body78..103 in decoded d1; flags70, two parameter bodies4..10/12..18
(Constructor-relative), no version IDs. Composition test calls classfile,
annotations, metadata header/d1, flat wire Class selection, this Constructor and
each delivered ValueParameter decoder with SEPARATE operation budgets. Constructor
bytes25/records4/work33+2S/owned2S; each parameter bytes6/records2/work10, raw name
indices5/6, other selected scalars absent. Inline Types remain opaque. No names/
types/constructor symbols/overloads/full nested initialization/version applicability/
JAR-to-symbol/server/full-parity/performance demonstration. Compiler provenance
UNVERIFIED. Extraction provenance and first safe-data lookup failure retained.

Owned new module/test/fixture/spec/review plus one serialized additive mod export.
No shared-helper/Cargo/lock/workflow/vendor/parser/server/public-status edits.
#18/#68 OPEN; T008/T008a HOLD; broaderC13/B3/B4 GAP; B11 UNVERIFIED;
#7/#83/#96 OPEN, Darwin UNKNOWN. Local native execution remains held under#96.
