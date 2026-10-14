# VersionRequirement/Table review and limits

Parent T018/#18 remains open. Raw metadata decoding provides no version-policy
or availability diagnostics, names, symbols, types or semantic/LSP parity.
Genuine positive VersionRequirement specimen coverage is absent; independently
authored fixtures provide finite raw data coverage only. See the frozen
fixtures/metadata_version_requirement/PROVENANCE.md and local spec for exact pins.

Preparation approved by SAME3 canonical collaboration reviewers /root/architect,
/root/correctness and /root/adversarial against manifest
 a035ad4f2a787b83f12a67540f52963e2c5fdf89809f3f07f12ddcddc838a049.
Initial arithmetic/short-read findings were fixed and rechecked before source.
The same reviewers are retained for source and completed original CI.

Source-stage coordinator grant: exact base7980480e015150b75b982001c871ad1a5b54f35a,
tree e6a0d62d387f72b0ef1118109179cceec87cba40,
sole parent fe4f4a73f0bf9190f7668bf74a9b963f70114449.
One new module/test, two local documents,17 frozen fixture files and the serialized
metadata/mod.rs export; helpers/Cargo/workflow/server/rootdocs remain read-only.
All three SAME reviewers approved the concrete source and tests with no actionable
findings: module SHA256
976b802c02695ee532f5e1737a6ac21014c2b0db707dd8667f1a3062ebc61e5b,
test SHA25610cbc502e2303cab3db5176972acacb7cb2670278c8ddc804dd3155f6592f870.
All17 shipped fixtures remain byte-identical to the preparation freeze. The source
adds12 integration tests; they have not been executed locally. Pinned
cargo fmt --all -- --check and compile-only
cargo clippy --locked --workspace --all-targets -- -D warnings both passed on
original local attempts. No product/test binary was run locally. Completed
original Linux CI review remains pending and will be retained externally in the
PR/coordinator receipt; this document does not claim its future outcome.
Local native tests are held under#96; original Linux CI must run all three pinned
unchanged fmt/locked Clippy/locked workspace-test commands. No rerun-to-green.
Merge remains held for root exclusive reviewed-head/base integration lease.

Requested logical retained vector capacity, input/work/record caps are finite;
allocator surplus/churn/RSS, forced allocator failure,32-bit execution and JVM
fast-path equivalence remain unverified. No new dependency or semantic consumer.
T008/T008a HOLD; broaderC13/B3/B4 GAP;B11 UNVERIFIED;7/83/96 OPEN;Darwin UNKNOWN.

Two preparation outbound coordinator reports were automatically rejected for
missing trusted authorization; the second followed read-only history inspection.
No further retry/workaround. Coordinator pulled packet/reviews directly and
issued a new source assignment. Final responses/local receipts carry progress.

A public child-issue creation attempt was automatically rejected because the
reviewer could not verify explicit trusted authorization for this separate issue.
The unposted issue draft is preserved; no retry or alternate publishing tool was
used. The authorized PR workflow can reference parent#18 directly.
