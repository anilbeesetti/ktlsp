# T021a: pinned workspace.json subset

Child [#63](https://github.com/anilbeesetti/ktlsp/issues/63) of T021/#21. Existing JSON/JPS/Maven model producers remain data-only Rust APIs. The separate opt-in `source_inventory` API below adds bounded read-only native filesystem discovery for #421; it does not change `from_json` or invent effective import/build semantics. This is source-contract compatibility, not official runtime equivalence. T021 remains open.

## Provenance

Official baseline `kotlin-lsp/v263.6379.0`, commit `4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748`:

- [model.kt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/src/com/jetbrains/ls/imports/json/model.kt), Git blob `6f0bfd6b930c3726b26850de3bd06f76345c4a48`.
- [conversion.kt](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/src/com/jetbrains/ls/imports/json/conversion.kt), Git blob `e257a64ee143b39654703b1c42facfd3b298c969`.
- [JpsJavaModule fixture](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/test/testData/jps/JpsJavaModule/workspace.json) and [JsonImporterUnitTest](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/workspace-import/test/test/com/jetbrains/ls/imports/JsonImporterUnitTest.kt) establish original field/macro/root/edge intent. Our small fixtures and implementation are independently authored, not copied upstream tests.

Upstream source notice: Copyright 2000-2025 JetBrains s.r.o. and contributors. Use of that source code is governed by the [Apache 2.0 license](https://github.com/Kotlin/kotlin-lsp/blob/4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748/LICENSE).

## Public contract

`from_json(absolute_project_directory, bytes) -> Result<ImportedWorkspace, ImportError>`. The official input is **unversioned**. Version/schemaVersion keys, even 1, are unknown fields. No native manifest version is expected in official input.

The complete supported result is `ImportedWorkspace`: ordered modules, content-root boundaries/source membership/exact root type, full dependency order (including moduleSource position), and library-root order/type, together with `project_projection()`. The latter is a validated ProjectModel v1 projection and **alone loses import semantics**. Later consumers must use the whole imported result. This import-local representation does not change ProjectModel v1.

Complete JSON-v1/v2, JPS and Maven import results retain immutable loader-produced
records. Use borrowed `modules()` and (JSON) `libraries()` slices; the former
public vectors are intentionally no longer mutable fields. Nested record types
remain ordinary public data, but shared result views cannot mutate retained
order, roots or provenance independently of the validated projection. Standalone
project-model clones remain mutable and must pass the normal loader validation
before reuse. No replacement API, duplicate catalog or workspace provider is added.

A separately admitted external native public consumer at reviewed2b739597/
treea0c514f9 compiled0/ran0 with empty stderr/PATH. OWNER+ROOT fully read actual
JSON-v1/v2 records/projections, invalid detached-clone input and real
UnknownReference refusal, subsequent genuine Commons Lang class selection with
original owner/library/artifact pointers, and complete JPS/Maven XML/records/
provenance/source-only projections. All630 binary/import data tests passed,
including seven compile-fail import authority checks. Full original consumer
source/output/commands/executable remain `/private/tmp/ktlsp-411-412-native/`;
only its throwaway Rust source was removed after both complete readbacks.
This is neither a prior dependency visibility bypass nor server workspace parity.

Supported fields/defaults follow the source contract:

- Workspace modules/libraries default empty. sdks/kotlinSettings/javaSettings may be absent or empty; externalSystem absent/null.
- Module name required; type absent or JAVA_MODULE. coordinate/externalProjectPath/externalProjectId absent/null; facets absent/empty. Ordered contentRoots/dependencies default empty.
- Content path required; excludedPatterns/excludedUrls absent/empty. Source roots default empty; each requires path/type. java-source/kotlin-source map to production and java-test/kotlin-test to test, with exact original type retained. Generated/resource/custom root semantics are unsupported.
- Module dependencies retain required name/scope (compile/test/runtime/provided), default-false isExported, and default-false isTestJar. True isTestJar blocks. moduleSource is retained in order. SDK/inheritedSdk/custom variants block.
- Library dependencies require name/scope; only compile/default-false isExported converts. Scoped/exported library references block; they are never downgraded.
- Libraries require name, **type even when null**, and roots. Only null type, default project level and absent/null module/properties are supported. excludedRoots absent/empty. Roots require path; type defaults CLASSES; CLASSES/SOURCES supported. inclusionOptions absent/root_itself; directory archive traversal blocks.

Meaningful unsupported fields return `ImportError::Unsupported` with JSON pointer/reason records and **no partial model**. Unknown fields block even when null, including nested fields in supported objects. Unsupported rich subtrees (SDK/compiler settings/facets) are reported as entire fields; this importer does not validate or translate their internal schema. Ill-typed decoded fields return Schema; fields restricted to a neutral value return Unsupported for any other value; malformed JSON/duplicate keys return Json. If both malformed supported fields and unsupported fields exist, the schema error may take precedence.

Native v1 validation rejects invalid/duplicate IDs, unresolved references, duplicate module/library edges, cycles, source ownership conflicts and incompatible settings/collection budgets. Edges are never coalesced or flattened to pass validation. Missing archives are retained without claiming resolution.

## Paths and resource limits

Paths support `<WORKSPACE>/` substitution and absolute host-native paths, preserving Unicode and normalizing `.`/`..` lexically. No URI decoding, backslash paths, other macros, environment lookup, relative-path guessing, filesystem existence/symlink check, archive scan, SDK synthesis, download or build execution. Parent-relative macro paths are permitted. Caller must supply an absolute UTF-8 non-NUL project directory. The official importer additionally repatriates existing ancestors using real/symlink paths; this library makes **no official path-parity claim** for those cases. Cross-platform/Windows path compatibility needs a separate child.

Before Value construction, a custom serde visitor limits 2 MiB input bytes, 100,000 JSON values (keys are bounded by bytes), and depth 32 including root; duplicate keys are rejected at every depth. The caller project-directory length and aggregate expanded path bytes are each capped at 2 MiB; expanded lengths are charged conservatively before joining/normalizing paths. Reports stop at 128 and set `truncated` if additional fields exist; no model is returned. Conversion also applies native v1 budgets, including its 2 MiB serialized projection limit; expanding long absolute paths can hit that limit. All graph validation is iterative through ktlsp-project.

## Additive v2 import (T021c / #171)

`ktlsp_import::v2::from_json(absolute_project_directory, bytes)` returns a
separate `v2::ImportedWorkspace` with an ordered sidecar and
`project_projection() -> &ktlsp_project::v2::ProjectModel`. Existing `from_json`,
v1 public types/errors and compile/non-exported restrictions stay unchanged.
There is no automatic fallback, migration or v2-to-v1 conversion.

Only this new API accepts all eight library scope/export tuples: lowercase
compile/test/runtime/provided, each with absent/default-false or true
`isExported`. Name and scope remain required. Library `isTestJar` stays unknown;
module `isTestJar: true` remains unsupported. All other original subset rules
above still apply, and unsupported semantics reject the whole call atomically.

The separate v2 sidecar dependency enum contains `Module(ModuleDependency)`,
`Library { library, scope, exported }`, `Sdk { name, kind }`, `InheritedSdk` and
`ModuleSource`. It preserves every entry, including repeated moduleSource
positions. The v2 projection omits only moduleSource and retains all other edge order.
Content boundaries, source membership/exact root types and interleaved library-root order
remain in the sidecar. The projection alone is incomplete import semantics.

Delivered project v2 validates tuple identity `(tag, target, scope, exported)`:
identical tuples reject; different scopes/exports survive separately; module and
library names occupy separate namespaces. Targets, self-edges, cycles across all
scopes/exports, IDs and root ownership use v2 validation without coalescing or
flattening through v1. New `v2::Error::{Import(ImportError), Project(v2::Error)}`
wraps the respective importer or project errors and exposes their error source.

The stricter importer byte/node/depth/caller/path/report limits above remain.
V2 additionally caps its intermediate compact native manifest at 2 MiB, counting
actual escaped JSON bytes **before extending** the serialized buffer; excess is
`Import(BudgetExceeded("projection bytes"))`. The intermediate manifest is private
implementation data; its key layout and default omissions are not a public contract.
The delivered v2 loader then applies its collection, 8 MiB prejoin and separate
2 MiB normalized-output limits. An exact intermediate limit can therefore pass
serialization and still fail normalized output. Early Schema/path errors can
override accumulated Unsupported reports; collected Unsupported precedes
serialization and project validation. V1 retains its serialization/error policy.

Decoded input/string bytes and JSON-node counts, aggregate expanded paths across
all content/source/library occurrences, fixed sidecar/projection copies and the
new serialized-length cap provide finite logical storage bounds. These are
separate limits, not an aggregate allocator-capacity, RSS or OOM guarantee.
Public-field mutation is outside loader-returned bounds. Valid maximum edge
counts may be unreachable under the simultaneous byte/node limits.

The independently authored [input fixture](tests/fixtures/scoped-exported.json)
freezes 12 sidecar entries and 10 projected edges, all eight library tuples,
same-name module/library identities, Unicode paths and ordered roots. Tests
cover compatibility, tuple/graph/reference errors, malformed/unknown fields,
atomic mixed blockers, truncation and byte/node/depth/path/projection boundaries.
Verification evidence is in [the review note](reviews/t021-import-v2.md).
No official runtime/oracle/build tool was run. Resources/exclusions/facets/
plugins/compiler/build metadata, path/symlink/platform parity, server wiring and
semantic visibility remain pending. This finite child does not complete T021/#21.

## SDK data import (#342 / #330 / #21)

Only importer-v2 accepts the pinned `sdks` catalog and explicit
`{"type":"sdk","name":"JDK","kind":"JavaSDK"}` / `{"type":"inheritedSdk"}`
dependencies. The catalog uses the single canonical
[`ktlsp_project::v2::Sdk`](../ktlsp-project/V2.md) representation exposed by
`ImportedWorkspace::sdks()` as a borrowed view of the native projection:
there is no second mirrored catalog. Catalog/edge/root order, two SDKs sharing a
name with different kinds, required nullable version/homePath, roots null versus
empty, and required opaque additionalData survive the complete result and native
serialization/reload. Omitted roots become null, per the pinned default.

SDK names/types and root types are nonempty; SDK strings reject NUL. Unknown
SDK/root/edge fields refuse atomically, as do malformed nullable/string fields,
unresolved `(name,kind)` references, duplicate identities/SDK edges, and inherited
markers repeated within one module. Module, library and SDK namespaces are
independent. No SDK is flattened into an ordinary library edge.
Catalog count (8,192), total SDK roots (32,768), decoded SDK bytes (2 MiB), existing
input node/depth/byte limits, projection and native output limits apply together.
The existing path expansion budget still covers ordinary content/source/library
paths; SDK homes are retained spelling, not base-expanded paths.

Homes admit only null, absolute host-native strings or `<WORKSPACE>/` spellings;
unsupported foreign paths/placeholders/URL homes refuse. SDK root URLs are
validated URL envelopes, retained as URL strings, never host PathBuf values.
No home expansion/normalization, root derivation, file/archive lookup, legacy
jrt URL rewrite, first-SDK selection, effective/default/inherited SDK resolution,
additionalData XML parsing, JDK execution or SDK-symbol consumer is implemented.
Explicit and inherited edges can coexist; neither overrides the other here.
Null/omitted roots mean unresolved roots, **not** a verified empty classpath.
The pinned conversion performs filesystem/JDK-based root derivation and path
handling that this native data-only contract intentionally does not claim.

## Source-owner dependency class selection (#381 / #330)

`dependency_class::select_class(&v2_workspace, source_path, internal_name,
java_release, bindings, scratch)` is a finite, data-only native query. It returns
the owning module and production source root even for successful absence; a
selected actual class retains its library, mixed projected dependency ordinal,
export bit, CLASSES-root ordinal/import path, retained artifact (digest and
provenance), and existing classpath origin/entry/rank/decoded identity.

Ownership and ordering come only from the private validated immutable v2
projection; complete ordered importer metadata is retained immutably alongside it. Paths must be absolute UTF-8,
lexically normalized host paths under declared source roots, not content-root
boundaries. Native Windows separators are accepted in query/provenance PathBufs;
foreign backslashes on Unix are refused. Imported JSON still uses the importer's
forward-slash/`<WORKSPACE>/` contract; that spelling is not a restriction on
normalized native output paths. This is **not symlink-safe physical ownership**. Only production
owners with direct COMPILE-library edges are admitted; test owners, module
edges, other scopes, explicit/inherited SDK edges and SDK archives refuse.
ModuleSource placement is irrelevant only because this selects a **dependency
class**, not a source declaration competitor. Export bits are retained data,
not an implementation of transitive propagation.

Caller-retained immutable `VerifiedArtifact` bindings are keyed by library ID
and binary-root ordinal. Every owner root must have exactly one binding and its
retained provenance path must agree with the imported path before lookup or
successful absence, including roots after an early match. Out-of-range bindings
for owner-selected libraries refuse, including libraries with no binary roots.
Unrelated libraries' bindings never grant visibility. Jar/Jmod kind is explicit,
never guessed from filenames;
all indexes are built through the existing archive decoders before lookup.
Digest equality verifies bytes against the caller digest, **not** coordinate,
path or digest authenticity. No filesystem reads, downloads or invented Maven
provenance occur in the query.

Ordered dependency/root precedence and MR/JMOD/name/version/CRC failure rules
remain those of `ktlsp_binary::class_path::lookup_class`; a malformed winner is
never skipped. Limits are 64 root occurrences, 65,536 aggregate directory
entries and 16 MiB scanned directory names, plus existing index/artifact/lookup
limits. Caller binding scans cap at 65,536 records and 16 MiB library-key bytes
(including unrelated records). Source/provenance paths cap at the importer
2 MiB path budget. Archive bytes stay borrowed; indexes are temporary metadata,
not owning self-references or cached filesystem state.

Tests compose authored imported JSON with retained genuine Commons Lang 3.17.0
binary/source archives and canonical coordinates/digests: the actual
`AppendableJoiner$Builder` class and the selected library's
`AppendableJoiner.java` source candidate. Authored archive cases cover explicit
JMOD kind, ordering, completeness, refusal and resource boundaries. Source
attachment remains a path candidate, **not** proof of exact compilation or line
provenance. Release is explicit, never inferred from SDK version or jvmTarget;
the class reader supports ordinary versions through Java 25, not preview. No full effective
classpath, Kotlin reference resolution, source shadowing, access checking,
type inference, SDK resolution or server/LSP capability is claimed.

## Next child

Resolve SDK inheritance/path policy, resource/excluded roots, facets/plugins,
compiler/language/output settings, special library metadata and build provenance
in separately bounded contracts with original-schema fixtures and comparisons.
This subset does not implement full T021, Gradle/Maven/JPS import, dependency
resolution or compiler semantics.

## Original JPS declared-source projection (#396 / #330)

`jps::source_projection(absolute_project_directory, &[jps::Document { path, bytes }])`
consumes a caller snapshot containing `.idea/modules.xml` and exactly its
referenced `.iml` documents. There is no I/O, discovery, physical existence or
symlink check. Snapshot keys are project-relative or absolute host paths;
normalized duplicates, missing/unreferenced documents, duplicate references and
ambiguous filename-without-`.iml` module identities refuse atomically.
Manifest order, not snapshot order, selects modules; content/source declaration
order selects roots. Every module retains its full original `.iml` element and
manifest reference ranges, and every root retains its original sourceFolder
element range and normalized original document path (including BOM offsets).

`ImportedSources::declared_source_projection()` exposes the existing validated
`ktlsp_project::ProjectModel`. Existing `serde_json::to_vec` and
`ktlsp_project::from_json` provide its immediate serialization/reload consumer.
This explicitly named result is **declared roots only**, not effective workspace
membership, a complete JPS model, or an empty effective classpath. It is not
`v2::ImportedWorkspace`. Java language/output settings and recognized leaf
source/module/library/JDK/inheritedJdk order entries are outside this operation.
Inline module-library schemas, other components/facets, resources/custom types,
nonempty package prefixes, exclusions/patterns and unknown elements/attributes
refuse. `<exclude-output/>` is recognized, but any declared source intersecting
an explicitly declared excluded compiler output refuses; inherited output paths
are never invented. Nonintersecting outputs do not establish effective membership.

Only JAVA_MODULE/version4 and ProjectModuleManager/version4 schema is supported.
Missing source type selects test only for `isTestSource="true"`, otherwise
production. Explicit `java-source`/`java-test` takes precedence over that flag;
`generated` is orthogonal/default-false. Present booleans require `true`/`false`.
No source defaults are invented. Roots must remain under their declared content
path. Native model validation rejects duplicate/conflicting root ownership,
including kind/generated ancestry conflicts; same-kind nested roots are retained.

Paths expand only leading `$PROJECT_DIR$`/`$MODULE_DIR$` at a path boundary.
Macro suffixes normalize lexically, including `..`; filesystem-root escapes,
recursive/unknown macros, foreign drives/UNC/device paths and archive URLs refuse.
XML URL attributes require `file://`; their bounded UTF-8 percent decoding is
distinct from literal filepath attributes (`%20` is literal in filepath).
Encoded separators/NUL/macros and URL queries/fragments refuse. Both manifest
spellings, when supplied, must normalize to the same path. No global defaults,
environment variables, filesystem probes or foreign-platform path guesses occur.

XML is strict UTF-8/XML1.0, optionally BOM/declaration, with bounded predefined
and numeric attribute references and XML whitespace normalization. Namespaces,
DTD/external entities, XInclude, CDATA/processing instructions, non-whitespace
text, duplicate/adjacent attributes, illegal scalars and unknown XML refuse.
Limits compose across the entire call: 128 documents, 64 KiB/document, 1 MiB
input and decoded-attribute bytes, 8,192 XML events, depth32, 16 attributes/tag,
4 KiB pre-normalization path and 1 MiB aggregate paths (including provenance),
100,000 counted structural work and 8 MiB aggregate allocated-storage allowance.
Fallible geometric buffer reservations charge actual retained capacities, even
for temporary documents; quick-xml scratch and existing model-loader allocations
are conservatively reserved before delegation. The model envelope is eight times
serialized-plus-manifest-path bytes, plus2048 bytes per module/operation and512
per root; this is an operation allocation allowance, not an allocator hook/RSS
guarantee. Serialized projection remains
bounded by the existing 2 MiB native limit; no per-document allowance resets.

Original unmodified release-pinned JpsJavaModule XML and Apache2 license/notices
are in `tests/fixtures/jps`. Authored tests cover original provenance/roundtrip,
order/generated/type defaults, Unicode/escapes/BOM, late atomic refusals,
intersections, paths/XML security and boundaries. No test, format, compile or
runtime observation is asserted by these source fixtures. Supplementary IntelliJ
`188b7ef96f72c032b7ee232ba6f5ec3dac9448f8` research is not proven release-matched.
Full JPS dependencies/SDK/compiler/resource/exclusion/facet/build integration and
official runtime parity remain required work.

## Bounded read-only source inventory (#421)

**Native data runtime and the capture-bearing composed consumer are verified within
the [recorded scope](../../docs/status/development.md#pr422-original-hosted-actual-gates-and-capture-only-retirement);
capture-free integration still requires its own reviewed full CI.** This separate API
performs explicit native I/O; JSON-v1/v2, JPS, Maven and T004 producers keep their
existing data-only contracts. No Java, build tool, compiler or C/C++ analyzer is
required. Windows uses target-only pinned `file-id = 0.2.3` Rust OS bindings for
full native file identity, not a semantic engine.

`source_inventory::load(project_directory, &mut Budget, &AtomicBool)` returns a
complete immutable `Inventory` or one error, never a usable prefix. Borrow
`project_directory()`, the ENTIRE original v2 `imported()` model,
`workspace_json()` exact bytes and `entries()`. Each entry retains its native
path, `EntryKind`, original Kotlin `Arc<str>` when applicable, and ordered
module/content-root/source-root membership indices into that same model.
Nested same-module roots retain every matching association; Java, scripts,
other files and directories remain classifications, not ordinary Kotlin parses.
Hidden/build/.git entries are not silently excluded.

Use ONE monotonic `Budget` for ALL configured projects, roots, entries and
consumer URI reservations. `Limits` may reduce, never raise, these ceilings:

| Resource | Whole candidate ceiling |
|---|---:|
| Projects | 16 |
| Original model bytes | 2 MiB |
| Visited entries, including nonsource files | 16,384 |
| Directories | 4,096 |
| Project-relative depth | 32 |
| Kotlin sources | 1,024 |
| Retained source bytes | 16 MiB |
| Individual source bytes | 64 KiB |
| Retained native/model/consumer URI path bytes | 2 MiB |
| Individual native/model path bytes | 4,096 |
| Original membership associations | 16,384 |
| Counted logical work | 256 Mi units |

Charges precede bounded reads/storage/delegation; failures do not refund work.
Model path expansion is admitted before unchanged v2 allocation. URI consumers
use `reserve_path_bytes` on that SAME budget. These are logical resource bounds,
not allocator/RSS/OOM or OS-I/O-latency guarantees.

The filesystem assumption is trusted, quiescent and LOCAL, as for existing
artifact readers: not a hostile replacement/mount/hardlink sandbox. Every
ancestor/leaf is checked; symlinks, junctions/reparse points, special entries,
external selected roots, physical aliases and contradictory ownership/kinds
refuse the whole candidate. Unix identities retain full device/inode pairs;
Windows requires full high-resolution native IDs without low-resolution/hash
fallback. IDs can be reused; no hostile-race guarantee is implied. No per-entry
descriptor remains open.

`Inventory::verify_source(index, &mut Budget, &AtomicBool)` accepts only an
original recorded Kotlin source. It rechecks original model AND selected source
bytes, full identities, ancestors, size and EOF, using bounded streaming reads.
Unknown/nonsource indices cause no filesystem reads. Model comparison charges
work, not retained-model storage again. The Session calls it before parsing and
before publishing a successful render with the SAME request budget.

Source and model changes require explicit reload; no automatic watcher,
discovery outside configured roots, source mutation, archive/SDK traversal,
environment lookup or tool invocation is added. Existing meaningful unsupported
v2 fields still refuse without rewriting input. Complete actual native data,
hosted CLI/overlay/reload proof and guarded integration remain separate gates.

