# T022 static Maven POM/reactor subset

Frozen base: `45619398c7d8d5ca7b59acc5178acb4b208a67ee`. This child of
T022/#22 adds a data-only API; it does not complete Maven import or add a server
provider. Existing completed research (`docs/research/official.md`, Maven rows;
`community.md`, project-import boundary) and `docs/specs/project-model.md` apply.
Maven [POM reference](https://maven.apache.org/pom.html) supplies the explicit
coordinates, jar packaging/compile scope defaults and Java source-directory
defaults. Fixtures are independently authored, not copied upstream.

## Owned contract

Own `src/maven.rs`, `tests/maven.rs`, `tests/fixtures/maven/*`, this note, and one
additive `pub mod maven;` export in `src/lib.rs`. Dependency/lock changes require
coordinator clearance. No change to project crate or JSON import v1/v2 behavior.

`maven::from_poms(absolute_project_directory, &[Pom { path, bytes }])` consumes
caller-supplied UTF-8 documents, with root `pom.xml`. POM keys are slash-separated
relative paths ending in `pom.xml`; module entries are relative directories.
Reject empty/dot/parent/absolute/drive/backslash/URI components and controls;
all source paths stay lexically inside their declaring POM directory. No file
access, symlink resolution, glob discovery, external tools, downloads or network.
Caller byte snapshots do not prove filesystem/symlink containment. Caller base
must be absolute UTF-8/non-NUL without parent components; dot/separator spelling
is normalized lexically before retained paths are constructed.

Accept only modelVersion 4.0.0, required explicit literal groupId/artifactId/
version, packaging absent/jar/pom, explicitly listed modules, dependencies and
build sourceDirectory/testSourceDirectory. Coordinates use ASCII letters,
digits, dot, underscore and hyphen; reject empty/dot/parent, case-insensitive
SNAPSHOT suffixes and LATEST/RELEASE versions, ranges and interpolation.
Timestamped snapshots remain literal; no repository resolution or immutability
authentication is claimed. Aggregators require packaging pom.
Each child must have explicit coordinates (no parent/inheritance). Traverse in
root-first declared preorder; require every supplied POM reachable exactly once;
reject missing/duplicate links and coordinates. Reject self/cyclic module paths
at path validation; an iterative visited-set traversal also rejects reused POMs.

Jar roots default to src/main/java and src/test/java. Explicit paths override
those defaults. Pom packaging has no source roots and rejects build root fields.
No Kotlin source roots are guessed: plugin-derived Kotlin/generated roots are
outside this contract. Keep ordered unresolved dependencies with literal GAV,
compile/test/runtime/provided scope, scope-default presence, POM path and byte
range. Dependency type absent/jar and optional absent/false are accepted;
classifiers, exclusions, system/import scopes and all other fields block.
Even coordinates matching reactor modules remain unresolved; no binding,
transitive resolution, version mediation or artifact/library synthesis occurs.

Expose a separate Maven sidecar plus a validated **source-only** ProjectModel v1
projection. Projection contains modules/source roots, empty libraries/edges/SDKs
and unset language settings. Consumers must retain the sidecar. Omitted Maven
resources, outputs, lifecycle/default plugins, Super POM/repository policy and
external settings are explicitly outside this projection, not verified empty.
Absent dependencies/modules mean empty declared lists; absent build means only
the stated Java roots. Unsupported fields including parent, properties,
dependencyManagement, profiles, plugins, repositories and descriptive metadata
reject atomically with location/reason; never return a partial successful model.

## XML and bounds

Coordinator-approved dependency: quick-xml exact 0.41.0, default-features=false,
no features.
Pure Rust normal closure is memchr (already locked); build=false, no enabled
build dependencies; upstream dev dependencies are not consumed. Coordinator
cleared the exact import Cargo/lock mutation. Streaming NsReader uses explicit depth/event limits before
accumulation, end-name checks and checked comments. UTF-8 XML 1.0 only; one root
project, no element prefixes; root namespace absent or Maven POM 4.0.0. Root
xmlns:xsi/schemaLocation metadata may use the standard Maven namespace/XSD
pair; no schema is fetched. schemaLocation requires both Maven/xsi bindings.
No other attributes or namespace redeclarations. Attribute/pseudo-attribute
separators must be XML whitespace. XML 1.0 characters are validated across all
input bytes; raw text `]]>` and references in metadata attributes reject.
Reject duplicate fields/attributes, non-whitespace container text, scalar mixed
content, DTD, processing instructions, CDATA and all entity/reference events
(including predefined references: a deliberate literal-input subset). Comments
and XML whitespace are allowed. XML declarations, if present, must be first,
version 1.0, optional UTF-8 encoding and optional yes/no standalone only.

Caps before growth: 128 POMs, 64 KiB per POM, 1 MiB aggregate POM bytes,
8,192 events per POM, depth 32, 4 KiB caller/relative/expanded path length,
1 MiB aggregate expanded paths, and bounded 2 MiB native JSON projection.
No recursive parser/reactor walk; the bounded temporary tree retains parsed
elements (including unknown elements until atomic rejection) and has depth at
most 32, including recursive destruction. Every retained POM provenance clone
is charged against aggregate paths before allocation. Logical caps are not an
allocator/RSS guarantee. ProjectModel retains
its own overlap/path/serialized-input validation; a projected invalid source
ownership model rejects the entire call.

## Acceptance and evidence

Fixtures freeze root-first reactor ordering, literal coordinates, Java default
and explicit roots, all four scopes with scope presence and source ranges,
repeated unresolved coordinates without coalescing, and source-only projection.
Negative/boundary coverage: malformed UTF-8/XML, duplicate fields/attributes,
mixed content, namespace/encoding/declaration errors, interpolation and every
unsupported subtree family, DTD/internal/external entities, deep input, missing/
duplicate/unreachable POMs, path escapes/cycles, bytes/events/depth/path budgets,
and source-root overlaps. Aggregate retained-provenance and exact private path/
projection writer boundaries have targeted regressions. Existing import
regression suites stay unchanged.

Local native execution is prohibited by #96 hold. Allowed checks: format and
compile-only Clippy with CARGO_BUILD_JOBS=2 after coordinator compile-slot grant.
Original unchanged hosted Native CI must run pinned format, locked workspace
Clippy and locked workspace tests. Independent correctness, adversarial and
architect review outcomes and original CI identity will be recorded here before
delivery. No oracle, JVM, Maven, Gradle or product binary is run locally.

Child issue: [#271](https://github.com/anilbeesetti/ktlsp/issues/271).

Actual retained source reviews: correctness/functionality, adversarial edge/
crash, and architect each approved the frozen contract, identified source
findings, and rechecked the applied fixes with no remaining actionable finding.
Fixes cover slash-based logical reactor keys, case-insensitive mutable versions,
required XML attribute separators, and aggregate retained POM provenance paths.
Review source SHA-256: `maven.rs`
`41a6027d786a9f269e0e4123caeb2d69b0d9953667d9f1c864d9580394c6ab14`;
`tests/maven.rs`
`48b76acd6af7aa5a46fc9c30724c557ef829e0d5b3f30b9b467c3afcb3265df0`.

Local validation: first format check failed on unformatted newly authored
source (exit 1); `cargo fmt --all` corrected it and subsequent pinned format
checks passed. Both the first source and final changed-source compile-only
`CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings`
passed (exit 0), under separately granted coordinator compile slots. No local
native test binary ran. Metadata-only offline lock resolution changed only the
ktlsp-import quick-xml edge and new quick-xml package/checksum; `cargo tree
--locked --offline -p quick-xml --edges normal,build,dev,features` showed only
existing memchr 2.8.3 with its std/alloc features, no build/dev dependency.

These are source/compile approvals. Original unchanged hosted Linux Native CI
and exact integration receipts remain delivery gates, recorded with the PR by
the owner/coordinator; this note does not claim an unobserved run. Windows/macOS
runtime behavior, effective Maven models, live import, symlinks, dependency
resolution and semantic/server consumers remain unverified or outside scope.
