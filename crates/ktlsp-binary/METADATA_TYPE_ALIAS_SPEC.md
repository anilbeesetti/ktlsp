# Selected Kotlin TypeAlias data

Reads one unframed, caller-selected TypeAlias body from Kotlin v2.3.20-pinned
mv=[2,3,0] Class/Package metadata. The caller selects supported header/version and
Class field11 or Package field5 independently. No production consumer is added.
The bytes-only API cannot identify a message kind or enforce header/version.

`metadata::type_alias::decode(bytes, Limits)` returns borrowed complete `raw`,
required signed `name`, optional signed `flags`, `underlying_type_id` and
`expanded_type_id`, plus `underlying_types`/`expanded_types` borrowed body vectors.
Only wire0 fields1/2/5/7 are scalars; low32 interpreted as signed i32, no zigzag;
last recognized scalar wins. Absent flags has schema default6, absent IDs default0;
presence stays None, distinct from explicitly supplied values. Wrong-wire fields
never set, clear or overwrite selected values.

Wire2 fields4/6 retain EVERY inline Type occurrence in per-field encounter order,
including empty/duplicate bodies. Pinned protobuf singular message fields MERGE;
these vectors do not pretend to be merged Types and must not be last-wins. IDs and
inline bodies can coexist without precedence. Complete raw retains cross-field
ordering and every unknown/wrong-wire occurrence. Nested Types, typeparameters,
annotations, requirements, plugin data and extensions remain opaque. Malformed
nested bodies can succeed. No full TypeAlias initialization check, alias expansion,
name/type/symbol semantics, parent traversal or server integration is delivered.
Caller-constructed public values provide no validated-object guarantee.

## Budgets and errors

Limits bytes/records/work/owned_bytes default to hard ceilings16MiB/65535/64Mi units/
16MiB. Callers can tighten only. One existing wire Budget and one owned counter
span the ENTIRE scan and BOTH vectors. Byte limit is checked first. Every attempted
field pays one record and one record-work debit; actual wire bytes cost one work
each; completed fields pay one dispatch-work before interpretation. No root/node/
depth charge. No early success after name. All wire/limit errors precede the clean-
EOF MissingName check. Empty input with zero budgets yields MissingName at0.

Vectors grow at logical lengths0/1/2/4/8/... by max(len,1) additional slots. Checked
slot arithmetic and aggregate owned debit precede allocation-work debit and
try_reserve_exact; failures report Wire(Limit(owned_bytes/work/allocation)) at tag
start. One slot costs size_of::<&[u8]>() requested owned bytes and work. No body
copy, length-derived preallocation, extra retained-record charge or budget reset.
Logical requested capacity excludes allocator surplus, churn and RSS.

ErrorKind is Wire(existing wire ErrorKind) or MissingName; offsets are relative to
the supplied body. Structural wire failures retain existing offsets/precedence.
MissingName reports clean EOF. Existing nonminimal/low32/low64 slow-path wire
policy and group rejection remain unchanged; no universal buffer-dependent JVM
compatibility/general protobuf-conformance claim follows.

## Frozen source and fixtures

[EXPECTATIONS.md](tests/fixtures/metadata_type_alias/EXPECTATIONS.md) is the exact
preimplementation contract approved by architect/functionality/adversarial review,
SHA256 e637048f7a70760644f56db28e969372984bc790daf85543eea283f167883e8e.
It freezes source hashes, literal fields/signed values/wrong-wire cases, every cut,
coexistence orders, budget arithmetic/precedence, structural errors/offsets and
finite hostile generation. [Fixture provenance](tests/fixtures/metadata_type_alias/README.md)
records data-only source pins and unchanged notices. No JVM/compiler/oracle ran.

Parents #68/#18 remain open. This is selected binary declaration data, not full
Kotlin metadata support, alias semantics, semantic parity or performance evidence.
