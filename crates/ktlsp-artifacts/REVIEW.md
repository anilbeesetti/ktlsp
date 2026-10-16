# T020a / #61 review evidence

Code reviewed: `b2df58700ebe6e7edeff46c2505cc45b8ca761be` (2026-10-12).
This evidence file is the only subsequent change before PR delivery.

Separate correctness/functionality and adversarial/edge/crash reviewers inspected
the implementation independently and exercised authored temporary fixtures.
Both found the incomplete snapshot exclusion; the fix rejects any case-insensitive
SNAPSHOT suffix and Maven timestamped snapshot shape. Both rechecked the changed
behavior and approved with no remaining actionable findings. The architect
reviewed the plan, special-file fixture failures and finished implementation;
final review found no material issue.

Correctness review passed all 10 crate tests plus independent exact sources and
provenance, normalized missing-root duplicates, non-UTF8 absent roots, symlink/error
precedence, unique unreadable-file I/O versus metadata-only ambiguity, coordinate
boundaries and snapshot/near-miss matrices.

Adversarial review passed all 10 crate tests and seven temporary probe tests,
including malformed ASCII and million-byte coordinates, deep/huge/NUL root paths,
corrupt/oversized candidate precedence over two valid roots, file roots and
intermediate entries, dangling root symlinks, and a FIFO rejected without opening
or blocking. Snapshot recheck exercised 594 date/time/build combinations, every
timestamp-byte mutation, hyphenated bases, suffix forms and near misses.

Pinned local checks passed after the behavior fix:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`: 86 passed, zero failed/ignored
- `cargo tree --locked -p ktlsp-artifacts`: no dependencies

The macOS sandbox initially blocked binding the Unix socket fixture; the suite
passed with the normal approved sandbox escalation. A separate earlier fixture
failure exceeded the Unix socket pathname limit; using canonical `/tmp` fixes it.
No failure was skipped. Hosted CI results belong to the PR's exact final head;
local checks alone do not establish hosted CI success.

No Windows execution, official oracle, integrity/checksum or ZIP/JAR validation
was performed. Success returns opaque local bytes with integrity Unverified.
Filesystem containment checks require trusted quiescent roots; hostile replacement,
hard-link/mount aliases and filesystem I/O deadlines are outside this child.
Parent #20 remains open for the rest of dependency resolution. No language
capability, complete resolver or official parity claim is made.

# T020b / #88 review evidence

Implementation reviewed: `4c920e20cdec228643b5eec41124ae300087c7af` (2026-10-12).
This evidence-only addition follows those reviews; exact PR head/check results
are recorded on the PR and in the coordinator delivery report.

Separate correctness/functionality and adversarial/edge/crash reviewers approved
with no actionable findings. Both independently inspected the API, tests,
exports, manifest/lock, trust claims, dependency sources/features and notices.
Both ran all seven integrity tests and four compile-fail doctests. External
compile probes confirmed E0594 for byte/provenance/digest mutation and E0451 for
direct construction, avoiding unrelated compile failures as false evidence.

Correctness exercised 19 independently hashed binary payloads of 1..65,537 bytes,
including additional padding/block boundaries and original-allocation/provenance
checks. All 18 checked-in expected constants matched hashlib and shasum, and the
published NIST/RFC provenance was checked. Adversarial exercised 288 independent
vectors (all 256 single-byte values plus 32 boundary/random lengths), all 256
expected-digest bit flips, exact cap/cap+1, forged metadata including usize::MAX,
error precedence, capacity above cap with length three, NUL paths and immutable
cloned evidence. These audit tools/consumers are not native test dependencies.

Architect plan/build and finished-work reviews approved the validation order,
immutable evidence, allocation retention, scope and dependency boundary. The
architect independently verified all ten archive checksums, 607 source-file
matches, 19 retained license copies, all four doctests and preserved lock entries.
Both other reviewers independently checked the dependency graph and archive
hashes; correctness also checked all source/license copies. See INTEGRITY.md.

Pinned local checks passed on the reviewed implementation:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`: 144 native + four compile-fail doctests
  passed, zero failed; two inherited subprocess fixtures are intentionally ignored
  for direct harness execution and invoked by enclosing native tests.
- `cargo tree --locked -p ktlsp-artifacts --target all -e normal,build,features`
  and workspace metadata: sha2 std/force-soft only; no assembly package/features.

The initial sandboxed artifact suite could not bind the existing Unix-socket
fixture. It passed in the approved full workspace run; no failure was skipped.
Hosted CI success is a separate exact-head gate, not implied by these local checks.

No Windows execution, cross-compilation or official JVM oracle was performed.
Verification proves equality of retained bytes to a caller-provided digest only;
digest authentication, coordinate binding, archive validity, filesystem sandboxing
and future path contents remain unverified. No project/import/server integration
or broad T020/full parity claim; parent #20 remains open.

# Native Windows #329: hostile-root fixture preservation

The original reviewed-source platform
[run 38879411025 / Windows job 116902229633](https://github.com/anilbeesetti/ktlsp/actions/runs/38879411025/job/116902229633),
attempt 1, reached release test execution after source admission, format and
strict Clippy passed. Its `lookup` target reported seven passes and one failure:
`duplicates_and_invalid_roots_are_rejected` at `tests/lookup.rs:187` did not
receive `LookupError::InvalidRoot`. The old assertion printed neither the input
case nor the actual result, so that original returned variant is unknown.
The retained log `ktlsp-platform-116902229633.log` has SHA-256
`2f03f9935557f68e3938a5c4402de6379cecf5b51eac8f4a5e6e11ba479ed956`.
No unchanged retry was used to determine it.

Source review identifies a hostile-input construction defect:
`Cache::new` canonicalizes its temporary root, while the traversal case used
`cache.0.join("../escape")`. Windows
[`canonicalize`](https://doc.rust-lang.org/std/fs/fn.canonicalize.html#platform-specific-behavior)
returns an extended-length path; Rust documents that
[`PathBuf::push`](https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.push)
normalizes `.` and `..` when appending to a verbatim prefix. Thus a join can
remove the very `ParentDir` this case intends to supply before lookup receives
it. This source-level diagnosis does not invent the missing original result.

The correction appends native separators and `..` through the raw `OsString`,
then asserts that the supplied path still contains `Component::ParentDir`.
Every invalid case still requires exactly `InvalidRoot`; failure diagnostics now
retain the roots and actual result. Duplicate, relative and oversized cases,
the root count/byte/component limits, canonical/symlink checks and the production
lookup implementation are unchanged. The lockfile lookup wrappers already
delegate to that same production validation and require no API/library change.

Changed-source Windows execution remains pending ROOT's separately admitted
original platform run. Local parser execution, an unchanged retry, push,
dispatch, merge, warning suppression or acceptance of a different error variant
is not supplied by this correction. Historical platform failures are preserved.
