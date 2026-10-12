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
