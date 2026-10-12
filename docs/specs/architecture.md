# Architecture and first-wave interfaces

Use a Cargo workspace with small pure-Rust crates. Keep parsing, text and project-model logic independent from JSON-RPC. The first wave establishes bounded, safe infrastructure; it is not a Kotlin compiler replacement.

## Crates

- `ktlsp-text`: `Position { line: u32, character: u32 }`, `Range { start: Position, end: Position }`, `TextChange { range: Option<Range>, range_length: Option<u32>, text: String }`, `Document { version: i32, text: String }`. `Document::new(version, text) -> Result<Document, TextError>`, `apply_changes(version, &[TextChange]) -> Result<(), TextError>`, `position_to_offset(&str, Position) -> Result<usize, TextError>`, `offset_to_position(&str, usize) -> Result<Position, TextError>`. JSON fields use camelCase. Atomic edits and strict UTF-16 boundaries. No protocol transport coupling.
- `ktlsp-protocol`: bounded `read_frame<R: BufRead>(&mut R) -> Result<Option<Vec<u8>>, ProtocolError>` and `write_frame<W: Write>(&mut W, &[u8]) -> Result<(), ProtocolError>`; defaults at most 8 KiB headers and 16 MiB payload. A separate `Session`/dispatch layer validates JSON-RPC lifecycle and preserves IDs. JSON payload deserialization belongs outside framing. No allocation based on unchecked content length.
- `ktlsp-syntax`: initial lossless Kotlin lexer. `lex(&str) -> Result<Lexed, SyntaxError>`, with tokens exposing `kind`, `start`, `end` byte offsets and lexical diagnostics exposing byte spans/message. Token spans partition the complete source, including whitespace/comments. UTF-8 boundaries hold for every span. Nested comments, escaped/backtick identifiers, strings/raw strings/characters, malformed EOF and delimiter token kinds are covered. Lexical classification is explicitly not semantic highlighting or compiler diagnostics. Error-tolerant syntax/declarations follow in separate tickets.
- `ktlsp-project`: `ProjectModel`/modules/source roots/dependencies/language settings with a versioned explicit JSON manifest; `load_manifest(&Path) -> Result<ProjectModel, ProjectError>`. Resolve relative paths against the manifest; validate duplicate IDs, dependency cycles, missing module targets, source ownership and bounded input. Data-only; never spawn build tools. Partial declarative imports follow separately.
- `ktlsp-server`: binary `ktlsp` and testable dispatch library. Single-owner initial document store, bounded worker/query interfaces later. Integrate transport/text/project/syntax only after their contracts pass review. Advertise only implemented providers. All message limits and unsupported features are visible in documentation.

## Semantic engine roadmap

Lossless error-recovering CST -> AST/HIR -> stable declaration IDs and scopes -> import/module resolution -> Kotlin types/generics/constraints -> overload resolution/extensions -> CFG/data flow/smart casts/contracts -> source/binary dependency symbols -> incremental query cache. Kotlin language-version gates are separate from LSP features.

Use immutable revisioned snapshots; invalidate only affected module/file queries. Long queries need cancellation and budgets. Semantic edits must use resolved symbol identity and document versions, never global name substitution. Parser resource limits must be checked before recursively consuming user input. Persistent indexes require versioned schemas and corruption recovery.

## Parser decision

Tree-sitter Kotlin builds C, violating the literal language requirement. `ktrs` is a promising Rust-only candidate but its parser has documented losslessness and pathological-input concerns; evaluate it in a dedicated ticket before integrating. The initial lexer is an independently testable foundation, not a substitute for a complete grammar.

## Agent ownership

Each ticket owns explicit files and an isolated git worktree. Workspace/root dependency changes are coordinated through the root agent. Implementers spawn two independent reviewers (correctness/functionality and adversarial/edge/crash), fix findings, rerun the relevant checks, then push, open and attach a PR, and merge only the reviewed head after CI/integration checks pass. Obtain a review-slot allocation from root before spawning to stay within seven concurrent agents. The user already authorized pushes, PRs and passing merges. No approval prompt is needed for routine delivery.

## Concrete initial resource and lexical contracts

Text limit: 8 MiB UTF-8 per document; at most 1,024 changes in one notification. Reject excess without mutation. LF, CRLF and lone CR each terminate a line; CRLF is one terminator. LSP positions cover content excluding line terminators. Offset at terminator start maps to preceding line end; offset between CR and LF is invalid; offset after terminator maps to next line start. rangeLength counts UTF-16 units of the replaced slice including every CR/LF code unit. Positions on surrogate interiors are invalid. Version must strictly increase.

Lexer limit: 8 MiB source, 1,000,000 tokens, 256 diagnostics, and 128 nested comment/template/scanner contexts. Exceeding a resource limit returns SyntaxError (no silently truncated partition). On success all token spans exactly partition source. String templates remain opaque inside a single string token in this initial lexer: `${f("x", { 1 })}` must not terminate the outer token at the inner quote or expose its braces as outer-file delimiters. Track balanced interpolation braces and nested strings/comments using bounded state; raw strings and escaped `$` differ. This is not enough to analyze interpolation expressions semantically; that remains T007/T008 work. Diagnostics may stop after the first 256 with an explicit overflow error rather than unbounded accumulation.

Framing grammar: require CRLF lines ending at CRLF-CRLF; header names case-insensitive, ASCII colon separator, trim ASCII space/tab around values. Require exactly one positive/zero decimal Content-Length, reject signs/overflow/duplicates. Optional Content-Type may specify only UTF-8 (case-insensitive utf8/utf-8); reject unsupported charset. EOF is clean only before any frame bytes; partial header/body is an error. Framing errors are fatal for that stream; do not guess resynchronization. JSON-RPC parse/validation errors on a fully framed payload are recoverable.
