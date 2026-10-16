# Optional Windows project path-budget validation (#21 child)

Status: the first optional dispatch POST was rejected without creating a
`workflow_dispatch` run or job.
Windows/Cygwin runtime correctness remains UNVERIFIED; no Windows install,
compile, inventory listing, test, job or artifact exists. Parent #21 and
packaging T046 remain open.

## First optional dispatch: rejected without a dispatch run or job

On 2026-10-14, the first dispatch of workflow `384102190`, ref `main`, with
`expected_workflow_sha=015057d40ea8739502e179b246b9385be016b5b2` and
`expected_workflow_sha256=dcd716340f22660fd2fa18fa9a7c0bc178107e38f3f03d7c4623302f93642036`
was rejected with HTTP 422. GitHub identified `runner.temp` in job-level env
at lines 24 and 25 as an unavailable named context. The initial response is
396 bytes, SHA-256 `f3894680583f17bea21ff9142e1ef7001f15536d5219286a2e2874c2e6f6232b`.
Read-only resolution confirmed that the rejected POST created no
`workflow_dispatch` run or job, compiler/test output or artifact. There is no
run attempt for that POST. This is dispatch validation failure,
not a Windows compiler/test outcome. The first failure is retained permanently;
no retry, resend, rerun or source correction was performed under that admission.

Separate original GitHub startup records are retained: main
`015057d40ea8739502e179b246b9385be016b5b2`, run `38562813506`; main
`e3a79570895d8cd0af4681424eebca819e725361`, run `38567332094`; and constructor
`60af5d3c5f0f55613be080ca222807ed2fbbf222`, run `38569059184`. Each is attempt 1,
event `push`, completed FAILURE with zero jobs. Constructor check suite
`104509956493` has zero check runs, so named native SUCCESS alone misses that
failure. Exact startup validator diagnostics are absent; attributing these
failures to the same unavailable job-env `runner.temp` context is an inference
from their identical original workflow hash and the explicit earlier HTTP 422.
Earlier native 587-pass proofs remain valid for native CI only; they do not
establish that all workflows passed. These startup failures provide no Windows
runtime or compiler/test outcome and do not alter the rejected POST's zero
`workflow_dispatch` runs or consumed admission.

[GitHub's context-availability table](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts#context-availability)
excludes `runner` at job env. The correction removes only those two
job env fields. After the unchanged first identity guard succeeds, the existing
`pwsh` step appends the same Cargo home/target paths derived from `RUNNER_TEMP`
to `GITHUB_ENV`. [GitHub's environment-file documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-environment-variable)
supports UTF-8 `pwsh` appends and makes their values available to later steps,
not to the writing step. That step does not use either new value or create
Cargo directories; the later freshness check remains unchanged.

This source-only correction supplies no new dispatch or runtime outcome. The
original W/H tuple is consumed. Source delivery retains separate root scope,
reviewed source/original Linux CI and ordinary guarded merge; any future first
dispatch needs a new immutable W2/H2 admission. Pending fields below refer to
that future corrected workflow, never an unchanged-tuple retry.
Frozen product source, all 23 pins, 14+3 selected tests, compile-only targets,
assertions, commands, identity guards, timeout and resource limitations remain
unchanged. All other expression placements received a source-only context audit;
that audit does not claim GitHub dispatch acceptance or Windows runtime success.

Frozen source `2d561ab30812dcdabc56260a8b510979e1e70a61`, tree
`be29ab7d30d31f6b071b57e4d48320521e190919`, sole parent
`7980480e015150b75b982001c871ad1a5b54f35a`.

A separately authorized manual Windows job will use Rust
`1.99.0-x86_64-pc-windows-msvc`, explicit target
`x86_64-pc-windows-msvc`, a fresh Cargo home/target, and `--locked`. Compile
`ktlsp-project --tests --no-run` (lib plus four integration targets), then list
and execute only `v1_path_budget` and `v2_prejoin_budget`. No source, assertion,
expected path, cap, fixture, dependency, build script or original Linux workflow
changes. No local native execution. Zero/missing/unexpected/ignored/filtered
tests cannot pass. Preserve first compile/list/test failure, no automatic retry,
rerun-to-green or pre-failure source correction.

## Frozen inputs (SHA-256 of canonical tracked blob bytes)

| Path | SHA-256 |
| --- | --- |
| `.github/workflows/native.yml` | `87c83b01c28c217c385fac416a8d60f7b7f7dbf8ec037d20040fbd4ea44686fe` |
| `Cargo.lock` | `8e34c8c44333b5dc1b469dd8c199199c6a9e864ab2976f398f35999d0c4f9c91` |
| `Cargo.toml` | `b6c8a2f95f1ba437c665f50bf34a423ddacf88d8099880ce636ae773b3e769d7` |
| `crates/ktlsp-artifacts/Cargo.toml` | `6986c43ca73ecb1b4421dd95cf05151b9000970a8a25d786c4f389f71f79e791` |
| `crates/ktlsp-binary/Cargo.toml` | `76e788c0eeba910ad6b43b3676274da6dcf51c2fc7af7ed0fdb7dcf5a6553c7e` |
| `crates/ktlsp-import/Cargo.toml` | `64655b866ddf78ebccf59879dc3a11641b5707e32d0a4cf58f6b10b2f35a6050` |
| `crates/ktlsp-parser/Cargo.toml` | `6a748901adda60de75f62458689d6523d386e77ad14a99799516c1b93361b961` |
| `crates/ktlsp-project/Cargo.toml` | `73208bae8fff1cbbe1c2f1133e87f7dc02e2af9c9c546fc691f921419bb5e5b8` |
| `crates/ktlsp-project/fixtures/t021b/contract-example.json` | `7a35d881c2f2b17a3c2d3f2639d3d40ada817f7846396782abbcab42783a70a6` |
| `crates/ktlsp-project/fixtures/t021b/ordered.json` | `dae4071c328f8c57feaef33734c3b0c7ef48a72ec0217a7886265180857e4ba7` |
| `crates/ktlsp-project/src/lib.rs` | `024f9825cf06fdef6db2aa1510b1e35f3233e4729847ac1b4533010fc2479704` |
| `crates/ktlsp-project/src/v2.rs` | `c87b891a91c7809e20fcd255c2876b3b4c1c98c0535b280e35bc511ac5c770d5` |
| `crates/ktlsp-project/tests/fixtures/multimodule.json` | `cc69f9f38c64e2d831b0654ff35d353f92658742e7d60c2f320566c5dc0adb31` |
| `crates/ktlsp-project/tests/model.rs` | `66eeae9849ecd527f98d101c82ed830170bf624e4dccd8fe7c51c87f968240a7` |
| `crates/ktlsp-project/tests/v1_path_budget.rs` | `65e92d9d8e36d6bb9128c2ccb0f9f4b1b8bcf431fb2bfcc55a47cdaa14ffded5` |
| `crates/ktlsp-project/tests/v2_library_edges.rs` | `2c983d058d89d6abaa23bbcf9496403e42e92b457250b23fa338ba2a557645de` |
| `crates/ktlsp-project/tests/v2_prejoin_budget.rs` | `641905a02a11f89b0b40f6c4411e4359027f1e4f33a8bb4fb0c23bf180eecad3` |
| `crates/ktlsp-protocol/Cargo.toml` | `a9ca342c3c32519a4eff1fbcff71de018aa55a8083ab9d30b63db64a77bbe79d` |
| `crates/ktlsp-server/Cargo.toml` | `f785e9d1bfdb32acec82f50af7a70420b98b8d61979a8aff5c9f4166e510982a` |
| `crates/ktlsp-syntax/Cargo.toml` | `08c6bebf8f2b3604fed6899f0ca62e58197a2daf134068cd0544a14112ebfde1` |
| `crates/ktlsp-syntax/evidence/85/probe/Cargo.toml` | `74a740477f48fb0abe1207c2c7e9ef0a2c6b9bd12de4a97e97bc5a8b6eab9d9e` |
| `crates/ktlsp-text/Cargo.toml` | `9d58a03a86abf7984d79b5fc9b71058028685526ec82c40b0e88298b6e10b19d` |
| `rust-toolchain.toml` | `2ad6f15416572b4836585087f1d4abc53b8fa2dbd616f8dacfaa587ed6714228` |

No repository `.cargo` or project build.rs exists. Immutable source tree pins all
other tracked bytes. LF materialization on the disposable runner precedes raw
working-file hash checks; source must be clean before/after execution. The
selected pair uses `tests/fixtures/multimodule.json`; two T021b fixtures are
compiled into the compile-only library-edge target.

## Frozen selected test/assertion inventory

Every body and helper is pinned by its containing file SHA-256 above. V1 selects
12 unrestricted tests plus drive/root/UNC/verbatim `cfg(windows)` and retained
prefix `cfg(any(windows,target_os="cygwin"))` cases. Its Unix-only non-Unicode
caller case is excluded on Windows. V2's whole file has the latter cfg. Its
helper asserts `/a` is not absolute on Windows and is absolute on Cygwin; only
the Windows preservation branch is selected here. Assertion lines below refer
to the frozen source, including exact expected paths and budget/error guards.

| Target | Test | cfg | Assertion/guard lines |
| --- | --- | --- | --- |
| `v1_path_budget` | `each_collection_accepts_exact_expansion_and_rejects_one_more_byte` | `all` | 51, 53 |
| `v1_path_budget` | `all_four_collections_share_admission_before_any_model_validation` | `all` | 65, 67, 73, 79, 84 |
| `v1_path_budget` | `absolute_paths_exclude_long_base_and_preserve_v1_v2_results_and_errors` | `all` | 100, 101, 102, 103, 107, 112 |
| `v1_path_budget` | `mixed_absolute_relative_paths_charge_only_relative_base` | `all` | 126, 128 |
| `v1_path_budget` | `caller_exact_and_above_limit_precede_io_and_nul_diagnostic_copies` | `all` | 138, 142, 148, 151, 158, 163, 167, 171 |
| `v1_path_budget` | `native_caller_root_trailing_separator_and_unicode_spellings_are_preserved` | `all` | 181, 187, 192 |
| `v1_path_budget` | `relative_caller_composition_is_checked_before_shrinking_normalization` | `all` | 204, 206, 216 |
| `v1_path_budget` | `model_normalization_shrink_does_not_bypass_conservative_admission` | `all` | 223, 229 |
| `v1_path_budget` | `decoded_unicode_and_escaped_keys_paths_use_bytes_not_wire_spelling` | `all` | 242, 251 |
| `v1_path_budget` | `json_version_and_input_errors_keep_precedence_over_caller_budget` | `all` | 259, 261, 266, 270, 274 |
| `v1_path_budget` | `caller_conversion_is_single_for_guard_open_and_base_resolution` | `all` | 305, 306, 311, 312 |
| `v1_path_budget` | `normalized_serialization_retains_existing_reload_limit_and_small_roundtrip` | `all` | 344, 347, 348, 358 |
| `v1_path_budget` | `windows_absolute_rooted_and_prefixed_paths_preserve_both_model_versions` | `windows` | 388, 389, 392, 397 |
| `v1_path_budget` | `retained_prefix_paths_share_exact_prejoin_budget_and_preserve_resolution` | `any(windows,target_os=cygwin)` | 412, 413, 417, 422, 424, 432, 438 |
| `v2_prejoin_budget` | `each_collection_cygwin_regression_windows_preservation` | `any(windows,target_os=cygwin)` | 49, 68, 75 |
| `v2_prejoin_budget` | `mixed_precedence_cygwin_regression_windows_preservation` | `any(windows,target_os=cygwin)` | 83, 85, 90 |
| `v2_prejoin_budget` | `small_resolution_cygwin_regression_windows_preservation` | `any(windows,target_os=cygwin)` | 98, 102, 106, 110, 113, 114, 120 |

Helper assertion lines: `v1_path_budget` 12, 20; `v2_prejoin_budget` 12, 13, 14, 19, 20, 25, 27, 33, 38, 49; 

Exact compiled listed names must match these 14 v1 and 3 v2 names before any
body executes. Actual per-test pass names and summary counts must match after
execution; no ignored or filtered case. A first v1 failure remains failure while
v2 receives its first independent outcome, without a repeat of v1.

## Selected closure and platform scope

Root has normal serde/serde_json only and no build/dev edges or script. Complete
selected registry closure: serde1.0.229(default/std/derive), serde_core1.0.229
(std/result), serde_derive1.0.229(default empty), serde_json1.0.154(default/std),
itoa1.0.18, memchr2.8.3(std/alloc), zmij1.0.23, proc-macro2 1.0.107(proc-macro),
quote1.0.47(proc-macro), syn3.0.8(clone-impls/derive/parsing/printing/proc-macro),
unicode-ident1.0.27. Locked archives were statically checksum-authenticated as
data during preparation. Transitive dev/disabled optional/cfg(any()) edges are
excluded. Rust-only build.rs in serde/core/json, zmij, proc-macro2 and quote
perform Rust cfg/generation/compiler probing; build-dependency tables empty.
serde_derive is a Rust proc macro. No parser/worker, JVM/JDK or C/C++ analysis
dependency is in this selected graph; ordinary platform linker remains needed.

This proves no Cygwin absolute-root semantics, general Windows server behavior,
packaging, official parity or performance. T008/T008a HOLD, C13/B3/B4 GAP,
B11 UNVERIFIED, #7/#83/#96 OPEN and Darwin UNKNOWN remain unchanged.

## Workflow admission and original outcome (pending)

The Tee-Object logs and uploaded artifact logs have no byte cap. Finite artifact paths and the 20-minute job timeout establish no byte bound. This optional CI validation supplies no product or G2 resource guarantee.

Workflow source is a later independent reviewed commit W and file SHA-256 H.
Future root grant must supply W/tree/H, freeze source above, and reserve its
first dispatch receipt. Dispatch main with expected W/H; mismatch in resolved
workflow SHA/ref/path/bytes aborts before compiler work. Main movement must
return for fresh review/admission, never silently retarget either checkout.
Attempt 1 only; a new unchanged dispatch is also forbidden by the coordinator
receipt while pending/unknown or after first outcome without separately reviewed scope. Unknown dispatch
outcomes are resolved read-only, never blindly resent.

Hosted windows-2025 image is mutable: record ImageOS/ImageVersion, runner OS/arch,
PowerShell, rustc/cargo/target cfg, resolved action/linker setup logs, source and
workflow identities. Existing checkout@v4 with persist-credentials:false and
contents:read; no secrets/context/config dumps or credential artifacts.
Always-upload finite logs/pins/names/workflow copy only, 14-day retention; root
retains original run/job/steps/logs/artifact bytes/digest before expiry. Timeout,
cancellation, setup failure or missing artifact is not a pass. Hosted raw logs
are fallback if runner loss prevents upload. No result or URL is fabricated.

Pending: execution grant; W/tree/H; dispatch timestamp/run ID/attempt; run/job and
artifact URLs/IDs/digest; exact first compile/list/run exits, names and counts;
SAME3 original-outcome reviews and residual limits. No runtime result yet.

The unchanged original Linux fmt, locked full-workspace clippy and locked
full-workspace tests remain required on the future PR reviewed head. SAME3
source plus completed original Linux PR CI review, root exclusive ordinary
expected-head/base merge and owner+root original exact-main evidence remain
separate from optional Windows execution.

Compile-only static Windows test names (no execution claim):

- `lib`: `path_budget_tests::arithmetic_overflow_is_a_named_resource_error`.
- `model`: `fixture_retains_semantics_and_missing_artifacts`, `defaults_and_empty_project`, `malformed_schema_unknown_fields_and_required_fields`, `versions_are_syntactic_numeric_and_api_is_ordered_numerically`, `ids_namespaces_and_references`, `graph_rejects_self_duplicates_and_cycles_including_disconnected_cycles`, `max_length_chain_is_iterative_and_accepts_all_scopes`, `lexical_paths_are_absolute_missing_parent_relative_and_root_bounded`, `ownership_handles_component_boundaries_normalization_and_nested_ancestors`, `bounded_manifest_read_and_io_errors`, `collection_budgets_run_before_typed_allocation_and_are_aggregate`, `arbitrary_truncations_bytes_and_deep_json_return_without_panicking`, `duplicate_library_edges_and_all_boundary_counts`, `load_manifest_reads_fixture_from_its_own_parent`.
- `v2_library_edges`: `ordered_fixture_roundtrips_all_scopes_exports_and_namespaces`, `v1_stays_strict_and_v2_does_not_accept_v1_fields_or_version`, `strict_json_and_tagged_variants_reject_malformed_unknown_and_duplicate_keys`, `ids_references_and_tuple_duplicates_are_validated_in_separate_namespaces`, `whole_graph_handles_parallel_edges_self_and_disconnected_cycles_iteratively`, `paths_ownership_and_settings_keep_shared_semantics`, `exact_collection_preflight_boundaries_precede_typed_schema_allocation`, `valid_large_libraries_roots_and_mixed_edges_remain_bounded`, `caller_base_amplification_is_bounded_before_any_model_path_expansion`, `short_input_with_oversized_normalized_output_rejects_during_load`, `exact_compact_output_boundary_counts_unicode_escaping_and_preserves_roundtrip`, `exact_byte_limit_bounded_file_reads_and_io_errors`, `malformed_truncations_arbitrary_bytes_and_deep_json_do_not_panic`.


## First corrected W2 attempt: frozen-source guard failure

This section supersedes earlier no-job/no-artifact and pending-status wording
for the subsequent W2 attempt. The earlier rejected W1/H1 POST and all three
separate failed push startup records remain permanently retained first outcomes;
none becomes a passing run retroactively.

After owner/root source delivery clear, one separately admitted POST for W2
`9d30fc2d22cc8003f3aca575ad8664114b393487`, tree
`08d7f8a97f5e3ff411d5fb692020ccb21931d60e`, and H2
`e38302eccecba61c46299ac43360378a79bd76e3274416fbae32cbf79eeff9be`
(14,289 bytes) returned HTTP 204. Original
[run 38575294702 / job 115890672910](https://github.com/anilbeesetti/ktlsp/actions/runs/38575294702/job/115890672910),
attempt 1, event `workflow_dispatch`, ref `main`, completed FAILURE.
Metadata and the uploaded identity/workflow bytes confirmed immutable W2/H2
binding; the first identity and workflow-byte checks succeeded. The source
checkout logged exact frozen HEAD `2d561ab30812dcdabc56260a8b510979e1e70a61`
and used depth 2. Git was `2.55.0.windows.5`, PowerShell `7.6.6`, runner image
`win25-vs2026` version `20260925.250.1`.

The first failure was `Frozen source identity or cleanliness mismatch` at the
source-guard script's line 8, exit 1. Runtime source tree, parent, status and
individual native exit values were not emitted; the failing OR predicate remains
UNRESOLVED. The artifact's declared source tree is not runtime verification.
Source-pin verification was not reached. Rust installation, compilation,
inventory listing and all test bodies were skipped. No v1/v2 runtime outcome
exists; all 17 selected Windows tests remain UNVERIFIED.

The original raw log is 57,198 bytes, SHA-256
`a0a7079bc89b74c785250d025e74c4483b21b5c7f8ed34b65d3c4ca4c403333a`.
Artifact `11899427448`, `t021-windows-38575294702-attempt-1`, is 5,175 bytes,
SHA-256 `b5f8ecb7d0711c5f3d7230365f1c35991693ed603211d7c3e48438b6804aa7a0`;
it contains only `identity.json` and `workflow.yml`. Original bytes and the
consumed reservation are preserved. Retained first-outcome reviews:
`/root/architect:T021-W2-first-outcome-r1`,
`T021-WIN-W2-FIRST-OUTCOME-CORRECTNESS-15-20261014`, `/root/adversarial`.
Root independently confirmed the failure and released the short identity freeze.
No unchanged retry, rerun, source tuning or local runtime followed that admission.

## Diagnostic source correction: no demonstrated cause-specific fix

Canonical Git-object data agrees with the frozen HEAD/tree/sole-parent
expectations. No tracked attributes or screened Windows path collisions were
found; six text blobs contain CRLF. Actual runner Git configuration and status
were not recorded, so neither checkout depth nor line-ending behavior identifies
the original failing predicate. That cause remains UNRESOLVED.

The diagnostic correction retains exact HEAD/tree/sole-parent and cleanliness
requirements. It records native outputs and immediate exit codes, the shallow
flag and seven relevant Git configuration keys with origins before checking
conditions. Evidence is emitted to raw logs and `source-before-lf.json`.
Unexpected native errors or output shapes fail closed; config query exit 1 is
accepted only with empty output. A dirty initial source stops before the existing
LF materialization can overwrite unexplained changes. That unchanged command's
result/exit and subsequent ordinary-config status are recorded in
`source-after-lf.json`, then checked for errors and cleanliness.

The new pre-materialization check may stop earlier; its outcome cannot identify
W2's original failing predicate. Step-local native error preference permits exit
capture followed by explicit failure checks; PowerShell cmdlet errors still stop.
There is no configuration write, cause-specific normalization fix, reset,
expected-value relaxation, helper framework or change to the frozen product,
23 pins, depth 2, 14+3 selected tests, compile-only targets, compiler commands,
runner, timeout or artifact limits. This supplies diagnostics and native exit
handling, not Windows test correctness. A future runtime attempt requires new
reviewed immutable workflow bytes and separate root admission; neither W1/H1 nor
W2/H2 may be retried. Mutable runner, uncapped raw logs, finite artifact retention
and all previously recorded platform/resource/semantic limits remain unchanged.

## Native platform #329 continuation: first strict Clippy outcome

The separate native-platform workflow's original
[run 38875113607 / Windows job 116888169239](https://github.com/anilbeesetti/ktlsp/actions/runs/38875113607/job/116888169239),
attempt 1, used reviewed source head
`0e8027d4905cf4ff7a66c3d860683112d98a8ef3`. Source admission, checkout,
preflight identity/cleanliness, ordered merge-parent verification, pinned
toolchain installation and format passed. This observes the explicit consistent
checkout/status byte policy working; it does not prove an earlier failure's
byte/config cause or alter historical W1/W2/W3 outcomes.

Strict locked workspace Clippy then rejected
`tests/v1_path_budget.rs:434`, where the retained-prefix assertion constructed
its expected path with `small_caller.parent().unwrap().join("/a")`.
The original log is retained as `ktlsp-platform-116888169239.log`, SHA-256
`43f4c57e7feca05377677388b4997359843bc1a37210a6872527029e026aa796`.
This is a real Windows compile/lint failure, not a project test-body result;
the full release workspace tests were not reached.

The source correction uses the independent expected native path
`\\server\share\a`, matching the existing v2 retained-prefix test convention.
A rooted-only input discards `dir` while retaining the UNC server/share prefix;
removing the input's leading separator would instead test the wrong behavior.
The input paths, exact-limit admission and one-byte-over-limit rejection,
error budget/limit, v1/v2 comparison, platform cfg and production resolver are
unchanged. No lint allowance, warning suppression or resource-limit change is
introduced. Windows/Cygwin execution of this changed assertion remains pending
a separately admitted original changed-source platform run; no unchanged retry,
local parser execution, push, dispatch or merge is supplied by this correction.
