// Standalone external native consumer SOURCE. ROOT alone admits/builds/runs it.
// Compile with rustc --edition=2024 and the admitted ktlsp_binary --extern/-L paths;
// no Cargo change, JVM, compiler-generated fixture, private API or product inclusion.
// The shared integration source calls parse_class -> annotations::decode ->
// metadata::decode -> existing extended resolved_class_supertypes::decode, asserts
// complete genuine values and borrowed pointers, and dynamically discovers exact
// operation-specific resource minima with separate mutation controls.
//
// Original data custody, preserved without relying on /tmp at future execution:
// kotlin-stdlib-2.3.20.jar: 1804720 bytes; SHA256
// 0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
// ROOT original receipt ktlsp-next-original-generic-class-data.json: 64360 bytes;
// SHA256 77f572ca59838e2a39011627ec4d922e9346b2b5aa7d98ccae1555bf3e7610fe.
// Only its original_complete_bytes_hex fields were extracted, with Python stdlib
// SHA256 length/digest assertions; no parsing/product runtime or byte mutation.
// kotlin/ranges/ClosedRange.class -> ClosedRange.class.bin: 1033 bytes,
// ZIP original ordinal658, CRC32 fd5d0861, compression8; SHA256
// 50c6eeaad95ce574993ee94a1cc490660ea3af6c3d6abe5db0914640cffe080a.
// kotlin/ranges/ClosedFloatingPointRange.class -> ClosedFloatingPointRange.class.bin:
// 1194 bytes, ZIP original ordinal656, CRC32 bfffc1aa, compression8; SHA256
// 7ed8a4ea14de1c471c3a37cdf69b215a699dc6e15835825419424698b59266dc.
// Distribution 2.3.20-release-208 does not authenticate a producing compiler/vendor.
// Existing Kotlin Apache-2.0 notices, relative to crates/ktlsp-binary/:
// fixtures/t017c2/LICENSE.txt SHA256
// cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30;
// fixtures/t017c2/NOTICE.txt SHA256
// 0b09a83d3ef7795c7dec65e815866597c066c53898f2852a76599b8f5552941a.
// Existing protobuf notice: tests/fixtures/metadata_wire/PROTOBUF_LICENSE.txt.
// No selected original TypeTable-ID witness, legal inheritance, classpath,
// classfile-owner authentication, full Kotlin semantics or LSP parity is claimed.
#[path = "../../metadata_resolved_class_generic_genuine.rs"]
mod proof;

fn main() {
    proof::run();
}
