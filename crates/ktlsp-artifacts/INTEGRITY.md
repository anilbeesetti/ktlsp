# T020b / #88 dependency and fixture evidence

Audit date: 2026-10-12. Product dependency: exact sha2 0.10.9,
`default-features=false`, features `std,force-soft` only. Published Cargo manifests,
resolved normal/build/all-target graph and build scripts inspected before addition.
Downloaded `.crate` SHA-256 values matched the audit Cargo.lock; all 607 published
files were compared byte-for-byte with the extracted sources used for review.
Original license files are retained in `notices/`; no upstream implementation or
dev dependencies were copied into first-party source.

| Package | Version | License | Published archive SHA-256 |
| --- | --- | --- | --- |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | `3078c7629b62d3f0439517fa394996acacc5cbc91c5a20d8c658e77abd503a71` |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | `59ed5838eebb26a2bb2e58f6d5b5316989ae9d08bab10e0e6d103e656d1b0280` |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | `78c8292055d1c1df0cce5d180393dc8cce0abec0a7102adb6c7b1eef6016d60a` |
| digest | 0.10.7 | MIT OR Apache-2.0 | `9ed9a281f7bc9b7576e61468ba615a66a5c8cfdff42420a70aa82701a3b1e292` |
| generic-array | 0.14.7 | MIT | `85649ca51fd72272d7821adaf274ad91c288277713d9c18820d8499a7ff69e9a` |
| libc | 0.2.190 | MIT OR Apache-2.0 | `ce5d3ddc6d3fa000eb1536d85e147bfe31aacaba692ed6a876f95cb7c855be78` |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` |
| typenum | 1.20.1 | MIT OR Apache-2.0 | `b6f5e870be6c3b371b77fe0ee0bafb859fa4964b4404c27de1d380043c4dda20` |
| version_check | 0.9.5 | MIT/Apache-2.0 | `0b928f33d975fc6ad9f86c8f283853ad26bdd5b10b7f1542aa2fa15e2289105a` |

Selected graph:

```text
sha2 -> cfg-if, digest, cpufeatures (aarch64/x86/x86_64 only)
digest [alloc,block-buffer,core-api,default,std] -> block-buffer, crypto-common
block-buffer -> generic-array
crypto-common [std] -> generic-array [more_lengths], typenum
generic-array -> typenum; build -> version_check
cpufeatures -> libc [no features] on aarch64 Linux/Android/Apple
```

Other packages have no enabled named features. `sha2` has `build=false`, but that
alone is not the audit: generic-array's Rust build script invokes version_check,
which asks rustc for its version. libc's Rust build script checks rustc/platform
configuration. It contains FreeBSD and Emscripten version probes, unreachable
through these selected libc target edges. cpufeatures declares a loongarch64 Linux
libc edge, unreachable from sha2's cpufeatures architecture predicate. libc is Rust
platform FFI declarations, not bundled C code or a native analysis/compiler build.
No cc/cmake/native library build, JVM, allocator or Java dependency is selected.

`sha256.rs` and `sha512.rs` select the Rust soft module before architecture paths
when force-soft is enabled. `asm`, `asm-aarch64`, `loongarch64_asm`, sha2-asm,
force-soft-compact, oid, rand_core/getrandom, optional generic-array serde/zeroize
and upstream test/benchmark dependencies are absent. cpufeatures/libc remain in
the target dependency graph even though force-soft avoids CPU-specific hashing.
Recheck feature unification after integration; do not enable assembly features.

Reproduce graph inspection:

```sh
cargo tree --locked -p ktlsp-artifacts --target all -e normal,build,features
cargo metadata --locked --format-version 1
```

## Test-vector provenance

The `abc` and 56-byte `abcdbc...nopq` known answers are published in
[NIST's SHA-256 worked examples](https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/SHA256.pdf).
The million-`a` answer is published in
[RFC 6234's test driver](https://www.rfc-editor.org/rfc/rfc6234#section-8.5).
No implementation code was copied from either source.

All expected constants in tests were independently checked with Python hashlib
and `shasum -a 256` on exactly the same bytes. These tools are audit-only; native
tests neither invoke them nor compute expected answers with sha2. Boundary inputs
are ASCII `a` repeated 1,55,56,63,64,65,119,120,127,128,129 times. Binary inputs
are 257 NUL/FF bytes and 4097 deterministic bytes: start u32 seed 0x12345678,
update with wrapping `seed*1664525+1013904223`, take the high byte. The exact cap
is 64 MiB of NUL, SHA256 `3b6a07d0d404fab4e23b6d34bc6696a6a312dd92821332385e5af7c01c421351`.

Tests also cover expected-digest and payload mutations, structural-error priority
with forged lengths and wrong digests, original allocation retention, an invalid
NUL-containing nonexistent path, path replacement before verification and deletion
after verification. Compile-fail doctests reject byte/provenance/digest mutation
and direct wrapper construction. Successful hashing authenticates neither supplied
metadata nor the expected digest and validates no archive structure.
