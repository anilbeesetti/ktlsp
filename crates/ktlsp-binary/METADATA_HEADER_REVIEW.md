# T018a1 / #97 delivery evidence

Parents #68/#18 remain open. Scope and source pins are in METADATA_HEADER_SPEC.md;
source hashes/license and data-only fixture provenance are in
tests/fixtures/metadata_header. No external dependency/Cargo/lock/runtime change.

Architect reviewed the plan before implementation: exact mv=[2,3,0], kinds1/2/5,
raw UTF16 preservation, explicit unsupported outcomes, schema defaults and
aggregate streaming budgets. Owner verified exact pinned v2.3.20 encoding source
before coding (architect initially could fetch only master).

Independent correctness and adversarial reviewers inspected source separately.
Correctness independently fetched the pinned upstream files; adversarial compared
their locally cached exact contents. No actionable source defects reported yet.
Their test-coverage suggestions (typed duplicates, exact work threshold, hard cap)
were added and pass the eight metadata-header integration tests.

Coordinator granted a short sequential binary-only check slot after #96's first
observation. `cargo fmt --all -- --check` passed after formatting;
`cargo clippy --locked -p ktlsp-binary --all-targets -- -D warnings` passed;
`cargo test --locked -p ktlsp-binary --test metadata_header` passed 8 tests in 0.02s.
Adversarial reviewer independently compiled six native tests against the current
rlib: exhaustive 65,536 direct UTF16 units and 65,536 legacy pairs, independent
bit-index reference lengths0..255, repeated-borrow hard codeunit cap despite
usize::MAX caller limit, duplicate-comparison work cap and opaque unknown/raw
identity. All six passed in 0.29s. Correctness independently ran the binary crate's
28 tests (annotations9/classfile11/metadata8) and an upstream-indexed reference
probe: 304,768 comparisons passed in 1.05s, covering exhaustive 256x256 low-byte
pairs, multipart/empty-leading chunks, high UTF16 and lengths0..64 at every split
in all three encoding forms. Both independent reviewers approve the implementation
with no actionable findings; all local jobs are idle. Architect's completed-code
review also found no blockers. No JVM/compiler-oracle execution or parser tests
ran in this slot.

Reviewed source SHA256:

- header.rs: `32629239e817f7078f5fdc28a10e097c0c0c0c32a68ea71ccc7c0faf07a4a70f`
- encoding.rs: `f0a77cd69abe451cc0976e5b22829cec62be38107ff07526dcc5625e0a288fec`
- mod.rs: `b065239c0bef97793ffbf49d6f3889a19c04e1a03541f90b2a34d8f6da21251e`
- metadata_header.rs: `88547e8099c45fa60c580cf406894629fce26bb6749237849e7fe1074fd1450a`

Temporary independent probes are review evidence, not required native test graph
dependencies: `/private/tmp/ktlsp97-adversarial.rs` and
`/private/tmp/ktlsp97-correctness.rs` (SHA256
`d771783344583d1fc7267153e4eb04535cdc779f5a2d6d8ab3b37c96e2f4367f`).

These results are historical evidence at `b9875cadf7fca6cd8eab412171eef97251117316`.
That head also passed Linux native CI (163 tests, two ignored):
https://github.com/anilbeesetti/ktlsp/actions/runs/38266799465/job/114870907784.
They do not establish approval of later combined trees or resolve #96.

Final integration uses coordinator-granted main
`ab00d55557cfea9bcee87aa41fe2922f82770027`. Its only conflicting file was lib.rs;
resolution preserves annotations/signatures/nesting with one additive metadata
export. Metadata source/tests retain the hashes above. Current parser/vendor,
Cargo/lock, workflow and shared documentation are unchanged from that base.
No local Darwin runtime/build/checks or repeated reference probes are permitted
in this stage. The unchanged Linux Native Rust workflow runs all three pinned
workspace commands on the exact combined head. Final head/tree, CI counts and
URLs, independent behavior/resource and architect approvals, expected-SHA merge
grant and exact merged-main validation are recorded in
https://github.com/anilbeesetti/ktlsp/pull/98. This record does not substitute
historical passes for those final gates. #96 remains OPEN/UNKNOWN.

Limits: d1/header data only; legacy incomplete trailing bits deliberately ignored.
No protobuf/name resolution, declarations/types, other version/kind acceptance,
compiler compatibility or official Kotlin LSP parity. #96 checkpoint verification
remains unresolved independently of this child until coordinator evidence changes.
