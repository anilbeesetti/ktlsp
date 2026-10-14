# Encoded CLASS type-annotation fixtures

The 28 `authored/*.hex` payloads and [expectations](EXPECTATIONS.md) were frozen
before product execution in contract SHA256
`28050fbc6c546cd2618c29e8d223eb9f246885b16e1f88de122c365bac2ee794`.
They are independently authored bytes for class targets00/10/11, paths, values
and malformed/error-precedence cases. No compiler/JVM/decoder generated them.
The CP contract and full limits are in the [specification](../../../CLASS_TYPE_ANNOTATIONS_SPEC.md).

The genuine fixture is reused unchanged from
[`../method_type_annotations/AbstractIterator.class.bin`](../method_type_annotations/AbstractIterator.class.bin),
with its existing [provenance](../method_type_annotations/provenance.json),
[Apache2.0 license](../method_type_annotations/LICENSE.txt) and
[original source-header notice](../method_type_annotations/NOTICE.txt).
Its 2361 bytes have SHA256
`6af0f4300e51fcb4d4720371f113e8f50e39b8f8e8c5f7f654a1297b275e0087`.
Class attribute header2329/payload2335 has ten bytes `00011100000000400000`:
visible target11, parameter0/bound0, empty path, CP64 Nullable, no pairs.
This provides genuine11 coverage only;00/10 remain authored-data coverage.

Encoded indices and paths do not establish superclass/interface/generic-bound
applicability, annotation identity, semantic nullability, linkage or LSP parity.
The fixture's compiler identity remains UNKNOWN; archive equality does not prove
artifact signature/trust-chain authentication. Existing notices are untouched.
