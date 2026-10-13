# B3–B5 production resource evidence (#107)

This finite child of [#83](https://github.com/anilbeesetti/ktlsp/issues/83) adds a
source bound and **reconstructed production-reply serialization evidence**. It
does not capture worker stdout. B3/B4 remain GAP, B11 active grammar-layer EOF
integration remains UNVERIFIED, and [#96](https://github.com/anilbeesetti/ktlsp/issues/96)
remains OPEN/UNKNOWN. This is neither full parser acceptance nor Kotlin LSP parity.

The coordinator amended #107 before implementation: only the new
`tests/resource_boundaries.rs` and this document; no direct-worker supervisor,
instrumentation, worker/vendor/API/dependency/workflow/shared-corpus changes.
Runtime checks belong to the existing controlled Linux CI. No local parser or
native tests ran for this child. Delivery/check identities belong in its PR;
until that evidence exists, the new runtime assertions are **unverified**.

## Frozen production identity

Production source M is `ab00d55557cfea9bcee87aa41fe2922f82770027`, tree
`2d46fd856392f961af4c727b9f6ff7f3a561ae09`. This child's base
`4d27673f703fd3c7b0ee57fc7a3cf35c6c21f05f` changes no parser/vendor/Cargo.lock
bytes relative to M. The [refreshed #83 ledger](https://github.com/anilbeesetti/ktlsp/issues/83#issuecomment-6130232640)
has classification hash `b54043bde87143ce0310e18cb2a67bc5f46fcd7b285966e3a3f8076d9267fb1c`.

| Inspected source | SHA256 |
| --- | --- |
| `src/lib.rs` | `e3da67da0c1f78a28395a0862e7bb810ed7124346fa5075c3b393937464c06a0` |
| `src/bin/ktlsp-parser-worker.rs` | `45f9fb6d5b95b02ca6bfe10327adf58df6f6c0e1bb5784804879b67d3905c819` |
| `tests/parser.rs` | `2b3f049e533ac11560cad98d16f6ad323dfb0cd010d1494ca91204ce5b4063b7` |
| `tests/worker_protocol.rs` | `0fdd53f92f2b5730419b89f02bbb505f9bbc98392f284dfa5c225d824ff8e8dd` |
| Root `Cargo.lock` | `96c8242c743b8dbc97b321e9f94c69c5ac7e056f96f0e76ce06ae0433ae4585e` |
| Vendor `formatted-sha256.json` (54 source files) | `f28d2cced3a9206a7c35fb87e1fbcecb2fa780698039d97003e0afbdd4db5156` |
| Registry `ktrs-syntax-0.5.1/src/generated/kinds.rs` | `3e747ba5c7d3eac1066fc6bd0bbfb9af27b2c7c36e14a6fe3d426665862209a8` |
| Registry `serde_json-1.0.154/src/ser.rs` | `f2b8cc0b97e30c49dbc58698fa22c14a7d56ea616cdb57b067835a59af49a75f` |

Locked registry package checksums are respectively
`4b29166c66ba5e9edb079497726eae48b28a5dc405b5e189d54ff96ddbd38e4f`
and `e7e9cc8b1b85264074fbcc02a88680c4096b1e47df8f739dceb03bf482f04bd6`.
Cached source hashes identify inspected bytes; this child did not replay registry
archives. The vendor's ordered recovery/formatting provenance remains unchanged.

## B5 universal bound over the actual worker

`Parser::parse_snapshot -> parse -> supervise` launches the production binary.
Worker `run` calls `ktrs_parser::parse_file`, checks source coverage, then rejects
more than 16,384 elements or 256 error messages before constructing `Reply::Tree`.
It checks depth 128 and individual message size 4096 while constructing fields.
`write_reply` serializes the entire reply with compact `serde_json::to_vec`, then
substitutes `OutputLimit` above 4,194,304 bytes. The parent separately collects at
most cap+1 and rejects excess. Allocation failure may prevent serialization.

The actual serialized fields are exactly:

| Component | Bound and reason |
| --- | --- |
| Element `kind` | Derived `Debug` of the pinned 293-value `SyntaxKind` enum; ASCII uppercase/underscore, at most 64 bytes, requiring no escaping. This is variant spelling, not `debug_name()`. |
| Element `start`, `end`, `parent`, `subtree_end` | Each originates from a `u32` range/tree ID, then casts to `usize`; conservatively ten decimal digits each, even without using the tighter input/count caps. `None` is `null` (four bytes). |
| Element `token` | JSON `true`/`false`, at most five bytes. |
| Diagnostic `start`, `end` | Same `u32` range origin; ten digits each. |
| Diagnostic `message` | Actual constructors inventoried below yield at most 256 UTF-8 bytes; compact JSON escaping uses at most six bytes per input byte. |
| Structural bytes | All field names, quotes, colons, braces and an extra comma per element fit **192** bytes per element (explicit worst-case total 171). Diagnostics fit **1600** bytes (1587 including the extra comma). The `Tree` tag, both array-field names, brackets/braces fit **64** bytes. Extra commas overcount empty/nonempty arrays safely. |

Therefore every completed normal-worker Tree reply at the frozen source obeys
`64 + 16,384 × 192 + 256 × 1600 = 3,555,392 < 4,194,304`, with 638,912 bytes
of slack. Non-Tree `Coverage`, `OutputLimit`, and `NestingLimit` replies are short
quoted unit-variant names. Worker argument/input/missing-message/extra-message
errors cause failed exit, not an additional wire error string.

Source text, token spelling, document identity and revision are **absent** from
the wire schema. The generic 4096-byte message cap with sixfold escaping would
exceed 4 MiB; it cannot establish this proof. Arbitrary caller-constructed public
`Reply` values are excluded. A source/schema/kind/message/serializer change needs
a renewed proof. This proves the normal serialization guard is dominated; it
does not replace the parent's defense against an arbitrary faulty executable.

### Exhaustive production diagnostic origin inventory

Paths below are relative to `vendor/ktrs-parser-0.5.1/src/`, with line numbers at M.
Each listed literal-bearing constructor/argument site was inspected, including
multiline arguments and both branches of conditional arguments. All fixed texts
are ASCII and less than 128 bytes, hence below the conservative 256-byte bound.
Duplicate messages/sites remain listed. This inventory covers both source and
script entry points, lazy block/lambda reparsing, KDoc and Markdown-link reparsing;
it is not a selection of runtime examples.

| File | All diagnostic origin sites (literal arguments unless noted) |
| --- | --- |
| `parsing/declarations/accessors.rs` | 68,71,80,89,95,120 (both parenthesis/bracket literals),173,194,202,218 |
| `parsing/declarations/annotations.rs` | 80,82,85,111,115,121,126,135 (local constant forwarded at139/156); generated messages at174/182 via251–253 |
| `parsing/declarations/blocks.rs` | 22,29,69 |
| `parsing/declarations/classes.rs` | 59,63,68,115,117,130,140,181,184 |
| `parsing/declarations/functions.rs` | 40,79,97,135,151 (format),215 (format),241 |
| `parsing/declarations/members.rs` | 27,29,85,116,123,155,173,223,234,237 |
| `parsing/declarations/mod.rs` | 201,232,286 (includes conservative fragment-entry coverage) |
| `parsing/declarations/modifiers.rs` | 223 |
| `parsing/declarations/parameters.rs` | 138,145,156,173,233,237,242,247,259,265 |
| `parsing/declarations/preamble.rs` | 72,82,88,131,142,156,166,189,200,206,213,290,296,302 |
| `parsing/declarations/properties.rs` | 73,97,120,138,172; static title forwarded at69/103 |
| `parsing/declarations/type_parameters.rs` | 59,64,86,88,200,227,250,260,286,294 |
| `parsing/declarations/types.rs` | 121,177,184 |
| `parsing/declarations/user_types.rs` | 29,30,41,108,124,173,201 |
| `parsing/expressions/atomic.rs` | 92 |
| `parsing/expressions/collections.rs` | 14,23 (literal arguments forwarded by30/55),49,80,85,94 |
| `parsing/expressions/control.rs` | 39,42,48,50,58,75,124,132,179 |
| `parsing/expressions/lambdas.rs` | 20,102 (forwarded by155),111,133,193,209,223 |
| `parsing/expressions/loops.rs` | 29,72,105,118,124,127 |
| `parsing/expressions/mod.rs` | 194 (forwarded by197) |
| `parsing/expressions/operations.rs` | 152,161 |
| `parsing/expressions/primary.rs` | 24,49,89,94,159,164,168,181,205 (forwarded expression argument) |
| `parsing/expressions/statements.rs` | 32,44 (local constant forwarded at48/50),71 |
| `parsing/expressions/strings.rs` | 42,44,84,87,107,117 (includes escaped double-quote literals) |
| `parsing/expressions/when.rs` | 47,53,58,81,91,99,115,129,131,155,165,172,185 |
| `kdoc/kdoc_link_parser.rs` | 18,27,40 |

There are exactly three dynamic diagnostic format sites. `functions.rs:151`
substitutes only the eight-byte titles `function`/`property` from callers52/69;
`:215` uses those same titles (callers60/103) and either the fixed receiver suffix
or empty string. `annotations.rs:253` uses only finite static `keyword_text()`
values, twice; the longest keyword spelling is eleven bytes. Each formatted result
is below128 bytes, conservatively below256. No identifier, token or source slice
is interpolated. `abstract_kotlin_parsing.rs:346–351 current_context()` does format
source, but has no production callers and does not feed diagnostics.

All propagation paths preserve that bound:

- `abstract_kotlin_parsing` error/expect/recovery/error-until/error-if helpers,
  expression/collection/lambda parameter helpers, preamble newline helper and
  `OptionalMarker::error` forward their message unchanged. Their complete callers
  are the literal/format origins above. Token-stream/expression calls/binary
  precedence code adds no message constructors.
- `Marker::error`, `error_before`, `done_before_with_error`, and `PsiBuilder`
  `error`, `process_done`, `done_before_with_error_item` forward to the sole
  `Production::set_message` store. Reused slots clear before copying; no append to
  prior contents or source-context enrichment occurs.
- `builder/tree.rs:199/221` copies stored messages into `TreeSink.errors` (or uses
  an empty default); `TreeSink::finish` transfers errors to `Parse.error_messages`.
  Lazy reparsing uses the same sink and constructors. Worker clones each error
  message into a diagnostic. Empty default messages fail parent validation; they
  cannot enlarge serialization. Missed-token source/debug data produces Coverage
  and is never serialized as a diagnostic.

Independent reviews must verify this entire inventory and propagation, all kind
variants, numeric origins, escaping and structural arithmetic before accepting
the universal claim. A missing/unbounded path is a proof gap, not waived coverage.

## Finite real-worker reconstructed evidence

`lf_production_replies_have_bounded_reconstructed_serialization` uses the actual
`CARGO_BIN_EXE_ktlsp-parser-worker` via `Parser::parse_snapshot/tree_for`. It asserts
source/snapshot identity, full token partition, exact kinds/ranges/parents/subtree
ends/token flags, names where present, and diagnostic spans/messages. Counts below
are independently authored expectations; Linux execution must confirm them.

| Named input | Source | Expected elements / tokens / diagnostics |
| --- | --- | --- |
| `exact-input-whitespace` | 65,536 ASCII spaces | 4 / 1 / 0 |
| `16003-elements` | 2000 copies of `val x=1\n` | 16,003 / 12,000 / 0 |
| `unicode-lf` | `val ` followed by backtick `café🦀`, `=1\n` | 11 / 6 / 0 |
| `256-diagnostics` | 256 copies of `?\n` | 771 / 512 / 256 |
| `healthy-followup` | `val healthy=1\n` after all preceding requests | 11 / 6 / 0 |

Derivation: Source mode emits a KT_FILE plus empty PACKAGE_DIRECTIVE and
IMPORT_LIST. Whitespace adds one token. Each property line adds PROPERTY, six
tokens (val/space/name/equals/literal/newline), and INTEGER_CONSTANT: eight total.
Each malformed line adds ERROR_ELEMENT containing QUEST, then a root newline:
three elements, two tokens and one diagnostic. These are resource controls, not
promotion into the declaration acceptance corpus or semantic validity claims.

For these LF-only sources, `normalize` changes no byte; its map is identity at
every byte offset, including Unicode interiors (accepted spans remain boundaries).
Parent `validate` remaps only element/diagnostic start/end through this map; every
other serialized kind/parent/subtree_end/token/message field is preserved.
`parse_snapshot` changes only the retained source Arc. Reconstructing `Reply::Tree`
by cloning all accepted fields and using the same pinned compact serializer thus
preserves the reply representation for these successful LF requests. No source
field enters the reconstructed reply. This is **reconstructed serialization**,
not stdout capture or an independent observation of transport bytes. CRLF mapping
is not identity and cannot support this equivalence argument.

The tests check reconstructed length under the universal source bound and4MiB;
they do not force an unreachable cap+1 production response or infer universality
from samples. Parser failures/timeouts are failures, with no retries or alternate
success outcomes. This adds no child process, supervisor, worker instrumentation
or external/native-forbidden dependency beyond the existing public parser calls.

## B3/B4 limits and recorded evidence

The unchanged supervisor starts its500ms wall-clock deadline before spawn and
polls every2ms, checking cancellation/deadline before output and exit status.
Timeout can reflect startup, I/O, parsing, serialization or scheduling. Once child
exit is observed, polling ends before writer/reader joins and possible
`receiver.recv`; these have no renewed deadline/cancellation checks. Cleanup and
validation are also outside the interval. No hard end-to-end or CPU-time bound is
established. Computation-attributable exhaustion remains missing; phase entry
alone would not distinguish CPU work from descheduling. This is an attribution
limit, not a new CPU-time acceptance requirement. B11 grammar EOF is separate.

The worker allocator charges cumulative requested bytes against67,108,864,
including startup and serialization and full new realloc sizes, with no refunds.
Denial returns null; infallible allocation normally aborts. Stderr is discarded and
failed status becomes WorkerFailed unless the deadline wins first. No current
public result identifies denied charge versus system OOM/panic/stack/deadline.
The private accounting test and unit-test allocation-denial child do not establish
production parsing exhaustion. No fixed input/tree bound proves churn unreachable.
Attributable production denial remains missing; this child adds no instrumentation.

Historical frozen [Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38289885940/job/114947148613)
checks out M and passes all three unchanged pinned commands:192 active tests,
two exercised helpers and19 separately counted protocol fault cases. Its
backtracking test permits success/WorkerFailed/Timeout plus healthy followups;
it does not identify a boundary cause. Its production count/diagnostic crossings
and fault-worker cap+1 tests retain their narrower meanings. This historical run
does not execute the new test. B3/B4 remain GAP even after this child's finite
acceptance passes; #83/#7 remain OPEN. There is no RSS/total-work/global performance,
Darwin correction, grammar-recovery, oracle or semantic-parity claim.
