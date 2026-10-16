# T020 / #267 caller-digest lockfile child

Frozen base: `45619398c7d8d5ca7b59acc5178acb4b208a67ee`.
Owned: `src/lockfile.rs`, `tests/lockfile.rs`, `tests/fixtures/lockfile/*`,
this file, and additive `pub mod lockfile;` in `src/lib.rs`. Coordinator cleared
existing workspace serde/serde_json crate references and ONLY their two dependency
names in the existing ktlsp-artifacts Cargo.lock package entry; no new versions.

## Acceptance contract

`lockfile::parse_lockfile(&[u8])` parses strict JSON schema version 1:

```json
{"schemaVersion":1,"entries":[{"group":"org.example","artifact":"lib","version":"1.2.3","extension":"jar","sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad","digestOrigin":"callerProvided"}]}
```

Every field except classifier is required; classifier may be absent or null.
Unknown and duplicate object fields reject. `digestOrigin` must be
`callerProvided`; it records who supplied the expected digest, not authenticity.
Digest strings must be exactly 64 ASCII hex characters (both cases accepted).
Existing `Coordinate::new` validates exact fixed coordinates; dynamic/snapshot,
traversal and nonportable fields reject. Duplicate coordinates reject even when
the digest agrees. Binary and sources classifier entries remain distinct.

Input is at most 1 MiB before JSON decoding, at most 1,024 entries during
sequence decoding, and at most 1,024 encoded bytes in each generated relative
path. serde_json's default recursion limit remains enabled. An entry limit
failure is a JSON decode error; other parse failures distinguish input/schema,
coordinate, duplicate, path and digest errors. Empty lists are permitted.
The path cap is defensive and unreachable under current Coordinate bounds.
Input bounds also bound raw strings before coordinate/digest validation;
these are not whole-process RSS or I/O latency guarantees.

The returned private-field Lockfile/LockedArtifact values expose ordered borrowed
entries, coordinates, digest and caller origin. `Lockfile::lookup(coordinate, roots)`
returns explicit `NotLocked` when absent. `LockedArtifact::lookup(roots)` calls
existing `lookup` then `verify_sha256`, yielding immutable retained bytes plus
unchanged `Unverified` lookup provenance. Typed lookup (including missing) and
integrity errors are preserved. One requested artifact is read per call; no
whole-lockfile bulk read or allocation of all artifact bytes is offered.
Caller-supplied roots inherit existing 16-root/4096-byte/256-component checks,
symlink rejection and trusted quiescent-cache boundary. Lockfiles store no paths.

## Fixture and negative coverage

Independently authored `ordered.json` binds binary and sources coordinates to
NIST `abc` SHA-256 (fixture bytes are arbitrary data, not JARs). Tests assert
order, classifier distinction, exact retained bytes/provenance/origin,
missing lock entry, missing cache artifact, digest mismatch and inherited
root/ambiguity/symlink rejection. Parse tests cover strict schema and malformed
JSON/UTF-8, duplicate/conflicting coordinates, invalid/dynamic coordinate fields,
invalid digests/origin, exact/over input and entry limits, fixture truncations and
bounded generated hostile inputs. No official oracle evidence is claimed.

## Delivery evidence and limits

Architect plan review approved with classifier-null, trailing-JSON and ambiguity
clarifications, and the unreachable defensive path-cap limit noted. Correctness
and adversarial reviewers independently reported no actionable findings on current
source; both reviewed the origin-object rejection fix. Architect requested valid
first values in duplicate-field tests and accurate immutability evidence wording;
those changes are applied. Both independent reviewers rechecked the changed tests
and documentation with no actionable findings. Final architect source/design review
approved commit/PR with no actionable findings. All three reviews were source-only
under #96 HOLD; hosted final-head CI and root integration guards remain required.

Local format passed. Initial compile-only Clippy failed on chunks_exact_to_as_chunks,
then on cloned_ref_to_slice_refs in tests; both style findings were corrected and
locked all-workspace/all-target Clippy passed with CARGO_BUILD_JOBS=2. No local
native runtime/tests were executed under #96 HOLD. Original unchanged hosted Linux
Native CI must execute locked workspace tests. Immutability is a private-field,
borrowed-getter API property, not an added compile-fail test claim.

No downloads, network, cache mutation, transitive/snapshot resolution,
authenticated digests, archive validity, JAR semantics, source-symbol/server
consumer or semantic/full Kotlin parity is introduced. Parent T020 remains open.
