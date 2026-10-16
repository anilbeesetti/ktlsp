# Session handoff — native Kotlin LSP continuation

## User request and stop condition

The user still wants full official Kotlin LSP functionality in Rust, faster through substantial parallel work and good code quality. **The goal is not finished.** The user explicitly requested this checkpoint and a prompt for a new session; pausing here is authorized, not a claim of completion.

The user changed `task.maxConcurrency` globally. Reading `cfg://task/maxConcurrency` returned **0**, numeric, source `global config`, default 32. An authorized write of `0` returned **already 0; nothing changed**. This session nevertheless had a separate explicit 32-concurrent instruction; do not assume its old instruction remains or disappears in the new session. Follow the new session's actual higher-priority limits, use all permitted capacity for genuine independent work, reserve two reviewers per implementation owner, and do not pad blocked/overlapping tasks merely to claim hundreds. This session launched 20 direct tasks plus nested reviewers, not hundreds.

## First actions

1. Read repo AGENTS.md, relevant skills, README, docs/PLAN.md, specs, and this handoff. Most public status documents intentionally freeze an old assessment; do not mistake it for present-day main or full parity.
2. Take over the root-owned product integration worktree below. Checkpoint code is committed, with **no unresolved merge and no active merge lease**. No product-union checks or smoke have yet run. Do not merge it as-is.
3. Finish the product integration (#319), including current README corrections, affected union reviews, formatting/Clippy/native checks, real composed-surface proof on hosted Linux, and exact guarded integration.
4. Only after #319 has verified original exact-main CI and independent OWNER+ROOT proof, grant the semantic and packaging lanes one stable incoming main SHA. They deliberately stopped before another integration/check cycle.
5. Continue toward scopes/binding/types and the remaining required features; never declare full parity from these finite subsets.

## Repository and ownership

Root repository: `/Users/anil/Developer/personal/ktlsp`, remote `git@github.com:anilbeesetti/ktlsp.git`.

The original root/main checkout was **not changed**; it remains at historical `0eb88b0becc256458c6ae8b3663e850cb3912375`. Remote main is newer. Do not overwrite user work or move a checked-out main ref behind its working tree.

Successor root is authorized to take over this root-owned checkpoint:

- Worktree: `/private/tmp/ktlsp-product-integration`
- Branch: `omp/product-integration`
- Issue: https://github.com/anilbeesetti/ktlsp/issues/319
- Code checkpoint before adding handoff documents: **`362cfaefd66493611ad9392c44d51c1cb06921f4`**, tree **`ff1408c60aa199583e5ae24db071aed1eb0d3a86`**.
- The handoff commit is a descendant of that code checkpoint. The branch is backed up to origin. There is **no product integration PR yet**.
- Preserve component history; do not force-push main or bypass branch protection.

Other `codex/*` and pre-existing worktrees are user/previous-agent work. Leave them untouched. Especially preserve the original uncommitted #308 worktree and the c13-parenthesized receiver worktree. New agents should use their own ticket worktrees or explicitly take over the named successor-owned checkpoint, never casually edit another owner's checkout.

## Verified delivered remote main

Current remote main at checkpoint: **`d208525a3ee5e21a16fb62b0d4a24230ddf31085`**. All exclusive leases described here are cleared after independent OWNER+ROOT proof.

|Delivery|Reviewed head|Squash / sole parent|Reviewed and merged tree|Original exact-main CI|
|---|---|---|---|---|
|[PR309](https://github.com/anilbeesetti/ktlsp/pull/309), artifact lockfile integration, supersedes closed PR269|`f19aafe69ad1f86053d8f2295d211562495a3109`|`a009cdfc8d075d9aef4b4cab9fb790835db22e4c` / `0eb88b0becc256458c6ae8b3663e850cb3912375`|`278348ee4d537ea70f8212c6597218ad81d1a34d`|[38842946851 / 116786946983](https://github.com/anilbeesetti/ktlsp/actions/runs/38842946851/job/116786946983)|
|[PR313](https://github.com/anilbeesetti/ktlsp/pull/313), corrected optional G2 anchor, supersedes closed PR281|`9a6ec663b04804cd1198755fffa8380ae94df6fd`|`d208525a3ee5e21a16fb62b0d4a24230ddf31085` / `a009cdfc8d075d9aef4b4cab9fb790835db22e4c`|`fdcb7b2404fe05c380c500dd83e3f4a0850b621c`|[38844101516 / 116790565872](https://github.com/anilbeesetti/ktlsp/actions/runs/38844101516/job/116790565872)|

Both original attempt-1 main push runs passed unchanged pinned Rust 1.99.0 format, locked all-target workspace Clippy and locked workspace tests: **821 = 808 ordinary + 13 compile-fail**, zero failures, two exercised ignored helpers; separately 22 protocol fault cases and two production CST snapshots passed. These counts describe those exact integrations, not parity percentages or future heads. OWNER reviewer `ToolsPRCorrect` and ROOT independently authenticated GitHub API identities, checkout logs, workflow blob `fc6202e6d32a00d430f4d4e12b502b6ca538edab`, executor and all mandatory step results.

G2 security fix: the phase Python invocation lacked `-I`; an adjacent unpinned `pathlib.py` could execute before route publication checks. Root and an independent reviewer reproduced the harmless sentinel import and its exclusion with `-I`, using an unchanged pinned auxiliary route in temporary directories. Both invocations are now isolated. Fresh preflight/admission must include the corrected workflow hash. **No optional dispatch, provisioning, candidate compilation, Docker/JVM or official capture was performed. G2 remains UNVERIFIED; execution/compile admission remains HOLD.**

## Product integration #319 — code composed, NOT verified as a union

Four independently reviewed components have been merged into the root integration branch:

|Component|Exact selected source head|State/evidence|
|---|---|---|
|[PR311](https://github.com/anilbeesetti/ktlsp/pull/311), #310, protocol compatibility|`cf8d8b27ec3880f3b298f2bdadeec89b3009055f`|UTF-16 mandatory fallback, standalone `--stdio`, stale wording/capability snapshots removed. Local format/strict Clippy/54 server tests and actual fragmented Unicode stdio passed. Both reviewers cleared. [Original final-head CI 38844948581 / 116793250031](https://github.com/anilbeesetti/ktlsp/actions/runs/38844948581/job/116793250031) passed.|
|[PR314](https://github.com/anilbeesetti/ktlsp/pull/314), #312, syntactic symbols|`4c3443841e43152541726786cc49b937f9f3456c`|Validated CST, hierarchical/flat profiles, trusted matching sibling worker, honest limits/errors, eight permanent actual-worker tests. [Final CI 38844713757 / 116792497739](https://github.com/anilbeesetti/ktlsp/actions/runs/38844713757/job/116792497739) passed. No local parser execution.|
|[PR316](https://github.com/anilbeesetti/ktlsp/pull/316), #315, JAR class consumer|`92f13c18408a6dbafc15f839504a92ef436858ce`|Bounded stored/deflated payloads, descriptors/CRC/overlap, reusable scratch and `ktlsp_binary::parse_zip_class`. Genuine unchanged Commons Lang JAR and notices retained. Local strict checks and 513 artifact/binary checks passed; separate offline consumer observed Builder 52.0, flags0x0031, Supplier, 4 fields, 7 methods, 2510 bytes/pinned SHA. [Original CI 38843990181 / 116790212052](https://github.com/anilbeesetti/ktlsp/actions/runs/38843990181/job/116790212052) passed.|
|[PR318](https://github.com/anilbeesetti/ktlsp/pull/318), #317, negotiated lexical pull diagnostics|`f8a2a3504bb8edb4c0e6384b7feb6974e6ed8e0a`|Local format/strict workspace Clippy/54 server tests and independent focused reviews/tests passed. ROOT separately exercised real framed stdio with empty PATH/no Java variables: pull flags false interfile/workspace, UTF-16 range5..6 after an astral character, full empty replacement, closed InvalidParams, no duplicate push; legacy push+clear and MethodNotFound retained. **Its hosted CI has not been inspected by ROOT.**|

### Conflict resolutions already made

- Protocol merged first, then Symbols, then Pull, then JAR; all component merges are committed.
- Session init retains **both** `(configuration, pull_diagnostics)` parsing and symbol-client validation before state commit. It advertises each implemented provider under its actual conditions.
- Both request arms and both private provider functions retained; push diagnostics and pull share the original bounded cache helper.
- Server README preserves matching-worker build/distribution/symbol limits and negotiated pull behavior; foundation spec preserves both `--stdio` and pull contracts.
- Cargo.lock combines server→parser plus JAR dependencies. Only new external packages are exact miniz_oxide0.9.1 and adler2.0.1; no JVM/C parser dependency. JAR owner preserved serde lockfile edges and corrected seven existing ambiguous empty-byte assertions to equivalent `is_empty()` forms after serde exposure.
- No parser production code was changed by the four product components.

### New composed-path regression and temporary observation

`crates/ktlsp-server/tests/document_symbols.rs` now:
- launches packaged servers with `--stdio`;
- retains the original eight consumer tests;
- adds `packaged_stdio_combines_symbols_pull_and_mandatory_utf16_fallback` to exercise simultaneous symbols+pull capabilities, UTF-16 fallback when only utf-8 is listed, no duplicate push, hierarchical `Combined/item`, an edited astral-character diagnostic, and controlled symbol rejection for malformed text;
- temporarily writes **`PRODUCT_STDIO_SMOKE`** with actual decoded wire replies using direct `Write`, bypassing libtest capture.

**This new composed test and marker have NOT run.** Get actual hosted Linux proof, inspect the observed wire and exact checkout, then remove the temporary marker only, retaining the meaningful combined regression. Final reviewed head must receive its own CI. No local parser runtime is authorized under #96.

Prior component symbol surface was independently observed by ROOT in [run38843829593/job116789694423](https://github.com/anilbeesetti/ktlsp/actions/runs/38843829593/job/116789694423): source `fbed703712e04c2c6b1f3a222de3cd8cc029f26a`, synthetic merge `1ff6c236d4ae0f0bac655883184ce28e159eba52`, parents `[a009cdf...,fbed703...]`, tree `c258d00db6f154fb187b392855c7cbde37fd1c19`. Actual wire advertised symbols and returned Outer/inner with correct ranges. Temporary component marker was removed in final PR314 head. This is not proof of the new product union.

### Finish before product publication/merge

- Correct root README current build/behavior claims: build/install **both** executables, accept no args or `--stdio`, optional syntactic symbols consume the parser, selected method8/descriptor decompression and real JAR→class parsing now exist. Keep full semantics/JAR-to-symbol/server-library integration unimplemented; do not rewrite old fixed assessment counts into current evidence. Current long README paragraphs still incorrectly say no parser consumer and no decompression.
- Run final format and locked all-target workspace Clippy. Locally run artifact/binary and parser-free server suites only; avoid `cargo test -p ktlsp-server` unqualified now because it includes real parser consumers. Hosted unchanged full workspace CI must run all parser/symbol cases.
- Run real composed native surface smoke, not only a test-count claim. The hosted actual-wire marker provides the otherwise locally-held parser surface proof. Re-exercise integrated non-parser CLI/API paths with throwaway smoke as needed, then remove scaffolding.
- Obtain affected union functionality/adversarial rechecks. Previous component owner/reviewer names: `ProtocolCompatibility`; `SyntacticSymbols.SymbolsCorrectness`, `.SymbolsAdversarial`; `JarClassPayload.JarPayloadCorrect`, `.JarPayloadEdges`; `PullCorrect`, `PullAdversary`. Old session agents are not assumed reusable; fresh independent reviewers may be necessary. Component approvals alone do not approve the conflict-resolved union.
- Create the root-owned integration PR for #319; component PRs remain open until verified aggregate delivery, then supersede them and close only child issues #310/#312/#315/#317. Broader parent tickets remain open.

## Semantic prerequisite wave #320 — frozen and backed up

- Issue https://github.com/anilbeesetti/ktlsp/issues/320
- Branch **`omp/320`**, origin backup **`20a26ee37765724fc8bd4bdefc2a3f1a40a0b75a`**.
- Worktree `/private/tmp/ktlsp-omp-320`; original user snapshot retained read-only at `/private/tmp/ktlsp-omp-320-source-snapshot`.
- Accepted prerequisites merged in order **307 → 304 → 299 → 302 → 305**, union `05ce52c5a34e87455a8039b8e0fcbe9fc17c4c74`. Sole workflow conflict retained main's corrected `python3 -I` and artifact data. No new dependency/lock edits.
- After dependency union/hash recheck, adopted only the two #308 files from the original user's `/Users/anil/.codex/worktrees/t009-bare-local-reference-initializers/ktlsp`, without modifying that original.
- Original/adopted SHA256: lowering source `cf43bb31f29d90fed9420c775c1ca3d03799bfafd5dc3ae2fc8b7a5519a4a422`; tests `6ecd5f3d52b73a3e566438b9b4a82b18134be65eafce5175ccfe3e9ab352d28b`.
- Reviewer finding fixed: function-level EQ could replace missing parameter list; initializer transition now restricted to properties, with a synthetic refusal regression. Final lowering source SHA256 `f7439f2a03f834ea186b7dca839241389293d452bf72ab5cf4d7a65f46fe5832`; adopted tests unchanged.
- Final static reviewers: `SemanticPrereqWave.SemanticCorrectnessReview` and `.SemanticAdversarialReview`, both clear at20a26ee.
- Production test asserts the frozen **37-row/27-token/zero-diagnostic** capture, 3 declarations, 2 regions, memberships `[0,1,1]`, same-origin lower_declarations→build_regions. Synthetic cases cover context/order, errors/siblings, linear work, stale/origin rejection and resource/allocation failures.
- **No local checks/runtime, PR publication, main merge or docs promotion.** Push-only backup was granted for this handoff. Await ONE actual verified main incoming after #319, then integrate/check/review/publish once. It remains declaration/reference-syntax admission, not binding/navigation/type semantics.

Original prerequisite heads: PR307 `9a03c349fa11dd61aa217fe59ada85ee091921a8`; PR304 `b34a370a2a660795f95b610bbc9ee1065549dd7b`; PR299 `c90fd9af478698ce712ad9751fccee9d7dd0e934`; PR302 `e98e01eca847b0a3c6ad870c893d30963745b895`; PR305 `d26a387765606637ced4124fc270f153d84940c5`. They each received separate functional/adversarial source review with no findings. PR304 original capture was independently accepted from [run38834058424](https://github.com/anilbeesetti/ktlsp/actions/runs/38834058424), merge `059646c5addc828e1ea70fc90cdf05214702e1da`, ordered parents `[7917f02...,b34a370...]`, tree `ad7539fab7850f796db215bf8123d781fb3debf3`. REFERENCE_EXPRESSION row32,parent25,range29..30,subtree_end34; IDENTIFIER row33,parent32,same range. One unrelated Cargo Running line was interleaved in combined CI transport; all37 consecutive JSON rows were present. That historical capture is not a current-union runtime proof.

## Native packaging experiment — source only

- Branch **`omp/native-packaging`**, pushed backup **`5c134d29e0efe680e39a94ac1f87050d2d0aeda1`**.
- Worktree `/private/tmp/ktlsp-omp-native-packaging`.
- Only adds `.github/workflows/native-packaging-experiment.yml`.
- Independent static reviewers `NativePackaging.PackagingCorrectness` and `.PackagingAdversarial` found no P0/P1/P2 issues.
- Intended manual/default-branch immutable-SHA matrix Linux/macOS/Windows: pinned Rust1.99; format, locked Clippy and full release workspace tests; matching server/worker release executables; parser/vendor and dependency licenses, Cargo.lock, build/workflow/OS/arch/commit/tree evidence, per-file checksums and tar+outer checksum; upload only on complete success. It rejects Cargo-zero/objcopy errors rather than hiding them.
- **UNEXECUTED. No PR, dispatch, artifact or general platform support proof.** Docs intentionally await actual proof. Integrate stable verified #319 main first; root must review publication, pass CI, and separately admit any manual dispatch. This is not full T046 release parity. No first-party license was invented.

## Remaining old PRs / known blockers

- **PR306 P2 — must fix before integration:** `tools/oracle/src/bin/ktlsp-oracle-pr174-host.rs` at original head `05fc5ab7033f5bf5a5bb80f1a1e848879ee79b24`.
  - `make_file`595–604 can leave an empty/partial file on write/sync failure.
  - `install_cleanup`1454–1480 writes the cleanup unit before daemon-reload/readback succeeds.
  - `supervise`3045 routes cleanup using unit-file existence, not successful ownership transfer/service usability.
  - Direct prearm cleanup2488–2493 rejects any existing unit file. On partial install failure a created container can have no usable cleanup owner/watchdog.
  - Track successful ownership transfer and safely roll back only an owned partial installation or otherwise retain direct cleanup; preserve pre-existing/unrelated units. Trace every helper caller and add a consumer-visible fault-injection regression. No fix was started. Previous security-only reviewer missed it; functional reviewer found it. Existing 15 synthetic tests and Linux/macOS CI do not cover this failure.
- **PR300** head `9318ffbe1ab97a386cf599ae49310e8c036616a6`: functional/security source clear, bounded consistency validation only. Original native CI38825373608 and optional-tools CI38825373594 passed; six PR174 tests on each Linux/macOS. Final Docs/Ancestor incoming union still gated. It does not supply/authenticate an official adapter or authorize launch.
- **PR306** original native CI38835118032 and optional-tools CI38835117928 passed (15 host tests plus ignored helper) but cannot override the P2.
- **PR255** historical docs head `c06aed06dacbbadf753a7c6212d4b5ce02d75c1d`: two-file append-only historical receipts; separate functionality/adversarial source reviews clear. Functional reviewer authenticated linked original PR/main CI counts, W3 failure and eebb349626 facts; adversarial reviewer could read source/Git/issue facts but Actions auth failed, disclosed rather than fabricated. Still open, not integrated; combining it with G2's EOF ledger append requires preserving both histories. Root owns this historical-doc integration, not #320.
- Existing c13-parenthesized receiver worktree remains user-owned/unmodified; do not duplicate or overwrite its unpublished work.

## Research saved for the next ready consumers

- [Finite binding/navigation contract research](session-2026-10-16/binding-contract-research.md): primary pinned Kotlin scope/overload/compiler evidence, ordinary local-val initializer ordering, exact proposed authenticated reference-site/binding consumer and residuals. Do not equate candidates/result-local ordinals with resolved or cross-revision symbols. Self-reference/forward/outer lookup must not be guessed. Existing TypesScope found no independent consumer-backed T012/T013 implementation ready before source binding/expression lowering; do not add an unconsumed generic type algebra just to occupy agents.
- [Missing official adapter research](session-2026-10-16/oracle-adapter-research.md): exact PR306 invocation/handoff schemas and existing bounded recorder reuse. The Rust official adapter is absent; protected original-job bootstrap/authorization, immutable image/workflow, same-configuration containment calibration and independent evidence receiver remain prerequisites. Actual locally retained archive/provenance exists. Do not claim fresh official capture or offload production semantics to JVM. ROOT still owns the cleanup P2.

## Guard and verification discipline

Repo AGENTS is authoritative. Every significant integrated behavior needs actual smoke, format, locked strict Clippy, relevant tests, full hosted workspace checks, two independent reviews and rechecks of changed behavior. Native default graph stays JVM/JDK/compiler/JNI/C-parser free. Optional user-existing JDK/build tools and a separately isolated official oracle are permitted only under runtime-boundary.md. T004 remains data-only.

**#96 remains OPEN/UNKNOWN.** Do not repeat local parser tests to confirm old unchanged-worker timeouts, increase the500ms deadline, retry until green, accept Timeout in assertions, or suppress failures. Hosted original Linux CI has exercised the new parser-consuming surfaces. Native macOS release previously had rust-objcopy SIGABRT/missing `@rpath/libLLVM.dylib` despite Cargo exit0; do not hide it or claim a repair from debug builds.

Administrative helper `/private/tmp/ktlsp-merge-guard.py` (SHA256 `6d9fd1664802caae5820f6583add54c3aff7d06e91341c4283a9fcb42b5504da`) was used successfully for309/313. A data-only reference is saved alongside this handoff. Lease JSONs309/313 are historical/cleared: **never reuse them for another merge**. The helper assumes a root integration head already contains the leased main, with identical local head/tested merge tree. This holds for the composed product branch; do not blindly apply it to an old unintegrated PR.

Fresh merge guards MUST run in a separate strictly read-only tool call. Explicitly inspect actual exit_code==0 and all complete valid facts matching the active exclusive lease: exact current remote main, source head, PR base, tested merge parents/tree, state/checks, scope/config and clean reviewed local identity. Any timeout/nonzero/missing/malformed fact/mismatch suspends the lease. Only then perform a separate ordinary expected-head squash (no bypass/force). Retain exclusive base ownership through exact squash tree/sole-parent verification and independent OWNER+ROOT original attempt-1 exact-main CI proof. Expected head alone does not prove base.

GitHub Actions checks are attached to the **PR head**, while the actual PR workflow checks out a **synthetic merge**. Verify both head check status and original checkout log + merge Git facts; querying only synthetic-merge check-runs returned an empty list in the first halted309 guard, and no merge occurred until the corrected complete guard passed. No automated polling loops; use finite gh watches/background completion delivery. Do not treat old job IDs/agent tool artifacts as new-session live handles.

Language-server reference lookup failed unexpectedly with code0 in this session and was reported through xd://report_issue; fallback grep confirmed the sole private parse_initialize caller. Recheck tool availability if code intelligence is needed; do not assume the failure persists or invent references.
