# Constructor selected parameter composition

`metadata::constructor_parameters::decode(bytes, constructor::Limits)` decodes one
caller-selected unframed Constructor into borrowed raw bytes, optional raw flags,
ordered signed version IDs and one ordered typed parameter directory. Each
`SelectedValueParameter` contains a Constructor-relative body span and the existing
`ValueParameter` scalar projection: flags/name/type_id/vararg_element_type_id.
The caller separately selects supported Class kind1/mv[2,3,0] field8/wire2.
Bytes alone cannot establish metadata kind/version/container or full initialization.
No production caller, semantic binding, constructor symbols or server provider.

The existing Constructor decoder first scans the COMPLETE parent, including late
malformed fields/packed IDs and all original vector charges. Its private shared
budget extraction preserves the public decoder wrapper and original scan verbatim.
Only after parent success are children decoded in encounter order with the existing
private ValueParameter decoder and the same wire budget. Child-local errors are
rebased exactly once by body.span.start; parent errors remain unchanged. Required
child-name validation precedes typed-directory allocation. Empty/malformed children
can now fail this separate composition API while the original raw decoder remains
opaque and unchanged. No partial output or early successful return.

Output consumes the parent opaque directory and retains ONE typed directory.
Raw/flags/version IDs move unchanged. Temporary opaque-vector logical capacity
remains conservatively charged; typed-vector growth adds charges without refunds
or counter resets. Returned raw bodies borrow exact original slices and preserve
unknown/wrong-wire/duplicate fields. Signed scalar conversion and independent
last-recognized-wire behavior come from the unchanged existing decoders. IDs are
not looked up, validated, substituted or interpreted. Inline Type/annotation and
deeper bodies stay opaque, even malformed; singular nested messages are not merged.

## One aggregate operation budget

Reuse the existing four-field Constructor Limits and string_table::Budget;
no new limits, node/depth charges, helper redesign or dependency. Bytes admission
first, min(caller,16MiB); records65535, work64Mi, owned16MiB hard caps unchanged.
Let N/F be parent bytes/fields; P parameter count, each n_i/f_i child bytes/fields;
V version IDs, K packed items and L packed-body bytes; B=sizeof(ValueParameterBody),
T=sizeof(SelectedValueParameter), I=sizeof(i32), C(0)=0 else next_power_of_two:

records = F + K + sum(f_i)
owned = C(P)B + C(V)I + C(P)T
work = N + 2F + L + K + V + owned + sum(n_i + 2f_i).

Existing push growth at len0/power-of-two charges slots=max(len,1), checked
capacity arithmetic, aggregate owned, work(bytes), then try_reserve_exact/append.
Parent opaque pushes retain tag-start offsets; packed IDs retain item-start;
typed pushes use body.span.start AFTER child success. Record/work/read/dispatch,
short-read and raw64/nonminimal-varint limitations remain existing wire behavior.
Logical requested storage includes staging capacity, excludes allocator surplus,
reallocation churn/stack/RSS. Header/classfile/annotations/d1/payload/resolver calls
retain separate upstream budgets; no whole-pipeline work/memory/performance claim.
Allocation failure has static-review coverage only. Normal shallow input bounds
make hard owned/work exhaustion unreachable on usual layouts; tests distinguish
caller-tightened minima and hard clamp equivalence from exercised exhaustion.

## Frozen evidence and prospective tests

Frozen contract manifest SHA256
c15da686ef2a843a55f3faa3d973e3098199149398642f38c226696828fbd2e0 was approved before
source at historical6b. Adopted77bd changes only six public documentation files;
feature dependencies and frozen expectations remain unchanged. Fixture EXPECTATIONS
and provenance files are copied byte-identically. Tests transcribe their literal
results,115 prefix cuts,18 exact faults,4 extra positives, full tuples/spans/pointers,
aggregate minima/one-less/growth/precedence and finite hostile cases. The nonfinal
missing-name literal12 00 08 00 explicitly distinguishes childEOF2 from parentEOF4.

The genuine test reuses committed unchanged licensed Pair.class.bin through Rust
parse_class -> annotations -> Metadata header/d1 -> payload -> Class wire selection
-> this composition, with separate upstream budgets. It checks payload47..285,
field76..103/body78..103, flags70/no version IDs, exactly two tuples
(None,5,None,None)/(None,6,None,None), spans4..10/12..18 and raw pointers at d1
82..88/90..96. Pair covers absent parameter flags/IDs only; explicit scalar/version
cases are authored. Compiler provenance remains UNVERIFIED. Notices unchanged.
Test source/compilation is not execution evidence; local native tests remain held
under#96, with runtime behavior reserved for separately admitted hosted checks.

Parent #18/#68 remain open; localruntime HOLD/DarwinUNKNOWN, T008/T008a HOLD,
broaderC13/B3/B4 GAP and fullB11UNVERIFIED remain. No full initialization, name/type
lookup, generic graph, semantic/provider/full-parity or performance promotion.
