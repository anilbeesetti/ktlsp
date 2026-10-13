# #138 review and validation evidence

Frozen source base64234587e333ebcd32d96600a7a193b06b0ac9df,
tree3f6828ce85a8b74a9142c4fd6d9893ad107210d3. Independent implementation checkout
/private/tmp/ktlsp-payload-implementation-01a13424, branchcodex/metadata-payload-138.
Issue https://github.com/anilbeesetti/ktlsp/issues/138 is exact reviewed Stage1 text;
one admission comment records the subsequent finite source/publication grant.

Final Stage1 correctness01a13424-df8a-7bd0-b126-a37ae154d045,
adversarial01a13425-0bb7-7012-ad56-8024135e3d46 and
architect01a13424-ba4d-7333-a8ee-e45090ff689b approved exact six artifact hashes:
PROPOSAL269112bd809d3c0c13d28eb4cda03cda036f3e7cfb6b7711ad0151a73e3c81b8,
FIXTURES8ca7d97dbf9bf270744c5aeb9fd5422b6d6b8bb8915538dd64ed2f6c32fd98c8,
ISSUE315a7bd374dd49cda0174181ba444760332e03307b8253baeea27b9bf385722f,
and the three raw fixture hashes in fixtures/metadata_payload/README.md.
Same reviewers retained for actual source and eventual completed CI.
Stage1 approval is not source/runtime/CI approval.

Source changes are the new payload module, exact private envelope and table budget
extractions, additive metadata::payload export, dedicated tests/fixtures/spec/review.
No existing tests/API/error variants/limits changed; no Cargo/lock/dependency/workflow/
header/name_resolver/parser/vendor/server/lib.rs changes. One budget covers THIS
envelope+typed-table call only. Payload is borrowed opaque with no work debit;
nested spans remain table-relative, new API errors d1-relative. Header/d1/resolver
budgets, class/package declarations/types, semantics/providers and whole-pipeline
CPU/RSS/official comparison remain outside this contract. #68/#18 OPEN, T008 HOLD,
#96 OPEN/UNKNOWN, B3/B4 GAP, B11 UNVERIFIED.

Local pinned `cargo fmt --all -- --check` initially exited1 with style diffs in
new payload.rs/metadata_payload.rs only. Applied `cargo fmt --all`; subsequent
identical check exited0. `cargo clippy --locked --workspace --all-targets -- -D warnings`
exited0, dev profile finished4.60s, compiling/checking all workspace targets without
executing tests. Initial source reviews from all three retained reviewers found no
product defect and requested the same exact capacity-growth test at len4. Added
five-element unknown Field and local-int exact/one-less work/owned cases with
manual work23+8F/work28+8I and final error offset9; frozen fixture hashes unchanged.
Recheck and final committed source review are pending; runtime CI remains pending.
Capacity-case compile-only recheck initially exited101: Clippy manual_bits mistook
8*sizeof(i32) retained storage for a bit-size calculation. Replaced that expression
with sizeof([i32;8]), the same eight-slot byte capacity, without suppressing the
lint or changing fixture outcomes. Failure log stage2-clippy-final.log retained.
Corrected pinned fmtcheck and identical compile-only workspace/all-target Clippy
both exited0; dev profile finished0.48s. Logs stage2-fmt-corrected.log and
stage2-clippy-corrected.log preserve those receipts separately from the failure.
Auxiliary logs: /private/tmp/ktlsp-payload-package-01a13424/stage2-fmt-check.log
and stage2-clippy.log. Formatting failure is preserved; its source formatting was
fixed before recheck, no product assertion/failure was retried to green.

No local runtime/tests/native/server/parser/worker/bench/
JVM/oracle/Gradle/Maven executed. Only pinned formatting and compile-only Clippy
admitted. Runtime must use unchanged existing Linux Native Rust all3 after a
separate root stable-integration grant. Integration/push/PR/CI/merge held.

Preserved preparation failures: initial sandbox SSH clone DNS failure (approved
same-endpoint clone later succeeded); read-only issue retrieval network/schema
errors; two package/source cwd mixups (missing manifest then missing HANDOFF/notgit).
Architect reviewed correction: separate explicit workdirs. None were product/runtime
or CI failures. No outcome-tuned fixtures or retries-to-green.
