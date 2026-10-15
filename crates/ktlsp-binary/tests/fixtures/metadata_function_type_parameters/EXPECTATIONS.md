# Literal acceptance freeze

S = size_of::<SelectedTypeParameter>(), B = size_of::<UpperBound>(); symbolic ABI
sizes. Every header/raw/child/bound slice borrows the exact supplied input pointer.
Child spans below are Function-relative; bounds are child-relative.

- minimal: parent name0, absent flags; no children. N2,F1; records2,nodes1,
  depth1,work9,owned0.
- selected: parent old_flagsSome(0),name1,flagsSome(6); one child6..20,
  id2,name3,reifiedSome(true),varianceSome(Out), bound0bff at10..12,
  IDs[7]. N22,F4,n14,f6,U1; records14,nodes2,depth2,
  work93+B+S,ownedB+4+S.
- duplicates: parent name0; identical children4..8,12..16 with id0,name1;
  duplicates and order retained; unknown field7 remains raw.
- wrong_wire: field4 varint/fixed32/fixed64 ignored, no children.
- opaque: parent Types/ValueParameter/extension invalid nested bytes ignored;
  one child14..20 with id0,name0, opaque empty bound6..6.
- nonminimal: nonminimal parent tag/value/child tag/length/id tag accepted;
  child8..13, id0,name0.

Required failures (default limits): empty parent Function(MissingName)@0;
10 00 22 00 -> TypeParameter(MissingId)@4;
10 00 22 02 08 00 -> TypeParameter(MissingName)@6;
22 00 -> Function(MissingName)@2;
10 00 22 00 80 -> Function(Wire(Truncated))@5;
10 00 22 03 08 00 80 -> TypeParameter(Wire(Truncated))@7;
10 00 22 07 08 00 10 00 32 01 80 10 01 ->
TypeParameter(Wire(Truncated))@11 (packed end, never sibling bytes).
Malformed framing always precedes required-field EOF checks. Child errors rebase
once. Depth/node admission precedes child parse; child parse precedes outer push.

For k minimal children 22 04 08 00 10 00 after 10 00, q=next_power_of_two(k):
N=2+6k,F=1+k; records2+4k,nodes1+k,depth2,work9+25k+q*S,
ownedq*S. Boundary and immediately-below tests freeze aggregate accounting.
Both scans, ignored fields, child bounds/packed IDs and all vectors share budget.
Known child limits/errors are mapped to TypeParameter and rebased exactly once;
outer admission/push faults map Wire at Function offsets.
