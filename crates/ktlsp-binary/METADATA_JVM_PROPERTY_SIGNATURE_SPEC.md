# T018/#18 selected Property JVM extension100 — contract/data only

Frozen base f6e2c6b3a8e43a4a76ba27a01205f4e44b7e8e04, tree
a920a637bf947ae8fd6d386d32b27044ddb6fbcd, sole parent
cca3a61842f5d90d2315250cc8f75e131fda8a98. Managed checkout
/Users/anil/.codex/worktrees/t018-property-signature-contract/ktlsp;
branch codex/t018-jvm-property-signature-contract; initially and finally clean.
No product edit, compilation, native/parser/worker/server/test/bench/oracle run,
push, issue/PR publication or CI. Auxiliary Python reads existing fixture bytes
and checks literal data only; it never imports or executes product/upstream code.
Root grants source/API/export/publication/compile-only stages separately.

## Exact reach and smallest API

New exclusive prospective paths:
- crates/ktlsp-binary/src/metadata/jvm_property_signature.rs
- crates/ktlsp-binary/tests/metadata_jvm_property_signature.rs
- crates/ktlsp-binary/tests/fixtures/metadata_jvm_property_signature/**
- crates/ktlsp-binary/METADATA_JVM_PROPERTY_SIGNATURE_SPEC.md
- crates/ktlsp-binary/METADATA_JVM_PROPERTY_SIGNATURE_REVIEW.md

The sole later serialized shared change is additive
`pub mod jvm_property_signature;` in metadata/mod.rs. No existing callers change;
new tests compose existing parse_class/ClassFile::utf16, annotations::decode,
metadata::decode, payload::decode, wire::decode, property_header::decode and
name_resolver::Resolver::{new,get_string}. Those operations retain their own
budgets. The new reader never calls the public parent/header/table/resolver APIs.
All helpers, wire/Budget, Cargo/deps/lock, workflows, config, public/root status,
server/parser and other owners' files remain readonly. Nothing deletes user data,
writes extracted names, changes a provider capability or changes existing APIs.

Proposed metadata::jvm_property_signature public API:
- Limits { bytes, records, work, owned_bytes: usize }, tightenable hard defaults
  MAX_BYTES / MAX_RECORDS / MAX_VALIDATION_UNITS / MAX_BYTES.
- Error/ErrorKind reexport existing wire types.
- Reuse PR219's PUBLIC JvmMethodSignature { name, desc: Option<i32> } and
  SignatureBody<'a> { raw: &'a [u8], span: Range<usize> } by public reexport.
- Own JvmFieldSignature { name, desc: Option<i32> }.
- JvmPropertySignature { field: Option<JvmFieldSignature>, synthetic_method,
  getter, setter, delegate_method: Option<JvmMethodSignature> }.
- Kind { Field, SyntheticMethod, Getter, Setter, DelegateMethod }.
- MemberSignatureBody<'a> { kind: Kind, outer_ordinal: usize,
  body: SignatureBody<'a> }.
- PropertySignatureExtension<'a> { raw: &'a [u8],
  signature: Option<JvmPropertySignature>, bodies: Vec<SignatureBody<'a>>,
  member_bodies: Vec<MemberSignatureBody<'a>> }.
- decode(bytes: &[u8], limits: Limits) -> Result<PropertySignatureExtension<'_>, Error>.

PR219 inspected readonly at c28e8020369436b8a9e500110cf655db44315d7b,
/Users/anil/.codex/worktrees/t018-jvm-method-signature/ktlsp. PUBLIC type reuse
remains SOURCEHELD until its delivery/release and root coordination. No private
name/descriptor helper exists there; none is proposed for extraction/reuse.
The new reader uses unchanged wire::field/string_table::Budget directly. No
generic signature framework, new dependency or parallel shared-helper change.
Final released public-type reconciliation needs root admission before source.

## Source-derived rules

Existing source DATA pin Kotlin v2.3.20
d57eb4a26ed01ab03cf195da686c8047032ff16f:
- core/metadata.jvm/src/jvm_metadata.proto SHA256
  5a80e63021b800afd2297b3713c4cfc12739cbc8e62c726578e1c5d1dfc771f9,
  lines67..93/107..114: field/name/desc optional int32; five optional signatures;
  Property optional message extension100. No signature enum.
- core/metadata.jvm/src/org/jetbrains/kotlin/metadata/jvm/JvmProtoBuf.java SHA256
  b1ad864ae872315c06740d6551d2d7233060f5bff3e9ad9b8f80f531018bc3a1,
  lines3139..3202 preserve/merge prior selected nested messages via subBuilder;
  mergeFrom3549..3562 checks each presence. Field/method scalar merge checks
  hasName/hasDesc before assigning. Empty messages are initialized, present data.
- Protobuf2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c,
  java/src/main/java/com/google/protobuf/GeneratedMessageLite.java SHA256
  14c63c0c074008f24dcacf101ee09b19691bfaacbfc7422b7ddf9c2aa9bc964f,
  lines573..592 reuse the prior singular MESSAGE value's builder and store its
  merged value, independently confirming duplicate extension100 fieldwise merge.

Source copies in sources/ are temporary references, never shipped/executed.
Existing metadata_wire source/build relocation pins and documented deterministic
raw64/stricter framing limits remain. Preserve fixture Apache LICENSE/NOTICE and
protobuf BSD notice unchanged; no generated/runtime code enters native graph.

Caller selects ONE unframed Property body and independently establishes supported
mv=[2,3,0], kind/container and parent identity. This byte-only API cannot establish
those. It does not require Property.name, validate parent initialization or call
property_header::decode (that requires name and starts another budget).

Only parent(100,wire2) is selected. Within each selected JvmPropertySignature,
only fields1..5 with wire2 are selected, mapping exactly to the five Kind slots.
Within each selected leaf only (1,wire0) name and (2,wire0) desc are interpreted:
signed low32 (`u64 as u32 as i32`), not zigzag. Scalar last recognized occurrence
wins across duplicates both within/across leaf and outer messages. A later absent,
empty or wrong-wire field cannot erase earlier present scalar/slot data. All five
slots merge independently; neither whole outer nor whole leaf is replaced.

Absence extension=None/bodies empty; empty extension=Some with all slotsNone.
Empty selected leaf=Some(nameNone,descNone) for its slot; explicit0=Some0.
Parent wrong-wire100 and outer wrong-wire1..5 never create presence. Unknown and
wrong-wire fields are retained exactly through full raw/body slices, without a
separate unknown vector. Their field framing is validated but their bytes stay
opaque, including malformed-looking bytes. Selected leaves are scanned fully;
an invalid later selected body/parent rejects the entire result, no partial output.
No enum interpretation, flag computation, getter/descriptor/backing-field inference,
names/types/linkage/access-control/lowering/accessor semantics or server consumers.

## Borrowing, order and offsets

out.raw is pointer-identical to all supplied parent bytes. bodies retains every
recognized extension100 occurrence in wire order, including empty duplicates.
member_bodies retains every recognized leaf in outer-then-child wire traversal
order, including duplicates/empty leaves; outer_ordinal is its original bodies
index, not slot ordinal or symbol identity. This association is needed even for
empty outer messages. Every body.raw borrows bytes[body.span], no concatenation,
copy, reencoding or synthesized span. Span is body-only half-open Property-relative;
complete field spans are separately frozen in CASES/EXPECTATIONS for readback.

For a caller selecting Property body at decoded d1[p..q], translated body/field
span is p+start..p+end; error offset is p+offset. Translation is caller-side checked
arithmetic, no global coordinate mode. Each child cursor uses original parent
bytes and its checked body end: following sibling/parent bytes cannot complete a
short child. Parent framing completes before outer scan; outer leaf framing
completes before leaf scan. Leaf validates then append member_bodies at its leaf
tag_start; complete outer validates then append bodies at extension tag_start.
Merged data stays internal until whole-operation success.

## ONE operation budget and deterministic precedence

Input bytes >min(limits.bytes,MAX_BYTES) => Limit(bytes)@0 before any scan.
Instantiate ONE unchanged string_table::Budget with records/work/owned mapped
from Limits, other fields Default; no node/integer/depth operations. Every outer
Property, property-signature and selected leaf field uses the SAME budget.wire.
wire::field charges record1/work1 and each byte; after a completed field, charge
work1 at field tag_start BEFORE dispatch. Scalar/presence/ordinal assignments are
covered by dispatch; no extra conversion or merge charge. Unknown/wrong-wire
fields and overwritten scalars still incur visits. Only selected bodies recurse,
to depth two; no input-dependent recursive graph or length-based allocation.

Append each retained leaf/outer occurrence with Budget::push AFTER its child
scan. At len0/powers2 request slots=max(len,1); checked len+slots, checked slots*T,
aggregate owned subtraction, then work(slots*T), then try_reserve_exact and append.
Allocator surplus changes no logical charge. Leaf push offset is leaf field start;
outer push offset is parent100 field start. Failure returns no partial value.

Let N=parent length; F=Property field count; G=all selected outer body field counts;
H=all selected leaf field counts; L=sum outer body lengths; K=sum leaf body lengths;
O=recognized100 count; I=recognized leaf occurrence count; B=sizeof(SignatureBody),
M=sizeof(MemberSignatureBody), C(0)=0 else next_power_of_two.
Complete success requires records F+G+H,
work N+L+K+2(F+G+H)+C(O)B+C(I)M,
owned C(O)B+C(I)M. Each vector grows independently but charges ONE shared bucket.
Raw bytes are reread/metered at each selected level. Exact values/one-below offsets
freeze before any runtime. No charge reset/refund per outer/leaf or per vector.

Precedence: byte gate; each attempted field record, record-work, tag/value reads,
wire structural checks, dispatch; selected descendant traversal; descendant vector
capacity arithmetic/owned/work/allocation; ancestor append; parent continuation.
Malformed descendants precede the ancestor's owned failure. A vector allocation
failure reports Limit(allocation) at that vector occurrence tag; static review
only, no allocator failure injection is promised. Empty bytes with all0 succeeds.
Limits usize::MAX cannot raise hard constants (16MiB/65535/64Mi/16MiB).

Wire inherited unchanged: accepted nonminimal varints up to10bytes; tags/lengths
low32, scalar low64 then signedlow32; zero field invalid before wire dispatch;
groups3/4 UnsupportedGroup; wires6/7 InvalidWireType; signedlow32 negative lengths
NegativeLength at length start; tenth continuation MalformedVarint at tenth byte.
Failed bulk read charges available+1 at read start, then Truncated at cursor end;
insufficient work wins before that truncation. These bounded rules are not a
universal JVM slow/fast-buffer compatibility claim.

## Existing genuine evidence and exact residual gaps

Reuse existing metadata_constructor/Pair.class.bin unchanged (no duplicate class),
2983bytes SHA256 8fed0fa88eb2df5723812a95330dee6bc91fd66b13906acb7906f9a813da9736.
org.jetbrains.kotlin:kotlin-stdlib:2.3.20, entry kotlin/Pair.class;
JAR SHA256 0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
Existing PAIR_EXTRACTION.json and t017c2/provenance.json preserve origin/license.
Distribution2.3.20-release-208 is not compiler proof; compiler remains UNVERIFIED.

PAIR_PROPERTY_EXTRACTION.json freezes source CP offsets, decoded d1/hash, both
Property bodies, extension/leaf field+body spans, and actual encoded fields/methods.
Both Property signatures have field{nameNone,descSome11}; getters respectively
{nameSome9,descSome10} and {nameSome12,descSome10}. No other slots present.
The current UTF16 resolver must resolve explicit9/10 -> getFirst/()Ljava/lang/Object;
and12/10 -> getSecond/()Ljava/lang/Object;, matching actual ClassFile.methods
Member.name/descriptor through ClassFile::utf16; future test asserts one exact tuple
per getter. Resolve explicit11 -> Ljava/lang/Object; and compare descriptor units
with actual ClassFile.fields. This is descriptor-only evidence: absent field name
staysNone; do not use Property.name, ordering, getter name or guessed backing field
to complete it. Field identity GAP; synthetic_method/setter/delegate_method have
synthetic literal coverage only. Genuine full field name+descriptor tuple absent
in selected Pair; no new artifact/compiler/oracle is generated to hide the gap.

Future test uses actual unchanged classfile -> annotations -> header/d1 -> typed
payload/table -> flat Class Property field selection -> new selected reader ->
current Resolver.get_string -> actual encoded member tuples. All other operations
are separately bounded. The temporary data transcription is checked; Rust API
composition remains UNEXECUTED until original Linux CI after root source grant.
Encoded equality establishes neither accessor/backing-field identity nor semantic
parity/performance/JAR-to-symbol/server integration.

## Acceptance and gates

CASES.json/fixtures/*.hex/EXPECTATIONS.md freeze literals, values, full field/body
spans, duplicate ordering/outer association, prefix cuts, malformed cases and budget
thresholds before runtime. Future focused tests assert raw pointer identity and
current resolver/member equality, exact/one-below limits and inherited hard caps.
Finite deterministic hostile tests: all65536 two-byte inputs plus uniform byte
length0..31, limits32bytes/8records/256work/1024owned; deterministic result/no panic,
error offset<=length, spans/pointers/outer associations valid. Finite probes are
not all-input proof, RSS bounds or performance evidence.

SAME3 reviews: /root/architect, /root/correctness, /root/adversarial, actual reviews
and changed-expectation rechecks recorded in REVIEW.md. Issue draft stays unposted.
No source/API/export/compilation/publication until explicit coordinator grant;
PR219 public reuse must first be released. Later required original Linux Native
Rust CI uses unchanged pinned cargo fmt --all -- --check;
cargo clippy --locked --workspace --all-targets -- -D warnings;
cargo test --locked --workspace. Darwin native runtime remains HELD under#96.
Source/completed original CI review, exact expected-head/base lease, fresh separate
strict readonly exit0 guard then separate ordinary expected-head merge are later
gates; any failure suspends lease. No force/admin/bypass/settings/main mutation.
OWNER+ROOT verify original main tree/sole parent/CI before delivery/archive.

Parents#18/#68 OPEN; T008/T008a HOLD; B3/B4/broaderC13 GAP; fullB11 UNVERIFIED;
#7/#83/#96 OPEN, Darwin UNKNOWN. This packet claims no feature delivery/full parity.

## Source admission reconciliation

The coordinator granted source, exclusive new property paths, sole additive module
export, format and compile-only Clippy after PR219 release. The clean owned branch
fast-forwarded ONCE to bb64eb5b5200ef801ffb224336bf2eb997b68054, tree
0ebc83052425eefe3747d3f99a3e02e5c6a2190a, sole parent
048c344c5a75f7cf16cadc627c7ff765c025c7f7. SAME3 independently reconciled the
released PUBLIC types and unchanged wire/Budget/resolver/classfile/Pair inputs.
The historical contract/data gates above describe the preimplementation freeze;
source admission is now fulfilled without expanding its47valid/24malformed scope.
Native runtime remains held under#96; original unchanged Linux CI is authorized.
No merge is admitted. No private helper extraction or dependency changes.
