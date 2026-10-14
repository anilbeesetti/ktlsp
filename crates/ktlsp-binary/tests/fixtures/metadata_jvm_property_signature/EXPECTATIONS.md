# Literal acceptance freeze — revision 1, no product execution

CASES.json contains47 valid complete parent literals and24 malformed literals;
fixtures/*.hex are exact text copies plus two named genuine slices pair_first/
pair_second. author_literals.py is only a handwritten-literal materializer with
span/arithmetic assertions, not a protobuf encoder/decoder. All outcomes/charges
are independently transcribed from source and current wire/Budget rules. The
signature notation and sizeof-based budget units are documented in CASES.json.
B=sizeof(PR219 SignatureBody); M=sizeof(MemberSignatureBody). No host size guess.

Every valid row succeeds at its exact bytes/records/work/owned values. Its work
threshold is work_constant+B_capacity*B+M_capacity*M; owned is the capacity terms.
Raising limits to usize::MAX also succeeds under unchanged hard clamping for these
small rows. V00 with all limits0 succeeds absent; other rows independently tighten
one bucket with others default. Raw and every outer/member body slice are pointer-
identical to supplied parent slices. All field/body ranges are half-open and
Property-relative. No source/API/runtime result generated this freeze.

V01 versus V02/V26 distinguish absent nested slots from present empty leaves.
V03 covers all five explicit slots in order. V04/V06 split name/desc inside one
outer; V05 plus C_field/C_synthetic/C_getter/C_setter/C_delegate prove recursive
merges across outer messages including empty later leaves/outer. D_* prove last
recognized scalar wins, then empty leaf preserves it. V18 wrong-wire name and
V32 wrong-wire descriptor cannot erase prior scalars; V31 wrong-wire getter cannot
erase an earlier slot. V33 interleaves all five slots across both outer occurrences,
merging independent names/descriptors. V28 proves outer associations0/2 across an
empty outer1. V34 overwrites both scalar values across duplicate outer messages
while preserving the earlier companion scalar at the middle prefix; ordinal is
the bodies index, independent of the flattened leaf count and slot kind.

V08/V09 signed-negative low32; V10 positive high bits truncate to explicit0;
V11 canonical10-byte negative; V12 nonminimal tags/length/scalar; V13 nested tag
low32; V29 parent length low32; V30 parent tag low32. V14 wrong-wire100 is absent;
V15 wrong-wire leaf slot creates no presence. V16/V17 wrong-wire scalar bytes/
fixed32 preserve present empty leaf. V19 unknown outer6 bytes00, V20 unknown
parent101 bytes00, V23 opaque Type00 remain unscanned. V21 leaf3=99 is unknown,
NOT an enum; signature schemas have no enum. V22 Property.name does not gate this
reader. V24 wrong-wire outer100 cannot clear earlier recognized getter.

## Every-prefix outcomes

For EACH of47 valid parents, cut0 succeeds absent. Full-length cut has CASES
result. Except for the complete parent-field boundaries listed below, every proper
nonzero prefix cut fails Truncated@cut (parent framing precedes selected descent).
Complete prefix results are valid independent messages, never partial error values.

|Rows|other complete cuts|expected at those cuts|
|---|---|---|
|V05|7,14,19|getter(7,None), getter(7,8), getter(7,8); respectively1/2/3 outer and leaf bodies, existing spans/ordinals unchanged|
|V22|2|absent, zero bodies; parent name ignored|
|V23|3|absent, zero bodies; opaque Type ignored|
|V24|7|getter(7,None), one outer/leaf, exact initial spans|
|V27|3,6|present extension all slots absent; first1/2 empty outer bodies|
|V28|5,8|getter(None,None); first1/2 outer bodies; one getter leaf ordinal0|
|V31|7|getter(7,None); first outer and its leaf|
|V33|23|all five slots with respective names1/3/5/7/9, descriptorsNone; first outer and first five leaves|
|V34|9,16|getter(7,8), getter(9,8); first1/2 outer and leaf bodies; full getter(9,10)|
|C_*|7,14|selected slot(7,None), selected slot(7,8); first1/2 outer/leaf bodies|
|P_first/P_second|2,6,10|absent extension, zero bodies|

No other complete cuts exist (not even a completed inner leaf when its containing
outer declared body is still short). Selected-body errors cannot consume later
sibling/parent bytes: M12 Truncated@6 and M13 Truncated@7 despite following10 00.
M15/M21 preserve no partial success from the earlier valid outer. M16..18 locate
negative lengths at parent/outer/leaf length starts. M19/M20 overflow-continuation
failure occurs at exact tenth byte. Unknown malformed-looking opaque00 succeeds,
but selected00 fails at its original parent coordinate.

## Exact/one-below and aggregate budget freeze

Offsets below are Property-relative. Only the named bucket is tightened; all
others default unless stated. Default constant values must be asserted exactly.

|Row|limit|error kind@offset|
|---|---|---|
|V25|bytes8|Limit(bytes)@0|
|V25|records0/1/2/3|Limit(records)@0/3/5/7 respectively|
|V25|work0|Limit(work)@0, record-work|
|V25|work1|Limit(work)@0, first tag byte|
|V25|work3|Limit(work)@2, length byte|
|V25|work9|Limit(work)@3, parent bulkbody (needs6 after4 spent)|
|V25|work10|Limit(work)@0, parent dispatch|
|V25|work18|Limit(work)@3, outer getter dispatch|
|V25|work26|Limit(work)@7, final scalar dispatch|
|V25|work27+M-1|Limit(work)@3, first leaf capacity|
|V25|work27+M+B-1|Limit(work)@0, outer capacity|
|V25|ownedM-1|Limit(owned_bytes)@3|
|V25|ownedM+B-1|Limit(owned_bytes)@0|
|V25|ownedM-1 AND work27|Limit(owned_bytes)@3 before capacity-work|
|V02|ownedM+B-1|Limit(owned_bytes)@0, empty leaf already charged|
|V03|records15|Limit(records)@31, last delegate scalar|
|V03|owned8M-1|Limit(owned_bytes)@27, fifth leaf growth|
|V03|owned8M+B-1|Limit(owned_bytes)@0, outer after all leaves|
|V03|work115+8M-1|Limit(work)@27, fifth leaf growth|
|V03|work115+8M+B-1|Limit(work)@0, outer growth|
|V05|records8|Limit(records)@19, fourth empty outer still counted|
|V05|owned2B+4M-1|Limit(owned_bytes)@17, third empty leaf growth|
|V05|owned4B+4M-1|Limit(owned_bytes)@14, third outer growth|
|V05|work54+4B+4M-1|Limit(work)@19, final empty outer dispatch|
|V27|records2|Limit(records)@6|
|V27|owned4B-1|Limit(owned_bytes)@6|
|V27|work15+4B-1|Limit(work)@6|
|V28|records4|Limit(records)@11, leaf of third outer|
|V28|owned4B+2M-1|Limit(owned_bytes)@8, third outer growth|
|V28|work27+4B+2M-1|Limit(work)@8|
|P_first/P_second|bytes22|Limit(bytes)@0|
|P_first/P_second|records8|Limit(records)@21|
|P_first/P_second|owned2M-1|Limit(owned_bytes)@17|
|P_first/P_second|owned2M+B-1|Limit(owned_bytes)@10|
|P_first/P_second|work57+2M+B-1|Limit(work)@10|
|M07|owned0|InvalidFieldNumber@5 before any leaf/outer append|
|M07|records2|Limit(records)@5 before invalid leaf tag|
|M07|bytes5|Limit(bytes)@0 before structure|
|M08|work14|Limit(work)@5, leaf tag|
|M08|work15|Limit(work)@6, failed read's work before truncation|
|M08|work16|Truncated@6|
|M12|work12|Limit(work)@5 before failed bulk Truncated@6|
|M13|work18|Limit(work)@7 before failed bulk Truncated@7|

V20 exact records1/work6/owned0 succeeds: malformed-looking unknown bytes are
opaque. P_* exact bytes23/records9/work57+B+2M/ownedB+2M succeeds. No other
pipeline budget is included in these selected-call thresholds. Allocation failure
is a controlled Limit(allocation) at push tag, static review only.

## Future hard-cap/hostile probes — NOT RUN

- Input vec![0;MAX_BYTES+1], all limitsusize::MAX => Limit(bytes)@0 before field0.
- Repeat literal a2 06 00 65535times => success, O65535, records65535,
  N196605, work327675+65536B, owned65536B. One more=>Limit(records)@196605.
- Repeat a2 06 02 1a 00 32767times then append a2 06 00 => success, O32768,
  I32767, records65535, N163838, work360442+32768B+32768M,
  owned32768B+32768M. Instead repeat the selected-leaf literal32768times =>
  Limit(records)@163838: the last outer is record65535 and its child attempts65536.
- Default fields exactly MAX_BYTES/MAX_RECORDS/MAX_VALIDATION_UNITS/MAX_BYTES;
  raising caller values cannot raise caps by unchanged Budget inspection. Dynamic
  work/owned hard-ceiling exhaustion is NOT established by these shallow probes.
- All65536 two-byte inputs and uniform bytes length0..31 under32bytes/8records/
  256work/1024owned: deterministic/no panic, error offset<=length; successful
  raw/body pointer identity, ordered bounded spans/outer associations, signature
  absent iff bodiesempty. Finite crash checks are not full fuzzing/all-input proof.

## Genuine data and hold

P_first/P_second are byte-identical to unchanged Pair d1[234..257]/[259..282].
Property field d1 ranges[232..257]/[257..282]. Add234 or259 to any selected-call
body/field span/error for decoded d1 coordinates. Getter leaf bodies d1[253..257]
and[278..282]; outer bodies[247..257]/[272..282]; field leaf[249..251]/[274..276].
Getters9/10 and12/10 resolve via current UTF16 resolver to real method CP tuples
23/24 and26/24. Field desc11 resolves to the encoded descriptor of TWO fields;
field name=None. No backing-field inference or other-slot genuine claim.
Data checks pass; Rust composition, tests, CI and source are HELD/unexecuted.
