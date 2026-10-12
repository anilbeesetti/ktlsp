# T007b2/T007b3 source-local parser repairs

Child #87 of #83/#7; production development follows merged #86 and explicit coordinator handoff.

Sources copied from the crates.io `ktrs-parser` 0.5.1 archive, SHA256
`ec8d43d9549490689477385c7678bbd22ecbc445973ab364e8ecb69dd4882bcc`.
`upstream-sha256.json` inventories original Rust source bytes. Upstream repository:
https://github.com/Hexay/ktrs (research pin a703f88d6d4ac9d983a17df66ddf6ac403546c00).
The registry archive is authoritative for this fork. License/NOTICE files retained
from the previously audited project notices; the published archive omitted them.

Only Rust `src/` is copied, including upstream unit-test source, without upstream
examples/integration fixtures/workspace or dev dependencies. Minimal standalone
manifest pins existing `ktrs-lexer`/`ktrs-syntax` 0.5.1; their sole transitive normal
dependency is `text-size` 1.1.1. No external dependency addition. Upstream lib unit
tests depend on expect-test and are not run by this standalone harness; no
mimalloc or facade/native parser dependencies are added. Ticket tests are original.

The product parser selects this fork as a path dependency. The root workspace
excludes the vendor workspace, keeping upstream dev dependencies out of default
tests. Production regressions live in crates/ktlsp-parser and traverse the actual
supervised worker through parse_snapshot/tree_for. Standalone tests alone do not
establish delivery; integrated review/checks/CI remain required. Direct upstream
parse_file requires LF; CRLF/lone CR production tests exercise the existing
normalized-to-original mapping through the supervised worker. Original input source
is never modified by this repair. Full Kotlin grammar and semantic parity remain pending.

Source changes are recorded in two ordered phases:

1. `recovery.patch` changes nine files from the pristine archive: parsing/mod.rs,
   builder/layers.rs and declarations/{classes,functions,members,modifiers,parameters,type_parameters,user_types}.rs.
2. `formatting.patch` changes 49 files from the recovery phase at commit cc4920d,
   using pinned Rust 1.99.0 rustfmt with edition 2024. All nine behavioral files
   are among those 49; the other 40 have formatting changes only. Five source
   files retain pristine bytes. Exact paths are the patch headers; the final
   54-file byte inventory is `formatted-sha256.json`. No formatting opt-out or
   changed root formatting command is used.

Applying recovery.patch then formatting.patch to the inventoried pristine source
reproduces all final source bytes. Formatting-only review and production rechecks
are separate from the earlier behavioral evidence; commas and import order need
explicit review in addition to whitespace. Original archive hashes and notices
remain unchanged.

The behavioral patch checks ordinary function parameter boundaries, probes existing modifier
and annotation grammar with rollback, and reports a missing ')' without consuming
a following declaration. Nested type/default parsing remains with existing grammar.
Constructor, function-type and context parameter loops keep their original context.
EOF diagnostics retain upstream behavior. A scoped speculative-probe flag avoids
recursive replay when annotation arguments contain anonymous functions. Remaining
work is superlinear in nesting; existing production resource gates remain required.

Known bounded repair limits: a missing default (`x: Int =` followed by named fun),
deeply malformed parenthesized types (`x: (@A`), malformed annotation arguments,
and unclosed class generic lists outside the bounded #100 simple-function-header
contract can still consume later declarations. These are
upstream grammar/recovery gaps, not accepted recovery or full grammar parity.
Missing simple types and open generic argument types improve recovery but retain
upstream diagnostic deduplication (e.g. only `Type expected` at one offset).

Run the bounded source-local checks with:
`cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/Cargo.toml --test recovery`

`worker-probe/` preserves the isolated production-source experiment without code
duplication: its manifest references existing production worker/library/tests and
patches ktrs-parser only in its standalone graph. Run:
`cargo test --offline --locked --manifest-path vendor/ktrs-parser-0.5.1/worker-probe/Cargo.toml --tests`
Neither standalone workspace is run by root workspace tests. Equivalent production
regressions now live in the parser crate and must pass integrated review/checks/CI
before #87 can close.

## #100 bounded classifier-generic recovery candidate

The additional behavior is confined to class/interface type-parameter boundaries;
objects and other generic callers retain ordinary parsing. Completed bounds require
specific surviving completed TYPE_REFERENCE/concrete-type production events and
closed type/value lists, not a void-helper return or merely no new diagnostic.
An isolated scratch parser reuses existing annotation/modifier grammar and current
committed token kinds, then requires FUN/name/LPAR. No live cursor, token remap,
production, diagnostic, newline/join or layer state is changed by a probe.

The scratch slice stops at the minimum nonnegative active truncation endpoint,
256 complete raw tokens including trivia, or 4096 bytes. All attempts are charged
against 4096 raw tokens per classifier list; exhaustion declines further recovery.
These are recovery-coverage bounds, not new worker failures or larger global limits.
The existing recursion-suppression flag disables nested parameter/generic probes.
Scratch byte limits do not bound RSS: the existing builder pool can reuse larger
buffers. Original archive checksum, upstream inventory and notices are unchanged.

The layer minimum uses compile-time assertions for absent, negative, nested,
reordered and zero boundaries. Production tests exercise exact declaration and
name ranges/parents, damaged class/list extents, same-offset diagnostic dedup,
original newline mapping, valid controls, incomplete bounds/excluded headers,
per-probe thresholds, aggregate decline and repeated recovery. Runtime evidence
must come from the authorized exact-head Linux CI; no new Darwin runtime test is
used for this ticket. Standalone vendor tests are not part of root CI.
