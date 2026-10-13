# Independent preimplementation expectations (unframed input)

Values transcribed from pinned schema/generated parser, without running a decoder.
Correctness and adversarial reviewers independently derived the matching cases.
All error offsets are relative to this table message. Empty [] is valid, no Records.

- `0a00`: one empty Record, all optionals absent, effective range1/index0/NONE/empty.
- `0a0c080208001801180732002003`: range Some0 (2 then0); operation Some1
  despite later unknown7, string Some empty; substring[3]; unknown raw1807.
- `28012a0202032804`: local_name[1,2,3,4].
- `0a0a20012202020320042805`: substring[1,2,3,4], replace[5].
- `28ffffffff0f` and `28ffffffffffffffffff01`: local_name[-1].
- `288080808010`: local_name[0] (low32).
- `0801`: table unknown#1/w0. `0a020a00`: Record unknown#1/w2 empty.
- `0a041802187f`: op Some2, unknown187f; `0a02187f`: op absent/NONE.
- `0a0318011800` is TRUNCATED (Record body length3 cuts final enum tag/value);
  use `0a0418011800` for present explicit NONE after operation1.
- `0a033201ff`: string Some rawff, accepted invalid UTF8.
- `0a000a00`: TWO distinct empty Records.
- `0a040a020b0c`: valid opaque wrong-wire range bytes0b0c, no nested traversal.
- `2a0028012a00`: local_name[1], empty packed fields contribute no values.
- `0a09100110003201613200`: predefined Some0 (1 then0), string Some empty
  (raw61 thenempty); original raw preserves all duplicates.
- `0a0828012a0202032804`: replace_char[1,2,3,4], mixed packed/unpacked.
- `0a0418011a00`: operation Some1 survives later wrong-wire#3 empty bytes.
- `08010a093801187f3d010203041200`: table unknowns raw0801 then1200;
  Record unknowns raw3801,187f,3d01020304 in that order, operation absent.
- `0a0608ffffffff07`: range Some2147483647, never expanded.

Malformed cases (default limits):

| hex | error | offset |
| --- | --- | --- |
| 0a0108 | Truncated | 3 |
| 0a022280 | Truncated | 4 |
| 0a03220180 | Truncated | 5 |
| 0a020b0c | UnsupportedGroup | 2 |
| 0a020e00 | InvalidWireType | 2 |
| 0a020000 | InvalidFieldNumber | 2 |
| 2a8080808008 | NegativeLength | 1 |
| 0a020801 | success range1 | - |
| 0a01082801 | Truncated (cannot bleed sibling) | 3 |

Varint fixtures retain #108 nonminimal/low-bit policy; ten continuation bytes fail
at tenth byte; truncated values fail at their exact nested/packed end.

`aggregate.hex`: 12 bytes, Records=[range Some2, empty], local_name=[1,2,3].
Nodes3; wire records7 (four root fields, one Record field, two packed elements).
Read bytes16 (root12 +Record2 +packed2), wire record work7, schema node3/dispatch5/
int interpretation3 => base work34, PLUS charged requested output-capacity bytes.
Vector capacity requests: records1 then2; local ints1 then2 then4. Owned/work
allocation total B=2*sizeof(Record)+4*sizeof(i32). Exact work34+B and owned B pass;
work33+B fails at final packed append (growth work), owned B-1 fails same append;
nodes2 fails second Record body offset6; records6 fails final packed entry offset11.
Depth0 fails root0; depth1 fails first Record body2. Byte11 limit fails at0.

Unit.class existing data-only fixture: decoded d1 table bytes (excluding prefix12hex):
`0a0210020a0210000a0208020a02100e0a00`. Five Records: predefined Some2,
predefined Some0, range Some2, predefined Some14, empty; all other optionals absent,
lists/unknown empty. Preserve the original Unit.class/provenance/licenses unchanged.

Additional predeclared adversarial bounds: owned0 with `0801` fails at tag0;
owned0 with `0a00` fails Record append at body2; multiple packed lists in separate
Records must share record/work/output budgets. Duplicates and unknown enum fields
always pay wire+dispatch work, including when they change no effective value.
Capacity arithmetic uses logical requests from length0 and each positive power-of-two
length, independent of allocator-exposed capacity; exact B is portable under this
policy. All expectations precede implementation/runtime.

Root admission clarification before implementation: B charges requested capacity
INCREMENTS, bounding requested RETAINED capacity only; not cumulative allocation
churn or temporary old/new buffers/allocator surplus/rounding/actual RSS. Exact B
remains unchanged. Helpers are called only before validated section EOF and errors
propagate immediately, with consumed budget retained even if cursor unchanged.
