# One-level TypeAlias/Type occurrence composition

Source is prepared at assigned base77bd0ac8b4e74dadd037a3404b82dba7e004c395.
Runtime validation is UNVERIFIED; source/compile permission does not admit test execution,
publication, CI or merge. The exact frozen preimplementation expectations are
[EXPECTATIONS.md](tests/fixtures/metadata_type_alias_types/EXPECTATIONS.md), SHA256
3576817faaa184ecf42d275d0acd4f0fcca03a7dea36db3f0a74112d5b5edbdb.

`metadata::type_alias_types::decode(bytes, string_table::Limits)` returns
`TypeAliasWithTypes`: borrowed complete raw, optional flags, required name,
optional underlying/expanded IDs, and ONE Vec of `SelectedType { field_number,
span, header: TypeHeader }`. Field identity is exactly4/6; spans are body-only,
TypeAlias-relative. Parent and children borrow pointer-identically from input.
The API accepts no caller-created parent or spans; public outputs are constructible
and only decoder-produced values have the documented correspondence guarantee.

Caller independently selects supported mv=[2,3,0] Class field11/Package field5;
bytes alone cannot validate kind/version/full initialization. Existing raw parent
reader is unchanged: recognized scalar fields1/2/5/7 wire0 use signed low32/last
recognized wins, with presence preserved. Wrong-wire/unknown/deeper bodies stay
raw. Child TypeHeader retains its selected optional scalars, including inherited
raw64 Boolean behavior. IDs stay uninterpreted and coexist with inline bodies.
Every inline occurrence, including duplicates/empty bodies, is returned in wire
order and UNMERGED. No alias expansion/type-table/name lookup/substitution/graph,
semantic consumer/provider, full Kotlin/LSP parity or speed/RAM claim.

## Validation, ordering and errors

Check byte ceiling first. ONE existing Budget spans complete parent decode,
both opaque staging vectors, composition and typed vector. Complete parent wire/
MissingName validation precedes EVERY new composition charge. Parent failures
pass through unchanged; root node(1,0) follows successful parent validation.
Consume the two individually ordered staging vectors with owned iterators;
retain neither directory in output and never copy bodies or reparse parent wire.

Recover head body spans with checked pointer-address subtraction and checked
start+length, bounded safe parent slicing. No unsafe/offset_from, content search,
sort or caller-forged parent. Empty-body pointer positions, including EOF, are
preserved. With both heads, choose smaller start (defensive tie field4) and charge
comparison work1 at selected body start. Charge occurrence work1 every iteration,
then consume one entry, child node(2,body start), decode TypeHeader on shared wire
budget, then typed push at body start. Child wire errors rebase once with checked
addition bounded by child end. Parent/merge/node/typed errors are already parent-
relative. Defensive invalid internal span/rebase => Wire(Truncated) at parent len.
Malformed child precedes its typed reservation; parent staging precedes children.
No partial success. Original parent staging reservation errors use field TAG start;
typed reservation errors use BODY start. Existing Error/ErrorKind are reexported.

## Monotonic budget and memory

Reuse unchanged Limits defaults/clamps16MiB bytes/65535 records/65535 nodes/depth2/
64Mi work/16MiB requested storage. Parent and both staging vectors, all child scans,
comparison/occurrence/node charges and typed growth share counters without resets
or refunds, including after staging allocation drops. Temporary staging/output
allocations can overlap. Existing growth at lengths0/1/2/4/8 adds1/1/2/4/... slots;
owned debit precedes reservation-work then try_reserve_exact. No preallocation.
Storage excludes allocator surplus, reallocation churn, stack and RSS. Header/d1/
payload/resolver/upstream pipeline budgets remain separate.

P=size_of::<&[u8]>(), S=size_of::<SelectedType>(), Q(0)=0, Q(n>0)=power-of-two ceil.
N/F parent length/fields; K=n4+n6; M/C total child bytes/fields; H comparisons:
records F+C; nodes1+K; depth1 without child else2;
owned P*(Q(n4)+Q(n6))+S*Q(K);
work N+2F+M+2C+(1+K)+K+H+owned. H<=max(K-1,0).
Work is logical accounting, not instruction/latency measurement. No independent
hard-work/owned/node runtime witness is claimed when preceding caps dominate.

Literal fixtures/provenance are [documented here](tests/fixtures/metadata_type_alias_types/README.md).
All ten are authored, not genuine/compiler-generated TypeAlias positives; this
raw composition lane has no genuine positive. The separate #383 nominal signature
consumer uses the existing genuine `metadata_package_member_names/KTypeParameterBaseKt.class.bin`
inline-TypeAlias fixture; this does not broaden this API's raw occurrence contract.
Existing source pins/notices and compiler provenance limitations are preserved. No Kotlin compiler/JVM/oracle
invocation or new artifact retrieval. #18/#68 stay open; #96 local runtime HOLD/Darwin UNKNOWN; T008/T008a HOLD,
C13/B3/B4 GAP, full B11 UNVERIFIED unchanged.

## Inherited source pins, data only

Source references are reused from the committed raw TypeAlias/TypeHeader specs and
the preimplementation contract; no new upstream artifact/source download or
upstream source execution.
Kotlin v2.3.20 commit d57eb4a26ed01ab03cf195da686c8047032ff16f:
- metadata.proto Type121..175 / TypeAlias450..474, blob
  18cfd13e9e512c491f2f62ec241f49c81d6e5e81, SHA256
  a08e508cc7d98cc225e8b0df7290724b545a0250f7d9c91460cfc1244a3c08e2.
- ProtoBuf.java Type parser4944..5065 / TypeAlias25229..25282, blob
  fc424d5e7aac8661deb862c8e791d4fcf779c194, SHA256
  da7fa6f03c1afa3499086cb5a1f92471e3f327c8f43298ea8d9548b7e4753716.
Upstream TypeAlias singular inline messages merge; this API intentionally retains
separate UNMERGED occurrences. Existing wire/raw64 provenance and notices remain
authoritative; no compiler provenance is upgraded by authored fixture success.
