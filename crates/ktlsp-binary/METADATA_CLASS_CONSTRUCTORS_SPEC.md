# Class selected Constructor composition

`metadata::class_constructors::decode` accepts one unframed Class byte slice.
Caller selects supported kind1/mv[2,3,0]; this API does not inspect a Metadata
header or distinguish Class/Package. It returns unchanged selected ClassHeader
raw/flags/required signed fq_name and one ordered `Vec<ConstructorOccurrence>`.
Each occurrence contains a Class-relative body_span and existing typed Constructor:
complete borrowed raw body, optional signed flags, ordered opaque ValueParameter
bodies with Constructor-relative spans, ordered packed/unpacked signed version IDs.
Repeated/empty Constructors stay distinct, ordered and unmerged. All raw slices
borrow supplied Class storage, including empty one-past-end bodies.

## Validation order and errors

The reused constructor::Limits has bytes/records/work/owned_bytes. Byte preflight
rejects input above min(caller,16MiB) as Wire.Limit(bytes)0, before other limits.
One existing string_table::Budget covers the entire operation. Complete unchanged
ClassHeader validation runs first, including required fq_name and malformed tail.
Those errors use ErrorKind::Class and are already Class-relative.

A second charged scan visits every Class field: wire::field and dispatch work1.
Only field8/wire2 invokes the delivered private Constructor whole-Budget helper.
Child errors use ErrorKind::Constructor and are rebased by body_start exactly once.
Selection and outer-vector errors use ErrorKind::Wire with existing Class offsets.
After successful child decode, budget.push reserves one typed occurrence at the
outer tag offset. No parent staging directory, extra raw vector, helper redesign,
node/depth accounting or pre-count scan is introduced.

Unknown/wrong-wire legal fields remain opaque while their outer framing is
validated; existing wire group/invalid-type/number/varint/length behavior remains.
Parent complete-tail/MissingFqName errors precede any child failure. Selection is
interleaved with child decoding; a child fault prevents visiting later selection
fields even though the complete first Class scan already validated them.

## Resource accounting

All records, work and logical requested capacities share one monotonic Budget:
parent scan, selection scan, every Constructor, parameter vector, version vector
and outer occurrence vector. No resets/refunds between phases, children or lists,
or on failure. Existing push debits owned bytes before capacity work/reservation;
allocation mapping and checked arithmetic stay unchanged. Logical capacity excludes
allocator surplus, reallocation churn, stack and RSS. No whole-pipeline RSS promise.

Let O=sizeof(ConstructorOccurrence), S=sizeof(ValueParameterBody), I=sizeof(i32),
P(0)=0 and P(n>0)=next power2. For Class N bytes/F fields and C children, child j
has n_j bytes/f_j fields, p_j parameters, K_j packed ID items with L_j encoded
value bytes and V_j total IDs. Exact success minima:

- records = 2F + sum(f_j + K_j)
- owned = P(C)O + sum(P(p_j)S + P(V_j)I)
- work = 2N + 4F + sum(n_j + 2f_j + K_j + L_j + V_j) + owned

Hard caps remain bytes/owned16MiB, records65535, work64MiB; caller values tighten
but never raise them. No extra node charges apply to these existing helpers.
Failed operations return no partial result; input bytes are never mutated.

## Frozen acceptance

Seventeen independent literal fixtures,477 prefix cuts,22 faults,ten scalar
values and wrong-wire controls freeze tuples, presence, raw/body/parameter slice
identity, offsets, parent-first precedence, exact minima/+1/one-less, failure
charges, growth0/1/2/4 and hard clamps. Two children both allocate parameter/ID
vectors, exposing aggregate-accounting resets. Parameter malformed/empty bodies
succeed opaquely; no child ValueParameter initialization is attempted.

The genuine unchanged Pair.class pipeline parse_class -> annotations -> Metadata
header/d1 -> payload -> new Class composition supplies Class flags1031/fq0,
Constructor body31..56 flags70, parameters4..10/12..18 (Class35..41/43..49,
d1 82..88/90..96), no version IDs. Separate existing ValueParameter assertions
use their own budgets; header/d1/payload calls also remain separate operations.
Compiler provenance remains unverified. Positive signed/version cases are authored.
See fixture README and existing metadata_constructor provenance/notices.

## Compatibility and limits

Only the new module/API and export are added. Raw ClassHeader/Constructor types,
functions, error APIs, fixtures and callers stay unchanged. No name lookup,
index/version applicability validation, nested merge/type reconstruction, overloads,
constructor symbols, providers or server integration. #18/#68 remain open;
full semantic parity, platform correctness and performance remain unverified.
T008/T008a HOLD, C13/B3/B4 GAP, full B11 UNVERIFIED and #96 Darwin UNKNOWN remain.
