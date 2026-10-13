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
2. `formatting.patch` changes 49 files from the current recovery phase (extending historical cc4920d),
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
a following declaration. Nested type parsing remains with existing grammar; the finite C14 default-entry
repair is described below.
Constructor, function-type and context parameter loops keep their original context.
EOF diagnostics retain upstream behavior. A scoped speculative-probe flag avoids
recursive replay when annotation arguments contain anonymous functions. Remaining
work is superlinear in nesting; existing production resource gates remain required.

Known bounded repair limits: missing defaults outside C14's complete simple zero-parameter header,
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
256 copied complete raw tokens including internal/trailing trivia, or 4096 source
bytes. Every eligible scratch window, including failed grammar, is charged before
parsing against 4096 copied raw tokens per classifier list; zero-copy eligible
rejection debits one and exhaustion declines recovery. Cheap kind/modifier/EOF
prefilter misses remain uncharged under the coordinator-amended contract.
Leading trivia inspection is bounded separately by input size and the advancing
live cursor; nested scratch probes return before scanning it.
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

A focused post-candidate audit identified that the original all-trivia/all-failure
reading was broader than the implemented copied-window counter. The coordinator
explicitly amended the private contract to the accounting above after independent
architect review; documentation alone did not resolve that original mismatch.
The actual loop now uses a private const byte/EOF predicate with unconditional
assertions for endpoints before/at/inside a token, absent EOF and independent
4096/4097-byte boundaries. This extraction preserves the original subtraction,
inclusive endpoint comparison and absent-EOF behavior. The minimum active EOF
feeds that predicate; an empty window returns false before constructing scratch,
and incomplete prefixes cannot satisfy FUN/name/LPAR. These are window-predicate
checks, not annotation parsing, active-layer integration or end-to-end truncated
recovery evidence; that remaining gap belongs to #83's acceptance audit.
Large-leading-trivia runtime evidence and renewed resource reviews remain pending.
No full-parser complexity or RSS bound is asserted.

## C14 / #121 ordinary missing-default entry

Only parameters.rs changes behavior in this child. Call-local ordinary-list
recovery context excludes constructor/function-type/context/direct parameter
paths without ambient Parser state. At post-EQ entry an isolated scratch builder
uses four grammar expectations to recognize FUN/IDENTIFIER/LPAR/RPAR. The window
ends within active truncated EOF,256 complete raw tokens including internal trivia,
4096 bytes, or four nontrivia tokens. The recursion-suppression flag remains
separate. There is no live speculative mutation or text-search reconstruction.
Broader headers remain recovery gaps; leading trivia inspection is bounded by the
input limit, outside copied-window accounting. Worker/resource/Coverage contracts
are unchanged. The ordered phases were recomputed as9 behavior/49 formatting/54
final source files with5 pristine survivors, not inferred from historical counts.
Source-frozen expectations and review requirements are in
../../crates/ktlsp-parser/reviews/c14-default-recovery.md. Controlled Linux validation
and coordinator delivery gates remain required; no new Darwin execution is used.
