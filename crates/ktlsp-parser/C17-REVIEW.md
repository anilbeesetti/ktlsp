# C17 / #104 finite residual fixture review

Base: actual main `4d27673f703fd3c7b0ee57fc7a3cf35c6c21f05f`, parser source
`ab00d55557cfea9bcee87aa41fe2922f82770027`. Authoritative final audit:
https://github.com/anilbeesetti/ktlsp/issues/83#issuecomment-6130232640,
classification SHA256 `b54043bde87143ce0310e18cb2a67bc5f46fcd7b285966e3a3f8076d9267fb1c`.

Scope is only new `tests/recovery_residuals.rs`, CORPUS.md, README.md and this
review file. No production parser/vendor/API/worker/Cargo/lock or existing-test
changes. Existing five-bound/six-header tests are mapped with their narrower
negative outcomes; missing accepted projections belong to separate C12 work.
Resource controls remain resource-only. This child records required C14–C16
failures and does not repair them, close #83/#7, admit T008 or establish parity.

## Independently authored expectations

The three fixed source literals end at their final `2` without a terminal newline.
Initial declaration and diagnostic expectations were authored from parser control
flow before the first runtime execution, not captured CST snapshots. Diagnostic
corrections after the failed CI were independently rederived from source as
recorded below. Both reviewers traced the following corrected paths:

- C14: `parameters.rs` default expression enters `atomic.rs` local FUN parsing with
  newlines disabled. The later FUN is nested under VALUE_PARAMETER through EOF;
  outer parameter-loop EOF gives a missing-comma error item and a distinct
  missing-RPAR error marker; both remain.
- C15: `types.rs` tentative parenthesized type rolls back to function-type parsing.
  Fallback `Type expected` binds before trivia at17, missing RPAR consumes18..21,
  missing arrow binds before trivia at21, return USER_TYPE ends29. Outer missing comma/RPAR consumes29..30;
  remaining RPAR/EQ/2 get exact top-level errors. No healthy FUN survives.
- C16: annotation value-argument parsing disables newlines and accepts local FUN
  under VALUE_ARGUMENT. The annotation's missing-RPAR EOF error item is followed
  by distinct error markers for parameter-name and outer missing-RPAR failures.
  All three remain even though their source ranges are equal.

All declaration projections include exact kind, original start/end, immediate
parent, nearest declaration extent and all direct identifier spelling/spans.
Snapshot identity, full contiguous UTF-8 token partition and original source are
strict. Each case performs a new healthy revision with exact FUN/name/parent/span
and no diagnostics, then rechecks the retained damaged snapshot result.

## Reviews and executor boundary

Architect plan review approved finite scope, requiring honest existing-evidence
labels and complete independently expected defect assertions. Separate correctness
and adversarial reviewers independently approved test source derivation. Both reviewers rechecked and approved full source/docs/preparation evidence after
correcting the documentation to map only actual caller controls (object behavior
remains unverified). Correctness review independently retrieved the final audit and
PR102 exact-main logs; adversarial review checked local tree/source compatibility.
Finished architect review and authoritative candidate execution remain pending at
this preparation checkpoint.

Both reviewers examined the candidate-specific execution graph: Cargo discovers
one additional ordinary integration-test executable with three active tests, each
starting two trusted production workers (six small subprocess inputs total).
The default harness can run those tests concurrently. No worker graph, deadline,
assertion or resource limit changes; no failure unions/ignored tests are added.
The extra ordinary executor load is not evidence about Darwin issue #96.

Local `cargo fmt --all -- --check` and
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
No local native tests, parser/worker runtime, benchmarks, Java, Gradle, Maven or
oracle ran. Existing controlled Linux Native Rust CI is the only authorized
runtime executor for this child. It must run all three unchanged pinned commands
on reviewed combined head and exact merged main; runtime/CI evidence remains
pending here. Final exact-head identities/check URLs/counts and integration/review
rechecks belong in the PR/coordinator delivery report, without relabeling this
preparation checkpoint. #96 remains OPEN/UNKNOWN; Linux success cannot establish
Darwin reliability or a cause/correction.


## First controlled candidate failure and source correction

[Run38292883243/job114956922744](https://github.com/anilbeesetti/ktlsp/actions/runs/38292883243/job/114956922744)
checked out `dc3bd555e342a27c497722e83657222ba6106c4a` (merge of base4d27673 and
reviewed head72d8dd3); formatting and Clippy passed, tests failed at all three new
exact diagnostic assertions. All declaration projections matched; no Timeout or
worker failure occurred. These failures happened before token-partition and
healthy-followup assertions, so that failed run exercised only three damaged
parses; it did not validate the intended six-worker complete flow or later
workspace targets. Owner and both initial reviews missed the builder distinction.
The first expected tuples incorrectly assumed all
same-offset errors deduplicate and placed two zero-width errors after trivia.
This failure is retained; no unchanged rerun or local runtime was attempted.

Source correction: `abstract_kotlin_parsing.rs::error_with_recovery` calls either
`error` (an error item) or `error_and_advance` (a completed ERROR_ELEMENT marker).
`builder/tree.rs::duplicate_error_items` filters only error items; completed error
markers are not deduplicated by that path. EOF name/RPAR recovery sets without
EOL_OR_SEMICOLON take the latter path even when advancing at EOF consumes nothing.
Thus C14 retains two errors at37 and C16 three at32. `balance_white_spaces` gives
error items DefaultRight binding, placing C15's empty Type/arrow errors at17 and21
before trivia; consuming error-marker ranges18..21/29..30 stay unchanged.
Both independent reviewers rechecked and approved the corrected exact tuples;
the architect approved this correction plan and evidence limits. Only exact
diagnostic tuples and this explanation were corrected. No grammar,
budget, error union, assertion strength or fixture input changed. Independent
source rechecks and a new exact-head controlled Linux run are required.
