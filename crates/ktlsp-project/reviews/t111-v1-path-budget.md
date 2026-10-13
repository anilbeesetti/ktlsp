# V1 caller and expanded-path budgets (#111)

The coordinator accepted the [reviewed contract](https://github.com/anilbeesetti/ktlsp/issues/111#issuecomment-6135644743)
and granted implementation/publication from frozen base
`61142f66f047a6ac45af5032f4237b30ce6134f6`. This decision explicitly accepts:

- 8 MiB raw/cwd-composed caller encoded-path bytes and one shared 8 MiB
  conservative prejoin path budget across source roots, binaries, sources and
  SDK archives. Formerly accepted oversized paths now return resource errors,
  including paths whose later normalization would shrink them.
- Additive `Budget::CallerPathBytes` and `Budget::ExpandedPathBytes` variants;
  downstream exhaustive `Budget` matches may need updating.
- Existing v1 non-Unicode caller support and normalized serialization/reload
  behavior. No normalized-output cap is added.
- One caller `AsRef` conversion, guard-before-I/O/caller-error copies, full
  aggregate admission before resolution/ownership-error copies, checked
  overflow, absolute-path base exclusion and the contract's error precedence.
- A narrowly scoped shared `resolve` absolute-path fast path, subject to both
  reviewers checking v1/v2 results and errors. No v2 policy/source changes.

The coordinator additionally accepted one retained-prefix exception after both
reviewers and architect inspected Rust's Cygwin join semantics: when the base
starts with `Component::Prefix` and the model path starts with `RootDir`, joining
can retain the caller prefix even if that platform calls the model path absolute.
One allocation-free predicate must govern v1 charging and shared resolution:
absolute without retained prefix uses own bytes/direct normalization; every
other case charges the full base plus separator plus path and keeps the original
join. This conservatively covers the temporary base copy and may reject
formerly accepted oversized retained-prefix expansions. V2 admission policy
remains unchanged; this exception is not a both-version hardening claim.

Exclusive implementation files: necessary v1 `src/lib.rs` helpers,
`tests/v1_path_budget.rs`, this contract/review record, plus the minimal current
v1-unguarded statement correction in `V2.md` and a README discoverability link.
No importer, server, artifact, dependency, Cargo/lock or shared status edits.

## Resource and compatibility contract

Encoded byte lengths use `OsStr::as_encoded_bytes`, preserving v1 native paths.
The raw caller is checked before normalization or a lossy diagnostic copy. For
a relative manifest, cwd bytes plus one separator plus raw caller bytes are
checked before joining. Overflow rejects. `load_manifest` checks the same
borrowed caller path before opening it, then passes that path to `from_json`.
The OS allocation returning cwd is outside the guard's control.

After normalizing the manifest and finding its parent, a complete borrowed-path
pass charges each absolute model path that discards the base by its own encoded
bytes. Other paths, including the retained-prefix exception above, cost base
bytes plus one separator plus their encoded bytes. All four path
collections share one accumulator. Exact limits pass resource admission;
existing schema/graph/path/ownership rules still apply. Parent-relative paths
and separator reuse are intentionally charged conservatively. No joined model
path or ownership-error path clone is created before the pass succeeds.

`from_json` preserves input-byte, streaming JSON/count, typed schema and version
error precedence before caller checks. Caller length precedes caller path
validation. Aggregate overflow precedes model graph/settings/path/ownership
validation. `load_manifest` returns caller-budget errors before I/O for oversized
raw callers; within-budget I/O behavior is retained. Budget errors do not copy
the rejected caller. Empty/NUL/nonabsolute path errors remain for admitted paths.

This is a logical expanded-path bound, not an RSS or total-allocation limit.
Joined/normalized temporaries, capacities, lossy errors and ownership clones add
bounded overhead/multiples. Public mutation, cloning and external serialization
are outside loader admission. Missing artifacts and symlink aliases stay data;
no filesystem discovery, SDK semantics or language-provider parity is implied.

## Source proof and caller scope

On Unix, a 1,048,576-byte absolute base without normalization shrink, a caller
`base/m`, and 32,768 SDK paths `"a"` need only 131,120 compact input bytes.
Current count/input rules admit that structure. Expanded path content would be
`32768 * (1048576 + 2) = 34,359,803,904` bytes (32 GiB + 65,536 bytes), excluding
capacities and temporaries. This is finite arithmetic/source proof; that model
was never constructed or executed.

V1 `load_manifest` and `from_json` funnel roots, library binaries/sources and
SDK archives through `validate_model`/`resolve`. The only external production
loader caller is `ktlsp-import`'s v1 projection handoff. Its projected paths are
already absolute and its caller/expanded input paths are separately bounded at
2 MiB. Importer sidecar/Value/projection/serialization allocations occur before
this loader and retain their existing limits. There is no server consumer.
V2 shares normalization/path/model validation helpers; v1 admission stays at v1
entry points so it cannot impose new v2 policy.

The shared resolver normalizes ordinary absolute paths directly. It retains the
old join route for a prefixed base with an absolute root-only input, preserving
Cygwin's special prefix-retaining join behavior. Windows rooted-only and
drive-relative inputs also keep the original nonabsolute join route. Source
equivalence review is separate from platform runtime evidence.

Unimplemented follow-up proposal for coordinator: a separate narrow v2 child
should align `v2::validate_path_budget` with the retained-prefix predicate before
any expansion. V2 currently charges such Cygwin absolute paths only their own
bytes; its normalized-output check occurs after resolution, so it cannot prevent
this intermediate amplification. Scope would be v2 accounting, focused
conditional prefix tests and its contract evidence; preserve the existing v2
8 MiB/output policies and shared resolver results. No v2 fix or new issue is
included in #111. Linux CI cannot establish Cygwin runtime correctness.

## Serialization remains independent

For base bytes 128, the same SDK count has 4,259,840 expanded path bytes: below
8 MiB, but compact serialization exceeds the existing 2 MiB reload-input limit.
V1 still accepts such models and subsequent reload returns `ManifestBytes`.
Defaults, escaping and nonpath fields also affect serialized size. Non-Unicode
resolved paths can still fail serialization. V2's separate normalized-output
counter is not added to v1. Ordinary admitted compact roundtrips remain covered.

## Review and validation

Contract functionality/adversarial and architect reviews approved after
rechecking single-evaluation `AsRef` and documentation ownership. Final source
reviews and Linux validation are recorded below when complete. No local native/parser/runtime,
performance, huge allocation, JVM/Gradle/Maven or oracle execution is permitted.
Optional local format/typecheck/clippy compilation is authorized. Existing
Linux Native Rust CI must execute the unchanged pinned commands on the reviewed
head and exact merged main:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Issue #96 remains open with unknown cause; Linux success is not macOS reliability
evidence. Windows runtime remains unverified unless separately recorded.
Parent #21 remains open. Publication does not grant merge: HOLD for the
coordinator's stable final integration turn and expected-head merge window.

Focused tests cover exact/+1 caller and each path collection, one mixed four-way
budget, full-pass precedence over ID/ownership errors, absolute paths under an
8 MiB caller, mixed absolute/relative accounting, raw/cwd/model normalization
shrink, root/trailing separator/Unicode-control callers, decoded Unicode/escaped
JSON paths and keys, non-Unicode Unix callers/lossy errors, single-evaluation
stateful AsRef, arithmetic overflow and unchanged oversized serialization/reload.
V1/v2 shared-resolution comparison and Windows-only rooted/prefixed/verbatim/
drive-relative fixtures are included. Tests use at most budget-sized expanded
models; the 32 GiB source-proof example is not executed.

Final source rechecks approved controlled Linux publication:

- `/root/contract_functionality` verified all callers/budgets/error precedence,
  stateful caller identity, unchanged serialization and shared v1/v2 resolver
  results/errors. No remaining source findings.
- `/root/contract_adversarial` verified ordering/overflow/path cases and no
  parser worker/dependency graph changes. Its retained-prefix accounting and
  conditional UNC-spelling findings were fixed and rechecked.
- `/root/architect_contract_plan` approved finished scope/helpers/contracts;
  its conditional UNC root-separator sizing finding was fixed and rechecked.

The conditional prefix fixture now uses backslash UNC with a normal directory
after the share. The predicate tests the model path before scanning the base
prefix. All reviewers approved these amendments. Local
`cargo fmt --all -- --check` and
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed with
no native test execution. Runtime approval remains pending completed exact-head
Linux logs from both independent reviewers. Hosted checkout/tree, check URLs,
head/review/merge evidence will be recorded in the PR and coordinator report;
this source record does not claim a future run passed.
