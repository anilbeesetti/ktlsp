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
