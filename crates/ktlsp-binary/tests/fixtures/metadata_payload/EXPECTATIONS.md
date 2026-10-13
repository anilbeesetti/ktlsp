# Independently authored byte expectations before source/runtime

All hex below authored/transcribed without decoder execution. Offsets below d1-relative unless explicitly table-relative. R=sizeof(string_table::Record), F=sizeof(wire::Field), I=sizeof(i32); portable logical storage equations, not assumed ABI sizes.

Raw fixture class.hex: `04 0a 02 08 02 08 01`.
Prefix0..1 raw04; table1..5 raw0a020802; Record range Some2, body table-relative2..4 (d1 3..5); other optionals absent/lists empty. Payload5..7 raw0801, uninterpreted. Records3 (envelope+rootfield+recordfield), nodes2, depth2, owned R, work14+R (envelope2 + table base12+R). Label class is independent fixture intent only, no typed Class acceptance.

Raw fixture package.hex: `0c 0a 02 08 02 0a 00 28 01 2a 02 02 03 0b ff`.
Prefix0..1 raw0c; table1..13; payload13..15 raw0bff (invalid flat wire/group deliberately accepted). Typed records [range Some2, empty], local_name[1,2,3], no unknowns. Record bodies table-relative2..4 and6..6. Exact combined limits bytes15, records8, nodes3, depth2, owned B=2R+4I, work36+B. Envelope2 + delivered table base34+B; opaque payload no work. Exact passes; one lower bytes => Limit(bytes)0; records7 => Limit(records)12; nodes2 => Limit(nodes)7; depth1 => Limit(depth)3; work35+B => Limit(work)12 at last packed append capacity debit; owned B-1 => Limit(owned_bytes)12. Max+1 for each caller limit also passes. Increasing opaque payload length only changes bytes threshold, no other totals.

| bytes | expected default result |
| --- | --- |
| empty | Truncated0 |
| 00 | success empty table, prefix0..1, table1..1, payload1..1 |
| 00 00 0b 0c 0f 80 ff | success empty table, opaque payload1..7 |
| 80 00 ff | nonminimal prefix0..2, empty table2..2, opaque ff at2..3 |
| 80 | Truncated1 |
| 01 | Truncated1 |
| 02 0a | Truncated2 |
| 01 00 | InvalidFieldNumber1 |
| 01 0b | UnsupportedGroup1 |
| 01 0f | InvalidWireType1 |
| 03 0a 01 08 28 01 | Truncated4 at nested end; payload bytes cannot supply missing value |
| 05 0a 03 22 01 80 ff | Truncated6 at packed end; opaque ff does not rescue it |
| 81 00 0b ff | UnsupportedGroup2 (2-byte prefix remap) |
| ff ff ff ff 07 | Truncated5 (positive i32::MAX envelope, no length allocation) |
| 80 80 80 80 08 | NegativeLength0 |
| 80 80 80 80 10 ff | low32-zero prefix0..5, empty table5..5, opaque ff succeeds |
| ten 80 bytes | MalformedVarint9 |
| nine80 then02 | low32-zero prefix0..10, empty table10..10 succeeds |
| 0c 2a 0a followed by ten80 thenff | MalformedVarint12 (packed tenth byte); ff opaque |

`00`: minimum records1/nodes1/depth1/work3/owned0/bytes1 passes. With otherwise default limits records0 => records0; work0 => work0; work1 => work0 (prefix read); work2 => work1 (table root); nodes0 => nodes1; depth0 => depth1. bytes0 precedes all => bytes0. With depth0/nodes0/work2 => depth1. With nodes0/work2 => nodes1. Empty input records0 => records0; work0 or1 => Limit(work)0; work2 => Truncated0 (envelope record1 plus failed read1). `01`: work2 => work1 (failed envelope boundary), work3 => Truncated1. `02 0a`: work3 => work1 (bulk availability debit2), work4 => Truncated2.

Packed failure `05 0a 03 22 01 80 ff`: existing unframed base failure17 + envelope2 =19; work18 => Limit(work)6; work19 => Truncated6. No output allocations precede failure. For `80 00` work3 => Limit(work)2 at empty table root; work4 succeeds. Negative/malformed prefix consume existing record/read work before structural error; failing work takes precedence.

Unknown/raw fixture unknown.hex: `0f 08 01 0a 09 38 01 18 7f 3d 01 02 03 04 12 00 00 ff`.
Prefix0..1/table1..16/payload16..18. Table unknown raw0801(table0..2),1200(table13..15); Record body table4..13; unknown3801(table4..6),187f(table6..8),3d01020304(table8..13), operation absent; payload00ff uninterpreted. Exact raw order preserved. Record span/unknown spans remain TABLE-relative. Borrow identity asserted for each raw slice, prefix/table/payload; no copies.

Raw invalid-UTF8 string: `05 0a 03 32 01 ff ff`: string rawff accepted; finalff opaque. Raw wrong-wire nested bytes: `06 0a 04 0a 02 0b 0c 0b`: range absent; unknown raw0a020b0c not recursively scanned, trailing0b opaque. Repeated/default/enum/lowbit behavior reused unchanged; compose existing authored unframed cases with known prefix, preserving exact table outputs and adding prefix width only to errors.

Every-byte truncation REQUIRED cases before runtime:
- class.hex cuts n=0 => Truncated0; n=1..4 => Truncated n (envelope); n=5,6,7 succeed, because table complete and opaque payload arbitrary. Typed table identical for n>=5; payload span5..n.
- package.hex n=0 => Truncated0; n=1..12 => Truncated n; n=13,14,15 succeed. Typed table identical at >=13, payload13..n.
- unknown.hex n=0 => Truncated0; n=1..15 => Truncated n; n=16,17,18 succeed.
- To reach nested truncation after outer availability, reframe each prefix of table `0a0c080208001801180732002003`: for table prefixes m=1..13 use length=m (one byte), expect Truncated(m+1); m=0 and14 succeed. No payload added. Distinct from truncating original envelope.
- Prefix sequences 1..9 continuation80 => Truncated n; ten=>MalformedVarint9; short work must preempt at current failed cursor per frozen rules.

Budget/capacity regressions: prefix lengths1/2/10 count each read; shared packed lists across two records; duplicate recognized enum followed unknown pays all charges; unknown Field and integer output independent vector growth at0/1/2/4; known Record growth2; owned0 unknown fails at d1 tag, empty Record append fails at d1 body. Clamp each usize::MAX caller limit to existing hard ceiling (no arithmetic wrap); MAX_BYTES+1 input => bytes0 before all. records hard limit includes envelope (MAX_RECORDS-1 root scalar fields maximum if work/owned suffice); nodes hard limit root+Records; depth above2 still2. Size calculations use checked arithmetic, failed reservations precede allocation work/allocation errors as specified. Cannot deterministically induce host allocator OOM; review try_reserve error propagation and no partial output rather than force memory exhaustion.

Deterministic malformed probes future tests: for all a,b in0..255 decode [02,a,b] and [00,a,b] twice with limits bytes32/records8/nodes4/depth2/work256/owned1024; assert equal Results incl bytes/spans/errors, successful slices in bounds and pointer identity. Repeat n=0..31 bytes of every a under same bounded limits. Opaque [00,a,b] always succeeds, identical empty table and raw payload, records1/nodes1/work3; no semantic assertion. Finite tests do not prove all-input CPU/RSS/compatibility.

No precompiled artifacts added/modified. Existing pinned Kotlin v2.3.20 d57eb4a26ed01ab03cf195da686c8047032ff16f/protobuf2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c sources/notices reused as recorded in metadata_wire/string_table fixture READMEs. Eventual fixture directory preserves copies of their Apache LICENSE/NOTICE and protobuf BSD notice. New hex is authored project data, no upstream runtime/code copied. No oracle/JVM execution.
