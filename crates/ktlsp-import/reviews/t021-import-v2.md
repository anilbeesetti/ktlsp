# T021c / #171: finite importer-v2 review evidence

Frozen base `941c72efd9896b0847ab342ec0dd22c10a000f3b`, tree
`c9a8c8ae5ee4984b6b21849007bafbeebea33975`. Parent #21 remains open.
Root admitted the six importer-only paths, additive public v2 API and shared
private decoding after reviewing the independently frozen contract and actual
architect/correctness/adversarial approvals. Project v2, existing importer tests,
JSON visitor, Cargo/lock/workflow/server and root status remain unchanged.

Preparation froze the literal 12-entry ordered sidecar, 10-edge intermediate
projection, all eight library scope/export tuples, module/library namespaces,
default/null/error behavior, separate input/node/path/storage limits and the
2 MiB serialization guard before implementation. Correctness review clarified
valid trailing-whitespace removal and froze intermediate keys/default omissions;
the same three reviewers rechecked and approved revision 2 before source admission.

Shared decoding preserves old schema traversal, Unsupported collection and v1
manifest shape; only v2 permits scoped/exported library edges. V2 filters only
moduleSource from the native projection and delegates graph/tuple validation to
the delivered project v2 loader. Its capped writer checks cumulative serialized
length before buffer extension. Independent frozen fixture comparison and new
regressions cover ordering/roots, duplicate tuples, namespace resolution, graph
and ownership failures, malformed/unknown/atomic unsupported fields, v1 source
compatibility and byte/caller/node/depth/path/escaped-projection boundaries.

The same correctness/functionality and adversarial edge/crash reviewers plus
architect approved the source statically. Adversarial review caught an unreachable
aggregate-path probe: a 2 MiB literal path inside larger JSON would hit the input
byte guard first. Replaced it with short macro input and a 699050-byte caller
base; independently asserted content+source charges 1398103 bytes and adding the
library charges 2097155 bytes, crossing the shared 2097152-byte cap. All three
reviewers rechecked the correction and approved with no remaining findings.
Production source was unchanged by this test correction. Both fixtures still
match their preimplementation literals. The implementation adds 14 regressions
(one private-manifest unit test and 13 integration tests on Unix).

Local `cargo fmt --all -- --check` and compile-only
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed, including
the changed test recheck; `git diff --check` passed. No local native runtime/test
ran under unresolved #96. Completed original CI approvals remain pending at this
writing. Original unchanged Linux Native Rust CI supplies the pinned format,
Clippy and full-workspace tests. The PR records exact head/check URLs, any first
failures/fixes and same-reviewer completed original CI approvals before requesting
the coordinator's exclusive merge lease.

Residual limits: data-only pinned source-contract subset. No server provider or
semantic module visibility; SDK/resources/exclusions/facets/plugins/compiler/
build metadata, special/module-owned libraries, symlink/foreign-platform path
parity, build-tool execution and official oracle remain pending. Logical byte/
node/path/output ceilings do not promise allocator capacity or RSS bounds.
No full T021/import or platform-parity claim; parent #21 and #96 remain open.

## Native Windows #329: remove the private-format snapshot

The original changed-source native-platform
[run 38884260319 / Windows job 116918089686](https://github.com/anilbeesetti/ktlsp/actions/runs/38884260319/job/116918089686),
attempt 1, passed source admission, format and strict Clippy. Artifact lookup
and binary targets passed before the importer library target failed:
`tests::v2_intermediate_manifest_matches_frozen_literal` unwrapped a caller
Schema error at `src/lib.rs:792`. The hard-coded `/ktlsp/t021` directory is
rooted-only, not absolute on Windows. Original importer unit results were
three passes and one failure; later importer integration targets were not reached.
The original job's source/workflow SHA was
`96f4b9562536ad34eefdfc6275c630dcbcb6961c`, with event head
`b2417bed7387c34d6d52afcf45e97a1d3e99b1af` and base
`abe98694226c86fced3aed31691b8f0945a565e0`.
The retained log `ktlsp-platform-116918089686.log` has SHA-256
`49a721dcdd7dcb1575f2a03d9f178d8d39ca1032129929715ab58d350d4e0095`.
No unchanged retry or historical line-ending cause is claimed.

The failed unit test only compared private `parse_workspace` intermediate
`Value` keys/default omissions with a literal. That incidental implementation
contract is removed, not repinned to a Windows directory. Its private projection
fixture is deleted, and the README no longer promises that format. This supersedes
the historical private-format freeze recorded above; the real public imported
sidecar, native projection semantics and serialized/reloaded native model remain
the contract. The integration test now asserts ordered public edge identities,
scopes/exports, source memberships/kinds, library paths and roundtrip retention
directly, without comparing the private intermediate representation. The
exhaustive-error-match compile-only wiring helper is also removed; actual v1
strictness and compatible-result behavior remain exercised.

The same-cause audit replaces Unix-only success-case bases throughout importer-v2
tests with native absolute paths. Logical caller/expanded-path limits retain
their exact byte arithmetic by padding from the actual native root. The escaped
projection boundary calculates native path/separator JSON escaping independently
from its input, preserving exact-cap versus one-byte-over error expectations
without increasing limits. The Maven oversized-caller probe likewise uses a
native absolute base. V1's accepted absolute input uses forward-slash spelling of
its real native prefix, preserving the documented rejection of backslash input.
Intentionally invalid relative/NUL inputs and Unix-only non-UTF-8 controls remain.

The production absolute UTF-8/non-NUL guard, path policy, parsers, APIs and all
resource limits are unchanged. This source correction does not claim changed
Windows/Cygwin execution, full import/platform parity, local parser/server
execution, a push, dispatch or merge. ROOT composes the reviewed changes for a
separately admitted original changed-source platform run.
