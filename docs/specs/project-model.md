# Native project model v1

T004 implements this explicit model, not dynamic Gradle/Maven/JPS import. File name is user-selected; default discovery later uses `ktlsp.json`. JSON uses camelCase and rejects unknown fields to avoid silently ignoring project semantics. Schema version must equal 1.

```json
{
  "schemaVersion": 1,
  "modules": [{
    "id": "app",
    "sourceRoots": [{"path": "src/main/kotlin", "kind": "production", "generated": false}],
    "dependencies": [{"module": "core", "scope": "compile", "exported": false}],
    "libraries": ["stdlib"],
    "languageSettings": {"languageVersion": "2.3", "apiVersion": "2.3", "jvmTarget": "21"}
  }, {"id": "core", "sourceRoots": [{"path": "core/src", "kind": "production"}]}],
  "libraries": [{"id": "stdlib", "binaries": ["lib/kotlin-stdlib.jar"], "sources": []}],
  "sdkArchives": []
}
```

Required: schemaVersion, modules, every module id, every source-root path/kind, every dependency module/scope, every library id/binaries. All other collections default empty; generated/exported default false; languageSettings defaults unset optional string fields. `kind` is `production` or `test`; generated is orthogonal. `scope` is `compile`, `test`, `runtime`, or `provided`. Language/API versions accept syntactically numeric major.minor strings, and jvmTarget accepts positive numeric strings or `1.8`; no claim these versions are semantically supported yet. Reject apiVersion greater than languageVersion when both set.

IDs must be nonempty after trim and unique in their collection. Module and library references must resolve. Reject self-dependencies, duplicate edges, and directed module cycles using an iterative bounded graph algorithm. Module/library namespaces are separate. Module libraries are compile/non-exported references in v1; test/runtime/provided/exported library dependency variants are explicitly unsupported until the schema is extended. Source roots must not equal or contain another module's source root; overlaps within one module must be rejected if their kind/generated semantics differ (equal duplicate roots always rejected).

Resolve relative paths lexically against the manifest parent; return absolute normalized PathBuf values. Eliminate `.`/`..` without requiring paths to exist. Parent-relative roots are permitted because real multi-project models can live outside the build root. This loader performs no classpath traversal, process execution, artifact downloads or symlink canonicalization. Missing files are retained for later explicit import-status reporting; they are not assumed resolved. Ownership validation is lexical only; symlink aliases require a separate filesystem-discovery check before analysis and must be documented as that limitation.

Budgets: 2 MiB manifest bytes, 4,096 modules, 8,192 libraries, 65,536 total dependency edges, 16,384 total source roots, 32,768 total binary/source/SDK archive paths. Check before unbounded allocation/validation; reject oversized input with a named ProjectError. No recursively traversed graph. Path strings cannot be empty or contain NUL. Malformed JSON/schema/path/graph errors return Result, never panic. Public ProjectModel retains source/test/generated kinds, scopes, exports and settings; models cannot be flattened to a list of files without losing semantics.

## Separate physical source authority (#421)

T004 and existing model/import producers remain data-only and retain their
lexical path/projection contracts above. A separately explicit read-only native
`ktlsp_import::source_inventory` layer admits actual immutable v2 models, original
model/source bytes, complete selected-root inventories, original memberships
and full physical identities under one aggregate budget. Its local/quiescent
filesystem assumption is NOT a hostile-replacement sandbox; it refuses
symlink/reparse/special/external/alias/conflicting roots rather than guessing.
Capture-bearing Session composition passed original full three-host runtime and
independent actual-value/edit gates. Capture-free source then passed its OWN
reviews/full three-host CI, guarded PR422 integration and both original
exact-main gates. [Exact evidence and limits](../status/development.md#verified422-capture-free-integration-and-original-exact-main-acceptance)
do not turn declared SDK/library/build metadata into effective semantics.

