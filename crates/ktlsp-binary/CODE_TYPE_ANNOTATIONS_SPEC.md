# T017 / #17 / #71: encoded Code type annotations

Child [#268](https://github.com/anilbeesetti/ktlsp/issues/268), parents #17/#71 remain open.

Frozen base: 45619398c7d8d5ca7b59acc5178acb4b208a67ee. Own only new
src/code_type_annotations.rs, tests/code_type_annotations.rs,
tests/fixtures/code_type_annotations/*, this file and CODE_TYPE_ANNOTATIONS_REVIEW.md,
plus additive `pub mod code_type_annotations` in lib.rs. No dependencies or callers
outside the focused tests. Existing Code/annotation APIs and helpers stay unchanged.

Reuse completed JVMS SE23 §§4.7.3, 4.7.20–21, 4.7.20.1–2 research in
CODE_SPEC.md and CLASS_TYPE_ANNOTATIONS_SPEC.md. No new oracle/compiler provenance.

## API and encoded acceptance

`decode<'a>(class: &'a ClassFile<'_>, code: &'a code::CodeBody<'_>, limits: Limits)
-> Result<Vec<CodeTypeAnnotationAttribute<'a>>, Error>` selects one existing body.
Limits has bytes/nodes/work, defaults MAX_BYTES/MAX_RECORDS/MAX_VALIDATION_UNITS;
callers may tighten, never raise. One budget covers the complete selected call.

Public Debug/PartialEq/Eq types: LocalVariableTarget { start_pc:u16, length:u16,
index:u16 }; CodeTargetInfo::{LocalVariable(Vec<LocalVariableTarget>),
Catch{exception_table_index:u16}, Offset{offset:u16},
TypeArgument{offset:u16,type_argument_index:u8}}; CodeTypeAnnotation { raw:&[u8],
target_type:u8,target_info:CodeTargetInfo,path:Vec<TypePathStep>,annotation:Annotation };
CodeTypeAnnotationAttribute { raw:&[u8],visibility:Visibility,
annotations:Vec<CodeTypeAnnotation> }. Raw annotation spans include target tag
through final element value, borrowed within raw attribute payload.

Only nested RuntimeVisibleTypeAnnotations / RuntimeInvisibleTypeAnnotations,
in input order, with at most one each by UTF16 name content. Present empty
attributes remain present. Version<52 recognized attributes fail before duplicate
or payload parsing. First payload failure precedes later duplicate detection.
Target forms:40/41 u16 table length then start_pc,length,index u16 triples;
42 u16 exception-table ordinal;43..46 u16 offset;47..4b u16 offset plus u8
argument index. Preserve tag and all encoded integers. All other tags immediately
fail Malformed("code type annotation target") without union-body reads.
Local ranges require start_pc<code_len and checked start_pc+length<=code_len,
index<max_locals (zero length allowed). Catch ordinal<exceptions.len.
Offsets<code_len. No instruction boundaries/opcode correspondence or category-two
local width, generic argument applicability, source scopes or semantic claims.
Paths reuse TypePathStep: kinds0..2 index0, kind3 any u8; read both bytes before
validation. Existing annotation decoder validates CP kinds, descriptors, duplicate
elements and nesting, borrowing UTF16 and preserving numeric bits. Trailing bytes
fail Malformed("type annotation trailing bytes"). All errors discard partial output.

## Trust boundary and accounting

First supported class version45.0..45.3 /46..67.0, then nonempty CP length<=65535.
No other class state is consumed. Selected Code need not belong to class.methods;
caller supplies the matching CP. Charge raw length against clamped bytes and work,
then precharge code exception/attribute container nodes before traversal. Re-read
Code raw with Reader: max_stack/max_locals/bytecode and exception/attribute framing
must match the public model exactly. Bytecode and nested attribute slices must be
identical borrowed spans within raw (pointer and length, including empty spans).
Check bytecode length1..65535, exception start<end<=code_len, handler<code_len,
nonzero catch CP Class kind; all nested names require nonzero UTF8 CP indices.
Reject mismatches Malformed("code body mismatch"), bad exception ranges as
Malformed("code exception range"), trailing raw as Malformed("code trailing bytes").
Truncation remains Truncated; invalid length Malformed("code length").
Validate the entire public body before decoding attributes; thus framing errors
precede malformed annotation contents. Unknown payloads are opaque but raw bytes
and all names/containers are charged. No raw copy or owned-byte/RSS promise.

One existing annotations::Budget is precharged by hard-limit minus clamped caller
nodes/work. Every visited UTF16 lookup uses Budget::text; raw length work covers
all framing bytes and opaque bytecode, while per-record work(1) covers traversal.
Output vectors/path/local table use existing type_annotations::vector (requested
and observed capacity nodes); inherited annotation vectors retain logical pair/
value nodes and MAX_DEPTH64. Root annotation records charged by their output
vector. Output attribute vector reserves2 on first recognized attribute. No budget
reset across visibility attrs, roots, local tables, paths or nested values.
Errors use existing annotation nodes/work/allocation/depth labels; byte cap uses
Limit("code type annotation bytes"). Logical nodes are not physical retained bytes. Inherited annotation vectors use
ordinary pushes; allocator failure recovery is not guaranteed.

## Acceptance and limits

Authored fixture covers all12 target tags, local/resource tables, catch ordinal,
first/last offsets, argument255, paths and nested values. Test visible/invisible
order, empty/absent/unknown attributes, duplicate text indices, version gates,
all illegal tags, every proper valid payload prefix, trailing bytes, malformed
paths/CP/annotation values, forged Code fields/spans/counts, range edges and
aggregate caller limits. Existing genuine Unit.class provides parse_class ->
code::decode -> this decoder empty-result integration only; it is not genuine
Code type-annotation evidence. Finite mutation corpus is not exhaustive safety.

#96 local native runtime HOLD: no local tests/product binaries. Formatting and
slot-authorized compile-only Clippy allowed; tests only original unchanged hosted
Linux Native CI. T017/#17/#71 remain open; Code verification/CFG/Record contexts,
resolved types/nullability, Kotlin semantics and server consumers remain pending.
