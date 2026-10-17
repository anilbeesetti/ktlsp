# Native server providers

`ktlsp` supports bounded stdio lifecycle, document synchronization, folding, one negotiated authoritative structural quickfix, negotiated lexical semantic-token full/range transport, parser-backed syntactic symbols, finite local definition/hover/references/highlights, eager local-value completion, and guarded local-val prepare/rename. Initialization advertises only the implemented negotiated providers.

Parser-backed capabilities require the trusted matching worker. Hover also requires a compatible content format; rename requires versioned `documentChanges`, with preparation advertised only when supported by the client. Lexical tokens need compatible token capabilities but no worker. The structural quickfix also needs no worker, but requires code-action literal support and versioned `documentChanges`. Clients advertising `textDocument.diagnostic` negotiate lexical/structural pull reports instead of legacy push diagnostics. Neither diagnostics, structural quickfix nor lexical token categories provide Kotlin compiler analysis or full semantic coloring.

## Build and distribute both executables

```sh
cargo build --locked --release -p ktlsp-server -p ktlsp-parser --bins
./target/release/ktlsp
```

Install **both** `ktlsp` and `ktlsp-parser-worker` from that same build into the same trusted directory (with `.exe` suffixes on Windows). The default CLI discovers the worker only beside its own executable, checking that the sibling is a regular executable file on Unix. There is no PATH, current-directory or environment-selected worker fallback. Installation trust and matching build identity are the distributor's responsibility; this is not a sandbox, signature check or version-authentication protocol.

Without that sibling, synchronization/folding/lexical diagnostics, compatible lexical semantic tokens and the negotiated structural quickfix remain available, `documentSymbolProvider`/`definitionProvider`/`hoverProvider`/`referencesProvider`/`documentHighlightProvider`/`completionProvider`/`renameProvider` are omitted and their requests return MethodNotFound (`-32601`), never empty success. Library callers retain parser-free `Session::new()`/`Default`, or explicitly use `Session::with_parser(trusted_absolute_worker_path)`, which rejects a missing/non-file/non-executable Unix path. A configured worker that subsequently fails is a request failure, not a fabricated outline or binding. No JVM/JDK, Kotlin compiler or C parser is required.

## Optional extracted-archive smoke (#373)

The existing manual experimental packaging workflow keeps the full unfiltered
locked **release workspace tests first**, then checks the just-created TAR's outer
SHA-256 before creating a fresh extraction directory. After extraction it verifies
every inner checksum, exact file inventory, provenance, admitted source/tree,
workflow bytes, lockfile and original run/attempt identities **before execution**.
Only then does the separately identified ignored Rust integration test run:

```sh
KTLSP_EXTRACTED_PACKAGE_DIR=/absolute/verified/extracted/package \
  cargo +1.99.0 test --locked --release -p ktlsp-server --test document_symbols \
  extracted_archive_smoke -- --ignored --exact --nocapture
```

An absent/invalid directory fails this explicit invocation; it is never a
success-shaped skip. Default native tests do not require an archive or JDK and
do **not** exercise this ignored test. The smoke runs the actual extracted server
`--help`, `--version` and matching sibling-worker stdio with empty PATH and
JAVA_HOME/JDK_HOME removed. It reuses the ordinary-parameter/local-shadow fixture,
original string/u64 request IDs, CRLF/non-BMP UTF-16 ranges, definition/hover,
references/highlights and full empty pull diagnostics without duplicate pushes.
All processes must exit successfully with empty stderr. The borrowing exchange
helper never deletes the extraction directory; only exclusively created `Package`
test directories have deletion ownership, and retained extraction is asserted.

The permanent bounded decoded wire/CLI/provenance receipt is
`extracted-smoke.json` in the **separate evidence ZIP**, alongside verification
facts, complete inner checksums and the captured optional-test log. It is not
inserted into its own TAR. Failure evidence remains always-uploaded; the package
is uploaded only after the extracted smoke succeeds. Source and compile checks
are not runtime proof. The first separately admitted [original experiment38911945117](https://github.com/anilbeesetti/ktlsp/issues/373#issuecomment-6218039840), attempt1 at verified main `f5f8a409dea209d44f9a9abe11c0becdf20f658e`, actually exercised the extracted Linux/macOS pairs after full unfiltered release workspace tests. Independent platform owners and ROOT authenticated all archive bytes and directly read the 16-reply receipts with successful CLI/stdio, empty PATH/no Java-home variables and retained directories. These are finite observations at that source, not the current source's archive acceptance.

Windows failed that first experiment's full workspace before staging: an actual definition reply was `-32803`, `parser deadline exceeded`, not `result:null`; no Windows package or extracted receipt exists for that run. The failure's phase/root cause remains unestablished, not repaired by later success. Every later manual dispatch still requires separate ROOT admission after reviewed source and exact-main verification.

At later independently verified main `040dbba580d908c649c7256405687e2044e4cec2`, separately admitted original [run38926477362](https://github.com/anilbeesetti/ktlsp/issues/373#issuecomment-6219465863), attempt1, actually exercised the extracted Linux/macOS/Windows pairs after full unfiltered release workspace checks. ROOT and independent platform owners authenticated all six original ZIPs, three outer TAR/sidecars, complete inner inventories/checksums/notices/provenance and native ELF/Mach-O/PE headers. Actual empty-PATH/no-Java CLI/stdio succeeded with retained borrowed directories; all three 19-message/16-reply wire payloads are byte-identical. Receipts remain separate from their TARs. #373's concrete extracted-runtime gap is closed, not the historical Windows timeout/#377 or general platform/official/release gates. Artifacts remain experimental14-day evidence; #96/B3/#46/first-party licensing/full parity remain open.

## Folding and diagnostics

Folding recognizes matched multiline `{}`, `()`, `[]`, block-comment tokens, raw-string tokens and groups of consecutive whole-line `//` comments. Comment groups need at least two adjacent physical lines with only space/tab/form-feed indentation before `//`; blank lines, code, inline comments and opaque tokens separate groups. Their ranges start at the first slash and end after the last comment's content, including trailing spaces but excluding its terminator. LF, CRLF, lone CR and EOF are supported. Strings/templates, characters, backtick identifiers and comments remain opaque to delimiter matching. The complete opaque raw-string/comment token may fold; nested template expressions/comment interiors do not create outer-file folds. Single-line ranges are omitted. Unterminated multiline comments/raw strings can fold to their token end while their lexical diagnostic remains visible. Imports, declarations and semantic structure are outside this declared subset.

On a mismatched closer, report it and discard/diagnose all pending openers. This conservative recovery cannot create crossing delimiter folds; complete valid pairs encountered later can still fold. Unmatched closers and remaining openers produce structural diagnostics. For legacy clients, accepted open/change notifications publish replacement diagnostics (including empty arrays); close clears them. Negotiated pull clients receive no duplicate push notifications. Rejected/stale/invalid edits preserve the current document. Push diagnostics include document versions only when `publishDiagnostics.versionSupport` is true; close clearing omits version.

Missing/closed-document folding requests return invalid params (`-32602`), without reading disk. Malformed request params/client folding capabilities also return invalid params. Analysis exhaustion leaves an accepted document version intact, replaces stale diagnostics with one explicit `analysis-limit` warning, and fails folding with RequestFailed (`-32803`). Subsequent edits can recover normally.

## Authoritative trailing-closer quickfix (#391)

With valid `textDocument.codeAction.codeActionLiteralSupport` and
`workspace.workspaceEdit.documentChanges: true`, initialization advertises
`codeActionProvider: {codeActionKinds: ["quickfix"], resolveProvider: false}`.
The client's kind value set is advisory, not an eligibility whitelist.
Absent negotiation omits the provider and returns MethodNotFound (`-32601`).
There is no Command fallback, resolve, applyEdit, fix-all or import action.

`textDocument/codeAction` reads the same current-document raw Analysis cache as
folding and lexical diagnostics. It offers one eager deletion only when the
complete analysis has exactly one unmatched-closing-delimiter diagnostic,
that original token is `)`, `]` or `}`, and only existing lexer-whitespace tokens
follow it. The final non-whitespace endpoint is retained in the existing token
sweep: no second lexer, parser, cache, source copy or pending job is added.
Mismatch recovery that also diagnosed pending openers, lexical/multiple/suppressed
diagnostics, non-whitespace suffixes and version `i32::MAX` cannot authorize it.
This finite lexical policy is not a compiler repair or official action equivalence.

Request URI/context/diagnostic shapes and both current UTF-16 range endpoints
are validated. Missing/closed documents, reversed/out-of-bounds/surrogate-interior
ranges and malformed parameters return `-32602`; analysis exhaustion is `-32803`,
never successful empty output. Client diagnostics are not edit authority:
empty or stale supplied diagnostics cannot hide or manufacture a current candidate.
Nonempty ranges must half-open-overlap the token; an empty caret may touch either
token boundary. Absent `context.only`, `"quickfix"` or its empty ancestor permits;
`[]`, unrelated kinds or child-only kinds exclude. Both valid trigger kinds are
accepted without broadening eligibility.

The action contains one `WorkspaceEdit.documentChanges` edit at the exact open
URI/current integer version, deleting only that one original ASCII token.
Nothing is applied by returning it. Accepted `didChange` commits and invalidates
analysis; rejected/stale changes retain current state, and close/reopen invalidates
the cache. As with rename, an already delivered URI/version literal retained by
a client across an identical-version reopen has no stronger incarnation guarantee.
Provider and real-stdio regressions apply the actual returned edit, then require
the exact version-8 text, fresh empty diagnostics and no remaining action.
At reviewed sourcee24c011, original platform38941146673 actually emitted the
complete8-message/6-reply exchange on both hosts. ROOT and the independent owner
read all packets and independently applied the received1:0–1:1 version-7 edit.
The actual sent incremental change omitted absent `rangeLength`, committed
version8, and produced fresh empty diagnostics followed by no further action.
Corresponding observation packets match across hosts:2707bytes, SHA256
564b94d9bfe250206a4ef4e45bbbe756cd60e0c53d640dc254059e105ec18790.
Successful exit/empty stderr/empty PATH/no Java homes are exercised source
assertions, not separate fields in that packet. No parser-backed request appears;
an available worker is not ruled out. Only temporary writers/bookkeeping were
removed after readback; cleanup-source CI/integration remain separate gates.


## Lexical semantic-token transport (#380)

`textDocument/semanticTokens/full` and `/range` use the bounded native lossless
lexer, without a parser worker. This is standard relative semantic-token
**transport over lexical categories**, not official semantic coloring or full
Kotlin LSP parity. Only positively known keyword, comment (including shebang),
string (including raw strings/characters), number and operator tokens are
emitted. Identifiers/backtick identifiers, punctuation and unknown tokens are
omitted: no type/function/variable identity or modifiers are guessed. Templates
remain opaque strings, including their interpolation expressions; parser-backed
resolution is required to go beyond this ceiling.

Initialization requires valid `textDocument.semanticTokens` string arrays for
`tokenTypes`, `tokenModifiers` and `formats`, including `relative`, and at least
one supported full/range request. The fixed native order
`keyword, comment, string, number, operator` is intersected with client-supported
types; an empty intersection or malformed/incompatible profile disables only
this feature. The advertised legend contains that intersection and no modifiers.
Full and range are independently gated (`true` or an object enables each);
client delta support never enables server delta support. Unnegotiated methods
and `/full/delta` return MethodNotFound (`-32601`). No delta, `resultId`, token
cache, identifier resolution, background job or JVM dependency is added.

Each synchronous query reads the current immutable open document, lexes once
and sweeps source/UTF-16 positions once. Multiline tokens become positive-length,
nonoverlapping single-physical-line fragments, excluding CR/LF; CRLF, lone CR,
LF and astral characters retain exact UTF-16 coordinates. Range endpoints are
strict, half-open document positions, with partial-token clipping. The first
returned token is relative to document `(0,0)`, not the requested range start.
Empty documents/ranges and files with no supported categories return `data: []`.
Malformed/reversed/out-of-range/split-surrogate ranges and unopened URIs return
InvalidParams (`-32602`), never clamped output.

Existing 8 MiB source, 1,000,000 lexer-token, 256 lexical-diagnostic and 128
scanner-context limits remain. At most **10,000 emitted fragments** and a
conservative **1 MiB encoded result** are admitted. Five maximum-width u32
integers plus separators cost at most 55 bytes per fragment plus 11 bytes for
the result object, so the fragment bound is tighter than the byte bound.
Checks precede appending; exhaustion rejects the whole query with RequestFailed
(`-32803`), never truncates or substitutes empty success. Range queries count
only emitted/clipped fragments but still lex and validate the complete source.
Rejected document edits retain the prior version and token result.

Permanent provider regressions decode real lexer output, negotiate independent
request/type profiles, cover strict UTF-16/mixed terminators/opaque templates,
edit rollback and exact resource boundaries. The real CLI regression decodes
full/range results and an applied edit with exact string/u64 IDs and pull-mode
diagnostics. Original hosted execution is recorded below; no local server/parser
runtime is authorized under #96. Final cleanup-source CI and guarded integration
remain separate gates; full official parity remains open.

## Eager finite local completion (#382)

With the trusted matching worker, `textDocument/completion` advertises only
`completionProvider: {resolveProvider: false}`, independently of hover format
negotiation. Missing workers omit that capability and return `-32601`.
Completion uses the existing eight-job asynchronous parser pipeline, admission,
cancellation, cleanup and exact incarnation/version/content publication guards;
there is no new parser, source repair, source copy, pool or cache.

The admitted site is a **nonempty existing whole-initializer identifier** in a
complete ordinary named top-level block function. Bare and complete backtick
tokens accept a caret inside or at their end; the original token start or just
after its opening backtick permits an empty prefix. No initializer hole,
unfinished quote, standalone expression, member/call/type/global context or
damaged body is invented. These unsupported contexts return `-32803`, not an
empty list. Strict invalid/split UTF-16 and unopened-document positions return
`-32602`; navigation's existing half-open endpoints are unchanged.

Exactly one `visible_local_values` query authenticates the original reference
and same-function parameters/preceding immutable locals under its shared
2,097,152-work and 1,024-value/131,072-byte storage ceilings. Local shadowing,
parameter fallback in self-initializers, sibling isolation and visible-name
ambiguity (including later duplicates) retain the producer API's atomic policy.
Only the original token text before the caret is matched, case-sensitively and
without normalization, against semantic spelling; opening/closing backticks are
excluded separately. Output remains in original declaration order.

The eager `CompletionList` has `isIncomplete: true`: further typing recomputes
and replaces this finite list, not full Kotlin candidate completeness or
official relevance ranking. A successfully authenticated universe with no
prefix matches may return `items: []`. Each item has the original raw `label`,
semantic `filterText`, Variable kind (`6`, including `val`) and a plain
single-line `textEdit` replacing the **entire original token** with the target's
raw spelling, including its original quotes. One UTF-16 sweep computes the
shared replacement range. Before repeated JSON allocation, both rendering
passes are bounded by a 131,072-byte spelling/prefix-work ceiling and a
conservative 1 MiB escaped-output ceiling; exhaustion refuses the whole list.

Real-worker regressions apply returned edits in memory and query the actual
resulting source's definition, including suffix/quote replacement, CRLF,
astral names, empty prefixes, parameter/local shadowing and atomic refusals.
Paired CLI coverage also retains exact string/u64 IDs, missing-worker and
hover-independent behavior; controlled failing peers cover cancellation and
edit/close/reopen guards, never fabricated successful CST. ROOT and the owner
independently read the original hosted wire, applied source and actual rebinding
described below. No local server/parser runtime was run.

No completion resolve, imports, snippets, commands, item data, dot triggers,
type inference, damaged-syntax recovery or full/official completion parity is
implemented or advertised. Type annotations remain retained syntax only.

## Original hosted completion, rename and token proof

At source `95bebd7762be59a6420a9942069d7113996bdf55`, original attempt1
[Native38920539902](https://github.com/anilbeesetti/ktlsp/actions/runs/38920539902)
and [platform38920540091](https://github.com/anilbeesetti/ktlsp/actions/runs/38920540091)
passed pinned format, strict locked workspace/all-target Clippy and full workspace
tests. Actual checkout `5b8e59624be2853dece9d9015d2d385a73a01857` has tree
`a3140441914bbd998e0ab98af1f7de805f3a6bae` and ordered parents
`[f5f8a409dea209d44f9a9abe11c0becdf20f658e,95bebd7762be59a6420a9942069d7113996bdf55]`.

ROOT directly read all three actual payloads; macOS/Windows payloads agree exactly.
Returned version7 rename edits replace only the original local name and bound use,
preserving annotation/comment/sibling text; the actually reopened version8 source
returns only the renamed declaration/use and its original definition. Completion
replaces the complete original `` `couSuffix` `` token with `` `count😀` `` despite
an inside caret, and the resulting real CLI definition returns parameter1:6–1:15.
The exact u64 ID, CRLF/astral ranges, hover-independent capability and missing-worker
refusals remain intact. ROOT independently applied both captured edit sets.

ROOT independently decoded every actual relative full/range/edited token tuple:
mixed CRLF/CR comment fragments clip correctly, and the committed edit changes the
number into a four-UTF-16-unit astral string. Pull diagnostics return full `[]`
without duplicate pushes. Linux additionally emitted matching rename/token payloads;
completion's captured libtest output is available from the two platform jobs.
Temporary writers were removed only after readback; behavioral assertions remain.
This is finite source-specific native execution, not official equivalence, Windows
packaging acceptance or an explanation/repair of #377. Final cleanup-source CI,
guarded integration and independent original exact-main proof remain mandatory.

## Proved lexical local-call definition (#389)

Definition on an authenticated direct standalone empty-call callee can return
its unique original preceding same-block local `FUN` name. The local function
must have a complete ordinary nongeneric zero-parameter header and empty body.
Every call in the admitted outer function must be proved by the same rules;
unknown invokable values or skipped bodies cannot silently lose value occurrences.
The nearest proved applicable local function precedes unknown package/import
candidates; no project-wide closed-world or zero-arity overload guess is made.

Supported mixed bodies retain ordinary parameters, immutable locals, these
empty local declarations/calls and restricted generic outer headers.
Existing value definition/hover/references/highlights/completion/rename and
integer-default queries keep original value/shadow partitions; annotation
definitions retain their original lexical typeparameter targets. Local functions
and call callees do not become value completion/rename/occurrence candidates.
Fresh value rename preserves call identity; a relevant local-function new-name
conflict, including a conservative future declaration conflict, refuses.

All origin/header/body/call/competitor work shares the existing semantic budget;
no public-query reset, recursive body validation or copied declaration is used.
Before-declaration calls, competing semantic names, richer arguments/type/lambda
or qualified/postfix shapes, nonempty local bodies and unsupported local headers
fail atomically rather than pretending globally unresolved. This adds callee
definition, not callee hover/references, function rename, general overload/invoke,
signature help or official equivalence; declaration-name source hover is unchanged.
ROOT and the source owner directly read all three original producer/target
records on both platform hosts:39/141/39 UTF8bytes,33/107/33 CSTelements,
empty diagnostics and exact original callee/target/body/owner relationships.
Actual minimal and mixed CLI exchanges retain6/3 and20/17 message/reply counts;
all eight value consumers, generic targets, function-name conflict and callee
refusals are distinct. Independently applied returned completion/rename edits
match actual version8 reopened sources and preserve function/typeparameter
targets. Sourcee24c011 passed original full Native38941146516 and platform
38941146673; the earlier failed bad67 run remains immutable. Temporary writers
were removed afterward; changed completion/rename rebind assertions remain.

## Finite function type-parameter references and rename (#392)

References/highlights and negotiated prepare/versioned rename now positively
select an original function type-parameter declaration (including unused names)
or a supported simple parameter/property/function-return annotation in a recursive ordinary-function tree.
Initializer, callee and value-declaration roles remain separate. Type hover/typeDefinition
and richer types remain unsupported; no new capability is advertised for them.

The kernel authenticates the complete top-level ordinary-function root and every
ordinary nested function, including uncalled descendants, under one existing
aggregate work/storage budget. Each function may own the restricted leading
unbounded/unmodified type list. Annotation lookup searches own declarations then
enclosing functions nearest-first; sibling scopes and other roots are not fallback
candidates. Partitions retain exact original declaration identity, not spelling.
Each ordinary block function may have one direct simple explicit return after
its parameter list. Return lookup starts at that function itself; inherited
returns keep the enclosing target-declaring function/body identity. The original
Function is the annotated declaration, with header membership outside its own
body. Every direct return, including a root return, enters the once-only ordered
partition and the whole-root hypothetical capture/unmatched-preservation proof.
Rename hypothetically relooks up every annotation: edited uses keep their original
target and unedited bound/unmatched annotations keep their identity/status.
Duplicate own-list names, descendant capture in either direction and capture by
unused binders refuse; safe sibling shadowing does not falsely conflict.
Values, functions/callees and labels remain separate namespaces. Existing value
and callable providers keep their stricter independent body/header contracts.
Bare `where`, `dynamic` and `suspend` outputs still refuse context changes before
same-name quote removal; value-rename name admission is unchanged.

Rendering reuses sorted original UTF-16 coordinates, declaration-Write and
annotation-Read highlights, exact selected-token preparation and a single
current-URI/integer-version `documentChanges` edit. Existing 64 KiB edited-source,
1 MiB escaped-output, negotiation, eight-slot admission, cancellation and
incarnation/version/content publication guards are unchanged. Permanent real
producer/paired-CLI regressions apply returned edits with native `TextChange`
helpers, reopen version8, and rebind original type/value/callee partitions,
including contextual `out`/`reified` outputs. First original PR413 source694f2f9/
treee0d3 passed full Native38998942941 and platform38998942890, both attempt1.
Independent OWNER+ROOT fully read all44 actual producer/CLI packets PER host and
applied all25 genuine returned version7 batches/135edits per host through
complete actual version8 source/producer/definition/reference/unmatched/shadow
replay. Recorded repeated-response applications531/3017 per host are not distinct
transactions. Only afterward were all four temporary writers/sole JSON captures
removed. Permanent actual edit/reparse/rebinding assertions and the CLI helper's
diagnostics/lowering/region checks remain at their original frequency/placement.
PR413 cleaned1369/treea41d received renewed reviews, OWN original full three-host
CI and guarded integration as4bbf. Independent OWNER+ROOT accepted original
exact-main39001476453 before lease clearance. No local Kotlin runtime ran.
This is not workspace refactoring, generic inference, official equivalence or
full parity; #96/#377/#395 stay open.

#414's first original PR416 source64de24/tree98c67178 now has actual complete
return-CST/paired-CLI evidence across Linux/macOS/Windows. Independent OWNER+ROOT
FULLread all21 packets PER host and applied all37 genuine original version7
batches/261 edits per host through complete version8 source/declaration/
parameter/property/return identity replay, preserving unmatched/unused/unedited/
sibling partitions, strict CRLF/astral coordinates and contextual out/reified.
Twelve duplicate selected-response fields/41 edits per host are additional
custody, NOT new transactions. ONLY AFTER both gates ROOT removed temporary
writers/sole observation JSON; permanent actual edit/reparse/query/assertion
flows retain their original frequency/placement, including target-pointer checks.
First-source Windows full CI separately failed the unchanged long-fence docless
hover assertion; its complete original response was not retained, so cause is
UNKNOWN under #417. Added complete-response assertion context changes no input,
acceptance predicate, cap or deadline and is NOT a behavioral fix or retry.
Cleaned-source reviews/OWN full CI/guarded exact-main integration remain required.

## Finite local definition and declaration hover

Over open Kotlin documents, a strict UTF-16 position selects an actual original value `REFERENCE_EXPRESSION` containing that position (end-exclusive). The existing declaration lowerer, structural-region directory, authenticated `ReferenceSite` and `bind_local_value` prove a unique eligible preceding direct immutable local `val` or same-function ordinary parameter. Only ordinary named top-level block functions with complete plain integer/whole-identifier local initializers and simple syntax-only parameter/local annotations are eligible. Definition returns the original target **name** range and same URI. Hover returns the original target **declaration text** (`p: T` for a parameter) and original reference range: no invented types, initializer chasing or inferred global unresolvedness.

Definition additionally accepts an authenticated simple parameter/local/function-return annotation
inside the admitted recursive ordinary-function tree. The annotation-only lexical
query returns the nearest matching original function `TYPE_PARAMETER` name range
and URI, preserving the actual annotated owner separately from the target owner.
Multiple parameters, supported bare/backtick/Unicode spellings, trivia and a real
trailing comma retain original identity; duplicate semantic names in any own list
refuse even when unused. Outer classes, imports/aliases, siblings, other roots
and values are not candidates or fallbacks. Bounds/variance/reified/annotations,
where clauses, malformed/empty/post-name/second lists, receivers/context,
expression bodies, member headers, richer annotation syntax
and unsupported syntax anywhere in the root remain refused. Bare `T` does not
imply semantic non-nullability. Recursive depth follows existing structural
ceilings, not a fixed function-nesting lane.
All eight existing parser-backed consumers retain their original value/body
contracts on this generic body. Type parameters stay out of value completion,
occurrences and rename edits; annotation references stay outside value partitions.
The original #386 delivery added no typeDefinition method/capability, type-parameter
hover/references/rename, generic inference or full Kotlin type resolution.
Its hosted producer/paired-CLI behavior is recorded below; #392's separate finite
reference/rename extension is described above. Capture-free reviewed b5f6128
passed final original native/platform CI, then guarded PR388 produced c4c7c6d,
exact reviewed treeba361/soleparent040. Independent OWNER+ROOT authenticated
original exact-main38933481416/job117083027573:1052passed+22faultcases.
These finite proofs do not establish full parity or unknown official runtime behavior.

At first source `1f118a4efe990f044759e1162bcd2b0361df4991`, original attempt1
Native38930044366/job117071397551 passed1052+22; platform38930044300
Windows117071396385 passed997+22 and macOS117071396733 passed1050+22.
Each full census has131 rows/0failed/3ignored/0filtered. Actual checkout
4b34cec4a99bdc4de9963b8413a680e3ec86feb8 has tree
2591f559118236a038ab158a59c51b07644a95b6 and ordered parents
`[040dbba580d908c649c7256405687e2044e4cec2,1f118a4efe990f044759e1162bcd2b0361df4991]`.

ROOT and the source owner independently read every original four-packet producer
set and the complete actual CLI payload on both platform hosts. Actual annotation
targets are T3:5–6, quoted-U3:8–13 and isolated g-T7:5–6; ordinary p/x still target
their original values. The main23-message/20-reply exchange exercises all eight
consumers. Returned completion and version7 local-x→T rename edits independently
reconstruct the actual version8 reopened sources; both real followups retain the
original generic-T annotation target. Type parameters do not enter value edits.
Original raw CLI payload13563bytes SHA256
c36709b823583840c882e02639a18c5fd6c44329ed16b11d489d18ebd0a1ce34
is identical across hosts. Linux's eprintln captures are not observed.
Only temporary writers/bookkeeping were removed; all semantic, edit/rebinding
and refusal assertions remain. This is finite native behavior, not official
generic semantics, an extracted current-source archive or a #377 repair.

Hover also accepts the unique original half-open **name-token** range of a complete lowered `CLASS`, `FUN`, `PROPERTY` or ordinary `VALUE_PARAMETER`, returning that declaration's exact original source and its name range through the same negotiated renderer. Same-spelled declarations at different positions remain distinct, including parameter/local shadows; no binder or inferred type identity is needed for a declaration's own name. Definition on those names remains null. Keywords, whitespace, braces, name ends and EOF remain null; incomplete selected declarations and unsupported whole-tree lowering fail explicitly, without syntactic-symbol fallback. Unsupported reference lookup never falls back to an enclosing declaration. This is declaration-source hover, not KDoc, type resolution or full official hover parity.

At first changed-diagnostic union source `2c940228bb4af10f76020503ce007dbd1cfa8b44`, [original platform run38907884039](https://github.com/anilbeesetti/ktlsp/actions/runs/38907884039), attempt 1, passed pinned format, strict locked Clippy and full release workspace tests on macOS/job116995986538 (**992 standard/doctest passes**) and Windows/job116995986522 (**939 passes**), each with zero failures, three ignored tests and 22 separately reported worker protocol fault cases. Actual checkout `0d1c4c35e5351a738e95d39ea209d4359acb2dc1` has ordered parents `[594cc51cb4f1994b5e4e896e3a3c171b0ff3bcf4,2c940228bb4af10f76020503ce007dbd1cfa8b44]` and tree `9ac8b5af275e357ba81153bb633651cc4b078823`. ROOT and the hover owner independently authenticated original API/checkout identities and directly read all six captured default/Markdown/plaintext paired-CLI payloads. Both hosts returned seven exact original IDs once each, including integer `18446744073709551615`, with no unsolicited pushes. Declaration-name ranges were class `1:6..1:10`, property `4:3..4:11`, function `5:4..5:8`, parameter `5:9..5:17` and local shadow `6:5..6:13`; contents preserved original CRLF/non-BMP source and the class's bound leading comment. Markdown preference skipped the unknown format and fenced the hostile four-backtick property comment with five backticks; plaintext-first and absent-format profiles returned plaintext.

Only the observed temporary capture writer was removed afterward; all behavioral assertions and production logic remain. This receipt describes that first union source, not execution of the cleanup head or main, an extracted release archive, official parity or a fix for #377. The earlier #377 failures were not reproduced or explained by the changed diagnostic. No local parser/server runtime was run under #96; final cleanup/integration reviews and CI remain separate.

Non-reference positions can return null. Unsupported contexts/names, queried-name duplicates, self/forward references without an eligible parameter fallback, and no eligible target return RequestFailed (`-32803`), not a claim that a symbol is globally unresolved. Malformed/out-of-range/surrogate-interior positions or unopened documents return InvalidParams (`-32602`). No imports, cross-file/project/member/full Kotlin resolution, type inference, full navigation or official parity is claimed.

`textDocument.hover.contentFormat` selects the first recognized client preference (`plaintext` or `markdown`); absent lists default to plaintext. Empty, malformed or wholly incompatible lists disable hover without breaking initialize or other providers. Markdown uses a Kotlin code fence longer than every backtick run in the source declaration, so untrusted source cannot close it; plaintext is emitted only when negotiated/defaulted. Source and response remain bounded by the existing parser input ceiling.

**Source-owned prose KDoc (#397).** The declaration-name lane additionally accepts
one producer-attached, directly owned leading `DOC_COMMENT` on a complete supported
declaration. It uses the original parse and caller-owned declaration lowering;
neither nearby comments nor enclosing/body/annotation/modifier names establish
ownership. The finite prefix admits plain modifier keywords only with an attached
supported comment; annotations and unsupported prefixes remain refused. Blank
lines and intervening ordinary comments do not replace producer ownership.

The accepted default section contains only plain text, whitespace and producer
parenthesis leaves, with original comment delimiters/leading asterisk decoration
removed. All other content whitespace, paragraphs, tabs, Unicode and LF/CRLF/lone-CR
line endings remain literal. Tags, bracketed links/images/reference destinations,
backtick/code forms, tilde fences, malformed or ambiguous structures refuse the
whole hover, never fall back to raw KDoc or an enclosing declaration. This is not
resolved KDoc links, inherited/tag-derived documentation or official hover parity.

Actual prose appears before the declaration's exact original
`documentation.end..declaration.end` suffix, separated by two newlines, so the
comment is not duplicated. Plaintext is literal; Markdown backslash-escapes every
ASCII punctuation character in prose, including HTML/entity/URI syntax. The source
suffix retains the existing dynamically sized Kotlin fence. No source-authored
destination is activated or manufactured. The hover range remains the exact
original name token converted to UTF-16, not a decoded-prose coordinate.

Only positive original-name/direct-comment eligibility enters this operation.
Docless names still use the exact old whole-tree lowerer and renderer, including
its acceptance, prefix refusals and long-backtick fences. Consequently a docless
name in a file containing a different documented declaration can retain the old
whole-tree `-32803` refusal; this feature does not widen that old grammar.
Documented value/type/callee references also remain refused by the old binding
proofs; this is not use-site documentation lookup.

One opaque parser documentation budget spans original authentication/selection,
the real lowerer's count-and-emit work, prefix/comment validation and prose decode,
then escaping, fence sizing, the shared UTF-16 sweep and output. Managed
source-proportional strings reserve fallibly once, charging requested storage and
actual retained capacity; failures refuse atomically without truncation. The
existing 2,097,152 work and 131,072 requested/retained-byte ceilings are not reset
between stages. Complete successful documentation response/wire size has a
conservative 1 MiB ceiling, including JSON string escaping, fixed keys/range digits,
framing and the real request-id upper bound; no serialization copy is made merely
to count. Owned strings move into the inherited fixed-shape JSON response.
Fixed-count serde_json map/key/envelope allocations and the existing final wire
serializer retain their ordinary infallible-allocation/OOM behavior; managed
String-capacity accounting is not full transport fallibility or an RSS guarantee.
The same borrowed cancellation observer is checked throughout substantial work,
without an executor, new timer or renewed parser deadline. Publication still
rechecks URI, version, exact bytes, incarnation and cancellation.

Permanent cases include exact literal/escaped output and dynamic fences, complete
JSON/id and retained-growth boundaries, mid-operation cancellation, paired
packaged CLI negotiation/refusals and actual UTF-16 doc/name/prefix edits,
move/removal and close/reopen ownership transitions. TEMP397 packet writers retain
full original requests/responses pending ROOT/OWNER hosted inspection. These are
source-authored checks, not an execution receipt: no local parser/server/Session,
build, test, lint or formatter was run for this change. ROOT-hosted composed-source
verification remains required; #96/#377/#395 and full-parity limits remain open.


All eight parser-backed methods share **one** bounded job pipeline, parser instance, admission limits, cancellation/tombstones, cleanup ownership and incarnation/version/content publication guard. There is no second validator/pool, parse cache or extra source copy. #350 tests exercise actual paired stdio replies and controlled-peer cancellation separately; source/review/hosted receipts are not full navigation, oracle or platform proof. #96 remains open: no local parser/server runtime is authorized.

The [first original #351 Native run38866505501/job116860467519](https://github.com/anilbeesetti/ktlsp/actions/runs/38866505501/job/116860467519), attempt 1 at source `96b54047ee242e34a66f68020c79a47e620793eb`, checked merge `accd703dd2224301d1c8c44d1b2834a56a8b557f` into `bb6a75f61fd467c9b090210a369c7e1d225713d5`. Format and strict locked Clippy passed; workspace tests **failed** a new fixture's reply-count authoring error (six requests but expected seven). Actual failure wire retained the original definition `2:14–2:19`, plaintext declaration ``val `x😀` = 1`` and reference `2:33–2:38`, plus Combined/item outline and full empty pull without pushes. The other new real-worker semantic/refusal/hostile-fence/no-chasing cases and shared-wire cancellation/edit/reopen regression passed. The count was corrected from the six explicit source requests and independently source-rechecked by both reviewers; no unchanged retry or passing-full-run claim follows from this failure. Full negotiation and the direct-Write observation were not reached. Optional synthetic-tools [run38866505445](https://github.com/anilbeesetti/ktlsp/actions/runs/38866505445), attempt 1, passed both hosts; that is not official-oracle or platform proof.

After the source-derived exact-ID amendment and ROOT's reviewed pending-input backoff fix, [original Native run38872239153/job116878810352](https://github.com/anilbeesetti/ktlsp/actions/runs/38872239153/job/116878810352), attempt 1, passed pinned format, strict locked workspace/all-target Clippy and full locked workspace tests at source `41cc6848bfeebb1f744c489861ebf492dc0c181a`. Actual checkout `4f5fe7658f98b93269a5ee8cc734d6bc848409b0` has ordered parents `[bb6a75f61fd467c9b090210a369c7e1d225713d5, 41cc6848bfeebb1f744c489861ebf492dc0c181a]` and tree `ba48d8a65b74a9dfbffcdb030c98a0b60ef0a739`. All 12 actual-worker provider tests, all three controlled-peer cancellation tests and the parser-free pending-input regression passed. Optional synthetic-tools [run38872239112](https://github.com/anilbeesetti/ktlsp/actions/runs/38872239112), attempt 1, passed both hosts.

The same-build packaged stdio consumer, with empty PATH and Java variables removed, directly wrote actual decoded wire evidence: string ID `"definition"` targets `untitled:outline` name range `2:14–2:19`; exact integer ID `18446744073709551615` receives original declaration ``val `x😀` = 1`` as negotiated plaintext or Kotlin-fenced Markdown, with original reference range `2:33–2:38`. Permanent assertions cover capability/preference/incompatible/malformed profiles, missing-worker behavior, exact response-ID sets, Combined/item outline, mandatory UTF-16 and full pull without duplicate pushes. Separate real-worker tests cover CRLF/astral/backtick ranges, strict invalid positions, finite-binding refusals, hostile source fences and no initializer chasing. Controlled failing-peer tests prove shared cancellation and committed-edit/identical-reopen invalidation mechanics, not successful CST semantics. Only the now-observed temporary direct-Write writer was removed afterward; all behavioral assertions remain. Two independent source reviews and composition rechecks cleared the source. This is finite native Linux evidence, not full navigation/types/imports, fresh official-oracle comparison or general platform reliability; #96 remains open and no local parser/server runtime was run.

### Finite same-body references and highlights

References/highlights select an original lowered declaration-name token (including unused locals/ordinary parameters) or a whole-reference initializer CST-index anchor. Exactly one `local_value_occurrences` call authenticates the `LocalValueAnchor`, resolves reference selection and collects original bound sites under ONE shared semantic work/storage budget; no fresh preliminary `reference_site`/`bind_local_value` query escapes that budget. Parameter ownership identifies the same introduced body while retaining original header file membership. No textual name search, global lookup, initializer chasing, per-occurrence budget reset or second parser pipeline is used.

`textDocument/references` requires `context.includeDeclaration` to be a boolean. The result is same-document `Location[]`, in source order, optionally beginning with the original declaration-name range. An authenticated unused target returns `[]` when declaration inclusion is false. `textDocument/documentHighlight` returns the declaration as Write (`kind: 3`) and each bound occurrence as Read (`kind: 2`), once each, in source order. One existing UTF-16 sweep converts retained original endpoints, preserving CRLF and astral/backtick spelling.

Both providers are independent of hover content-format negotiation and require the same canonical worker availability. Nontoken positions may return null; unsupported declaration/use/parameter forms, ambiguous names, member/global lookup and self/forward use selection without an eligible parameter fallback fail explicitly with `-32803`, never a claim of complete global no-references. Invalid context/URI/position shapes, unopened documents and invalid or split UTF-16 return `-32602`. Source/job limits, stale edit/close/reopen rejection, exact string/u64 cancellation, EOF/shutdown/exit and cleanup acknowledgement are shared unchanged.

Output is bounded by the kernel's 1,024-site/storage ceiling and a conservative 1 MiB rendered-size ceiling (including repeated escaped URIs), failing the whole query rather than truncating. Only the admitted same function body is searched; identical names in other functions and unrelated duplicate names do not change the target. This is **not workspace/cross-file/full Kotlin references or highlight completeness**, general type resolution, or official parity.

The composed typed-local lowerer admits an optional single simple type-name annotation on these same plain local vals, including supported backtick names. Outside the lexical type-parameter lane above, this is retained declaration syntax only: hover preserves it exactly, type-name tokens are not value occurrences, and no builtin-name whitelist or global type identity is inferred. Explicitly typed targets refuse integer-default inference; an annotated containing val does not invalidate a different untyped target. Qualified/nullable/generic/annotated/parenthesized/function/intersection/dynamic types remain outside this bounded subset. The paired-stdio composition regression also uses a type-name token spelled exactly like the value target, checking that it never enters references or highlights.

The #364 permanent regressions assert real same-build packaged CLI payloads/IDs, declaration/use/unused anchors, includeDeclaration, highlight kinds/order, Unicode/CRLF and missing-worker behavior without duplicate pull-mode pushes. Additional real-worker cases assert semantic-equivalent quoted names, function isolation, no chasing, strict invalid input and atomic refusal. Shared controlled-peer tests cover each provider's exact cancellation and stale/lifecycle guards; those deliberately failing peers do not fabricate successful CST.


At source `84a2baffed1ee75f43627c7d74a5475fe5abcd4e`, [first original #366 Linux Native run38884683910/job116919452694](https://github.com/anilbeesetti/ktlsp/actions/runs/38884683910/job/116919452694), attempt 1, passed pinned format, strict locked workspace/all-target Clippy and **963 workspace tests plus 22 separate protocol fault cases**. Actual checkout `77978b9df59dd167c70b29b96715309aa4552a0c` has ordered parents `[abe98694226c86fced3aed31691b8f0945a565e0,84a2baffed1ee75f43627c7d74a5475fe5abcd4e]` and tree `f85a217bcc13bef5c27c37ba663f68c64b6cedd1`. All 16 matching-worker provider tests and three controlled-peer cancellation tests passed. The initial local compile-only Clippy caught `chunks_exact_to_as_chunks`; its zero-allocation correction received both affected source rechecks and the admitted changed-source format/Clippy gates passed before this first push. No local parser/worker/server runtime was run under #96.

Actual direct decoded paired CLI JSON advertised both providers despite incompatible hover formats. Declaration and either use selection returned the same original `untitled:outline` references: declaration UTF-16 `2:5–2:10`, reads `3:9–3:14` and `4:9–4:14`; includeDeclaration false omitted only the declaration. Exact integer ID `18446744073709551615` and string highlight IDs received Write (`3`) then Read (`2`, `2`) at those ranges, once each. Unused declaration references returned `[]` or original `5:5–5:11` according to inclusion, with one Write highlight. Existing definition and full empty pull remained correct without duplicate pushes. These are finite native Linux observations, not official-oracle/type/workspace completeness, composed typed-lane evidence or general platform reliability.

ROOT independently fetched and authenticated that original job's checkout/API identities and actual direct JSON before accepting the proof. Only the temporary observation writer was then removed; permanent payload/ID assertions remain. Both affected source reviews recheck this writer-only cleanup and receipt. No cleanup runtime retest or tested-cleanup-head claim is implied; final composed source/CI and integration remain ROOT-owned.

At composed source `507526e20fe37be13d8ac5fdf8c35b0db743031f`,
[original #368 Native run38887526600/job116928807682](https://github.com/anilbeesetti/ktlsp/actions/runs/38887526600/job/116928807682),
attempt 1, passed pinned format, strict locked workspace/all-target Clippy and
**967 workspace results plus 22 separate protocol fault cases**. Actual checkout
`02be84f46622477fdc56ae923a5bd5d1d4c4c437` has ordered parents
`[abe98694226c86fced3aed31691b8f0945a565e0,507526e20fe37be13d8ac5fdf8c35b0db743031f]`
and tree `96cd63b2b9d6fce9d24b75e0ed399bf1957a0bbb`.

ROOT independently authenticated the original API/log identities and actual
paired-stdio JSON. Typed definition selected original UTF-16 `3:5–3:10`;
plaintext hover retained ``val `x😀`: Long = 1`` at use `4:14–4:19` with exact
u64-maximum ID. References and Write/Read highlights contained only those two
value locations, not the identical spelling in a later type-name annotation.
An untyped target remained navigable from a typed-containing val; pull remained
full and empty without pushes. This proves the composed finite native surface,
not type resolution, compiler diagnostics, workspace references or official parity.

After that direct readback, ROOT removed the temporary observation writer and
an incidental error-message wording assertion; error-code/atomic refusal and all
payload/ID assertions remain. ID comparison now uses iterator equality without
two temporary vectors. Cleanup, later incoming-main composition and final
integration require their own source reviews and original CI; the preceding
runtime observation is not attributed to an unexecuted cleanup head.







### Ordinary parameters and local shadowing (#369)

Ordinary parameters require positive name/colon/simple-type shape, comma separation
and at most one trailing comma after a real parameter. Missing/richer types,
defaults, vararg/modifiers/annotations, receivers/context/explicit returns,
expression bodies, members/constructors/lambdas/nesting/destructuring and relevant
recovery remain refused. The leading unbounded function-type-parameter list
described above is the only generic header extension. Type names otherwise remain
retained syntax, never resolved builtin identities. Queried-name duplicates are checked separately in parameter/local
scopes; a unique preceding local wins, otherwise its same-function parameter.
Before-shadow and `val p=p` initializer reads bind the parameter; later reads bind
the local. References/highlights exclude valid same-body sites bound to the other
target instead of failing or merging the two sets. Header types are not values.

The [first-source producer/CI receipt](../ktlsp-parser/README.md#ordinary-parameter-source-observation-369--pr370)
records exact source `9e96b16bcee0e10cf9228a7e467860c52576e9ec` and original
Native38898391304/platform38898391737. ROOT directly read identical actual paired
release-CLI wire on macOS/Windows: parameter name `2:6..2:11`, reads
`3:16..3:21` and self-initializer `4:13..4:18`; local name `4:5..4:10` and read
`5:9..5:14` formed a distinct set. Hover retained original `` `p🦀`: Long `` at the
read with exact integer ID18446744073709551615. Unused `2:19..2:25` produced empty
references without declaration inclusion and a single Write highlight; sibling
function anchors stayed isolated. Annotation lookup failed explicitly, full pull
was empty, all16 original request IDs appeared exactly once and no push appeared.
Copied matching binaries ran with empty PATH and Java homes removed. Only the
observed temporary writer was removed afterward; permanent assertions remain.
This is first-source native evidence, not final-cleanup/main/official parity proof.

## Bounded syntactic document symbols

Requests read only open Kotlin document text, never disk. Each request retains one immutable URI/version/source snapshot, runs the supervised `Parser::parse_snapshot_with_cleanup` on a bounded Rust thread and projects the validated original-byte CST. Session control, JSON-RPC validation and the document store remain single-owned. Immediately before publication, the owner checks the open-document incarnation as well as exact version/content; close/reopen cannot alias an old request even with identical bytes and version. No parser result or outline is cached.

Only direct declaration-name structure proves emitted names. Supported producer shapes include named classes/interfaces/objects/enums, functions/methods, properties, typealiases, enum entries, explicit `constructor` keywords and `val`/`var` primary-constructor properties, including nested/local declarations. The outline uses nearest emitted enclosing declarations, not resolved scopes or semantic identities. Unicode and backtick spelling are preserved verbatim. A declaration range is its complete producer CST extent, including any bound leading comment; its selection range is its original name token. Implicit unnamed constructors, ordinary parameters/type parameters and anonymous expressions have no invented names. A missing function name in a top-level/member declaration is rejected, not treated as an anonymous expression.

Clients advertising `hierarchicalDocumentSymbolSupport: true` receive `DocumentSymbol[]` with `range`, `selectionRange` and `children`; other clients receive flat `SymbolInformation[]` with URI/range and nearest emitted `containerName`. `symbolKind.valueSet` is respected. Without it, the original kinds 1–18 are used: object falls back to Class, enum entry to Enum, and a typealias uses Class without implying a resolved target. If neither the intended kind nor its compatible fallback is supported, the request fails instead of emitting an unsupported client kind.

Incomplete/error trees, unsupported declaration shapes (including destructuring and unnamed companion objects), parser coverage/resource/process failures and outline ceilings reject the **whole** symbol request with RequestFailed (`-32803`), never partial/truncated success. A valid empty/comment-only file can return `[]`. Missing/closed documents or malformed request parameters return invalid params (`-32602`) when the provider is configured. Parsing uses the existing conservative **source-mode** grammar, not a complete script/grammar implementation. Comments/string mentions never become declarations. The outline itself proves no binding/type semantics; finite local navigation above is separate. There is no completion, type, refactoring or official document-symbol parity claim.

### Cancellation and library lifetime

`Session::handle` returns immediate responses; accepted parser-backed symbol/definition/hover/references/highlight/completion/prepareRename/rename requests return no immediate response. Call the nonblocking `Session::poll()` while continuing to service incoming messages to collect deferred responses. `Session::pending_requests()` includes cancelled work until its thread actually terminates. Existing parser-free requests remain immediate. The real-worker test request helper polls, and the packaged stdio client waits for each request before sending dependent edits or shutdown; bulk input does not imply synchronous completion order.

A valid `$/cancelRequest` signals the exact active string/integer ID's `AtomicBool` passed to the existing Parser and returns that request's single RequestCancelled (`-32800`) response, never a notification response. Unknown IDs do nothing; invalid cancellation notifications produce bounded error records, not wire errors. IDs are reusable after their response, while old cancelled generations remain retained solely for cleanup. A duplicate still-active ID is rejected with InvalidRequest and null ID, without terminating the original request. Committed didChange/didClose invalidate matching work with ContentModified (`-32801`); rejected edits leave it alive. Stale/cancelled results never become empty success.

Shutdown and exit cancel owned jobs. `Session::close() -> (Vec<Value>, bool)` is terminal: it returns remaining cancellation responses and observes already-cancelled threads for a one-second grace, joining only handles whose `is_finished()` is true. True requires every owned handle actually joined **and** every original Parser invocation proved no spawn attempt or same-child reaping (plus non-Unix scoped-I/O exit). A request-specific false report or parser-thread panic permanently prevents clean success, even if cleanup might finish later. False means closure is unproved, not proof a child remains alive; later/global state never invents proof or revises the selected request result. `Drop` cancels without joining or reporting success; callers needing an outcome must use `close`. A bounded process-wide registry retains every unfinished handle after Session drop; admission/poll reap only finished handles. The one-second grace excludes time inside OS calls, including join of an already-finished handle.

`run` uses the shared resumable `ktlsp_protocol::FrameReader`; partial headers/payloads survive input readiness gaps. It borrows the supplied `BufRead` without spawning a reader thread or queue. Library readers must yield WouldBlock periodically when idle to permit deferred completions; arbitrary blocking readers can delay them. The CLI supplies `BufReader<StdinInput>` over a duplicated, exclusively read native handle (not the globally buffered `std::io::Stdin`): Unix poll and Windows redirected-pipe PeekNamedPipe yield idle readiness ticks within 10 ms, excluding OS scheduling. Windows console stdin is rejected explicitly. The handle and decoder are dropped on return, including exit while the client keeps stdin open.

EOF, fatal framing or broken output cancel jobs and attempt bounded cleanup. EOF is success only after shutdown; exit is success only after shutdown; any incomplete cleanup or output failure returns status 1 and controlled stderr. No writes are retried after broken output. Responses cannot be delivered after a broken transport, but work is still cancelled. Output write/flush, filesystem/handle operations and thread creation are synchronous OS calls: backpressure, OS scheduling or stuck OS calls have no hard real-time guarantee. The server does not hide a blocked join as successful cleanup.

- At most **8 retained parser jobs per Session**, no waiting-job queue, and **8 owned parser thread handles process-wide**, shared across all eight parser-backed methods, with unfinished handles retained globally until joined. Capacity exhaustion returns RequestFailed (`-32803`) before copying source or spawning more work. These limits do not replace or weaken the parser's existing eight global Unix supervision reservations.
- Snapshot text is copied once into immutable shared storage; worker and owner share that snapshot. Each job has one bounded single-result channel, at most one rendered response, and one cancellation flag. A cancelled job releases its ID immediately but not its storage/admission slot before completion.
- The incremental decoder retains at most one 8 KiB header and one 16 MiB payload plus the CLI's bounded read buffer. There is no accumulated input-frame queue or reader-thread lifetime to join.
- Idle pending-input polls back off for 1 ms, so an immediately nonblocking
  library reader cannot keep an otherwise idle session busy-spinning. The wait
  does not change parser deadlines or claim an OS scheduling bound.

Hosted cancellation regressions use a compiled controlled **failing Rust peer** through the real Parser supervisor and packaged stdio. They check active-work cancellation, exact string/u64 IDs, wrong/invalid IDs, duplicate IDs, ID reuse, committed versus rejected edits, identical close/reopen, shutdown/exit/EOF and resource recovery. This is cancellation-mechanics coverage, not a fabricated successful CST or official oracle evidence; actual symbol/range assertions remain in the real matching-worker suite. No local parser/supervision runtime is authorized under #96.

At #337 source head `c908d13bf458da4934369e251dc2611f7877116d`,
[original attempt-1 Linux CI38860743651/job116842212955](https://github.com/anilbeesetti/ktlsp/actions/runs/38860743651/job/116842212955)
passed pinned format, strict locked workspace Clippy and 891 workspace tests.
ROOT authenticated checkout `d5be0216bcc6dc2f104fbf55239f5fc5825296dd`,
ordered parents `[a52f9518ff1532c25aff8fc1bdb1c566a05526b4,c908d13bf458da4934369e251dc2611f7877116d]`
and tree `8f2960f1ee34b78dd35ec80b91596493bb8cbde0`.
The hosted interactive consumer received RequestCancelled (`-32800`) with
the original string ID `"active"` while stdin remained open; its controlled-peer
cleanup and Linux PID-reaping assertions passed. Only the temporary observation
writer was removed after inspection. This is not successful-CST or official
oracle proof; final root-composition reviews, CI and integration remain separate.

### Independent limits

- Synchronization still accepts up to **8 MiB** per document; all parser-backed providers accept only the parser's **64 KiB** original/normalized UTF-8 source ceiling. Larger open documents remain synchronized and usable by lexical providers, but parser-backed requests explicitly fail.
- Existing parser limits stay unchanged: **500 ms** supervised exchange deadline, **16,384** flat CST elements, CST depth **128**, **4 MiB** worker reply, and **64 MiB cumulative allocation requests in the worker**. The server does not move that allocator into its own process or run the grammar in-process. OS spawn/termination/reaping and scheduling are not hard real-time bounds; allocation requests do not measure RSS or stack. See [parser supervision and residual limits](../ktlsp-parser/README.md).
- Outline output is capped at **1,024 symbols**, **32 emitted declaration levels** and a **1 MiB conservative rendered-size budget**, including worst-case escaping and repeated flat-client URI/container names. The CST traversal/direct-child scans are bounded by the validated element ceiling; output endpoints share the existing sorted UTF-16 sweep. JSON hierarchy construction is bottom-up, without recursive CST walking or subtree copies.

### Hosted native evidence

At source head `fbed703712e04c2c6b1f3a222de3cd8cc029f26a`, [original Linux Native Rust CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38843829593/job/116789694423) checked PR merge `1ff6c236d4ae0f0bac655883184ce28e159eba52` against base `a009cdfc8d075d9aef4b4cab9fb790835db22e4c` with pinned Rust 1.99.0. Format, locked workspace/all-target Clippy and locked workspace tests passed, including all **eight** new actual-worker consumer tests. The packaged stdio test copied both real binaries into one temporary directory, removed Java-related environment variables and emptied PATH. A temporary direct-Write observation recorded the actual decoded native wire reply: `documentSymbolProvider: true`, class `Outer` (kind 5, range `0:0–0:31`, selection `0:6–0:11`) enclosing method `inner` (kind 6, range `0:14–0:29`, selection `0:18–0:23`). The same permanent test verified that a server-only package omits the capability and returns `-32601`. The observation line was then removed; the regression remains.

The consumer suite also exercised hierarchical/flat kinds, original Unicode/CRLF/backtick ranges and containment, explicit constructors/local declarations, changed/stale versions, close/reopen, empty/incomplete/unsupported input, parser-process failure, exact and exceeded source/symbol/depth limits and flat rendered-output exhaustion. Two independent source reviews covered functionality and adversarial/crash boundaries; their declaration-context, comment-range and absent-provider routing findings were fixed and source-rechecked. This is bounded native Linux behavior, not a fresh official comparison, release packaging certification, performance evidence or Darwin/Windows reliability proof. **#96 remains open; no local parser runtime was rerun.**

## Lexical pull diagnostics

`textDocument/diagnostic` is available only when negotiated. It reads the current open document and returns an always-full report without result IDs, including an explicit `analysis-limit` warning when analysis is exhausted. Malformed requests or missing/closed documents return invalid params; unnegotiated requests return method-not-found. Inter-file dependencies, workspace diagnostics, related reports, refresh and progress are unsupported.

## Bounds and complexity

Existing lexer limits apply: 8 MiB document, 1,000,000 tokens, 256 lexical diagnostics, 128 nested scanner contexts. Structural nesting is capped at 1,024 open delimiters. At most 256 lexical/structural diagnostics are returned; further structural diagnostics replace the last entry with an explicit suppression marker. Lexer exhaustion instead yields the analysis-limit warning.

Retain at most `min(client rangeLimit, 10,000)` multiline fold candidates, including zero. Selection is the first candidates completed during source-order scanning; results are then sorted by start line/character/end line. A comment group completes at its separator or EOF, before a separating token can complete another fold. Nested inner folds can exhaust a small client budget before outer folds. `lineFoldingOnly: true` omits both character fields. Client `foldingRangeKind.valueSet` is ignored; kinds are not negotiated, so a comment fold still has `kind: comment` for a region-only profile. No collapsed-text support is claimed.

Each analysis scans the bounded lexer token partition with an iterative stack. Whole-line eligibility and one pending comment group use constant auxiliary state in the existing byte/token sweep; there is no backward line scan or accumulated-group rescan. Recovery consumes each stack entry once. Only retained output endpoints (at most 20,512) are sorted/deduplicated and converted in one UTF-16 sweep; no per-token scan from the document or line start. Source/token work is linear; bounded output sorting/index lookup is O(k log k).

One session cache retains the most recent document's raw analysis (or static limit error), shared by push/pull diagnostics and folding. Its exact key is URI/version plus the analysis-relevant folding range limit; the private document store invalidates it after committed edits and on close/reopen. Rejected edits preserve the current analysis. Another document's analysis evicts it before scanning. Retention is independent of open-document count: one URI of at most 8 KiB, at most 10,000 fold and 256 diagnostic records, and bounded vector spare capacity; no source or token copy. Diagnostics render only diagnostic endpoints; folding renders current `lineFoldingOnly` fields. This lexical cache adds no dependency or measured latency/RSS claim. Lexical analysis stays synchronous; native parser-backed symbol/definition/hover jobs support cooperative cancellation. Cancellable lexical analysis and complete syntax/semantic parity remain later work.

## Official observation comparison

The native test reads the committed `docs/research/oracle/lifecycle-folding/Main.kt` and its actual captured response for pinned `kotlin-lsp/v263.6379.0`. For `fun main() { ... }`, native character-aware output matches captured start/end lines 0/2, UTF-16 characters 11/1 and `kind: region`. Official also returned `collapsedText: "{...}"` and character fields despite captured `lineFoldingOnly: true`; native intentionally omits these fields for that client profile. Tests assert the exact official response and explicit inequality for the line-only native response; they do not normalize away those differences. This one recorded comparison is not full folding parity or a fresh official replay. T045 owns further isolated replay/corpus tooling.

Native tests also cover real fragmented stdio with Java variables removed and empty PATH, CRLF/lone CR/non-BMP ranges, lexical errors, malformed nesting, stale/change/close behavior, scanner/token/depth/output limits, many folds, one long Unicode line and deterministic generated arbitrary UTF-8.

The line-comment subset has independently specified native regression fixtures for boundaries, UTF-16 endpoints, shared budgets, diagnostics/recovery and fragmented stdio. No official line-comment observation or equivalence is established; the recorded block comparison above remains separate. Darwin reliability remains unresolved in #96; these fixtures do not establish current platform support or comparative performance.

## Local performance observation

2026-10-12, macOS arm64 Apple M4 / 16 GiB, pinned Rust 1.99.0 release build. `/usr/bin/time -l target/release/deps/providers-7d4529ca788e2bde --exact many_folds_and_long_unicode_line_have_bounded_outputs --nocapture` measured 0.11 s wall time, 63,520,768 bytes maximum RSS and 62,390,752 bytes peak footprint for the complete test process (including generated source, retained JSON results and test harness). Within that run, 50,000 sequential multiline pairs (200,000 source bytes, result capped at 10,000) took 30.36 ms for open plus folding; a roughly 1.2 MB single Unicode line with 200,000 astral string characters and 200,000 single-line pairs, then one multiline pair, took 82.19 ms for change plus folding. An earlier run reported 45.93/84.13 ms. These are observations on one host, not budgets, isolated per-query memory, stable benchmarks or comparisons with the official server. Real native stdio also passes 30,000-fold/limit/recovery coverage.

## Versioned finite local-val rename (#379)

With the matching trusted parser worker and
`workspace.workspaceEdit.documentChanges: true`, `textDocument/rename` supports
the authenticated ordinary-function immutable local-val subset described above.
`renameProvider` is `true` unless `textDocument.rename.prepareSupport: true`,
when it is `{ "prepareProvider": true }` and `textDocument/prepareRename` is
available. Missing versioned-edit support omits both methods, which return
MethodNotFound rather than falling back to unversioned changes.

Preparation returns the selected original declaration/use range and semantic
placeholder. Rename returns one `WorkspaceEdit.documentChanges` text-document
edit at the exact open URI and guarded current integer version, containing the
original declaration-name and every bound whole-value use once, in nonoverlapping
original-coordinate order, with the actual new spelling. Nothing is applied to
disk or sent as a server-originated applyEdit/command/resolve request.

Output names are deliberately bare ASCII: initial letter/underscore followed by
letters/digits/underscores, excluding hard keywords and all-underscore names.
Quoted or Unicode existing targets may become these supported names; new names
are never autoquoted or normalized. Invalid names/URI/UTF-16/unopened-document
requests return `-32602`. Unsupported bodies, parameters, ambiguous targets,
same-function new-name parameter/local/function conflicts (including conservative
future declarations), potential new-name capture, semantic-work/storage exhaustion,
source growth past64 KiB or escaped output past1 MiB fail atomically. Quote removal
with unchanged semantic spelling preserves existing shadow partitions.
Unsafe lexical quote removal (such as an adjacent `val` keyword and quoted name)
is refused even by preparation, rather than merging tokens in the edited source.
Parameters are refused even if their current body uses are known: external
named-argument calls are not covered.

Both methods share the original asynchronous parser admission, cancellation,
cleanup and incarnation/version/content publication guards (`-32800` cancellation,
`-32801` stale). All bounds are checked before repeated replacement strings are
materialized. This guarantees atomic computation/publication, not client rollback
after applying an edit; standard versions cannot encode post-publication reopen
incarnations. No workspace/member/parameter/full Kotlin rename, compiler conflict
analysis or official rename equivalence is claimed. Permanent paired-worker tests
apply the actual returned original-coordinate edits in memory and reparse/rebind;
their actual hosted wire/application/rebinding observation is recorded above. No local runtime or full rename/parity claim follows.
