# #138 data provenance

class.hex, package.hex and unknown.hex are independently hand-authored synthetic
envelopes. Their names express fixture intent, not validated class/package schemas.
EXPECTATIONS.md was frozen before product implementation/runtime, SHA256
8ca7d97dbf9bf270744c5aeb9fd5422b6d6b8bb8915538dd64ed2f6c32fd98c8.
Raw hex SHA256 respectively:
c0b5909783dc552c06d852967cc2961a571e9c3bd5ea1bef5bdc030450ac9194,
e9412a4f15753e407bb02da1d25b7408a5514389f42c8a4d7d29e776a04afc97,
a649b174667afafa937cded56ecdd81b4b6c7ef11593dbf87102d287cd0bf778.

Same Kotlin v2.3.20 pin d57eb4a26ed01ab03cf195da686c8047032ff16f and
protobuf2.6.1 bba83652e1be610bdb7ee1566ad18346d98b843c source rules as
../metadata_wire/README.md and ../metadata_string_table/README.md.
Their recorded schema/generated parser/framing source hashes remain authoritative;
no source rediscovery, upstream runtime or new precompiled artifact is added.
Apache LICENSE/NOTICE and protobuf BSD notice copied unmodified from the existing
metadata_string_table fixture. No Java/compiler/oracle/Gradle/Maven execution.

Additional test derivations before runtime, from the same frozen charge schedule:
three unknown scalar fields need work15+4*sizeof(Field), owned4*sizeof(Field);
three unpacked local ints work18+4*sizeof(i32), owned4*sizeof(i32);
two empty Records work13+2*sizeof(Record), owned2*sizeof(Record).
Last growth occurs at d1 offset5 in each case. Duplicate enum table requires
work20+sizeof(Record)+sizeof(Field); one less fails Record append at d1 body3.
Three packed lists in two Records require records12 including envelope; records11
fails final packed entry at d1 offset16. These are manual equations, not outcomes.
Hard-limit table lengths131068/131070 have independently encoded prefixfcff07/feff07;
65,534 scalar fields fit the aggregate record ceiling, the next fails at its tag.

All three independent source reviewers requested exact growth at len4 before
runtime: five unknown scalars need work23+8*sizeof(Field), owned8*sizeof(Field);
five unpacked local ints work28+8*sizeof(i32), owned8*sizeof(i32). Final growth
error offset9 for either one-less work or owned. These manual source/contract
derivations extend the checks without changing the frozen raw fixtures/expectations.
