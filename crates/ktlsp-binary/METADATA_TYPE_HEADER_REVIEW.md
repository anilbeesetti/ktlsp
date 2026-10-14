# Exact final source integration — new original CI pending

Root admitted one final integration cut at fe4f4a73f0bf9190f7668bf74a9b963f70114449,
tree01e0300947dc10cb4f737fded962987d556358fa, soleparentcb0c8c7dd064a5a3f2d1c82097a2cfd0547e496a.
Only merge conflict was the permitted adjacent additive module exports; retained
ALL incoming modules and added type_header. Reviewed TypeHeader source/tests,
literal fixtures and behavior are unchanged. Earlier c0e44e99 PR-CI is historical;
SAME3 actual combined-source and new original-CI reviews are required. Merge held.

Compatibility readback: delivered #193 private decode_with_budget extraction
preserves public Class/Function/Property/payload APIs used by this child's genuine
consumer. Existing public calls create their own budget and delegate to the same
loops. Resolver's existing public standalone path retains separate budgets;
new private shared composition paths are not invoked by TypeHeader. wire::Budget
work visibility changed only, its field/charge behavior is unchanged. TypeHeader
still uses new+field+dispatch directly, zero allocation/recursion and N+2F.
ValueParameter retains nested Type occurrences solely in raw; TypeTable exposes
borrowed opaque bodies; TypeAlias preserves every inline occurrence in ordered
borrowed lists. None calls or changes TypeHeader, merging, table lookup or
semantic nullability. No helper, Cargo/lock/workflow/parser/vendor changes relative
to the exact incoming base. B3 remains GAP: first and final observed Timeout /
sendOk16 / MissingACK are not promoted by green CI. #96/Darwin remain unknown.

The prior source-review receipt below is historical, preserved for provenance.

# Selected Type header review

Root admitted frozen contract a90880e51fedead92eba370cebf9b83a30e49c06314dc8a0fc51ec9b3b5b5157
and literal expectations b74771ca41036bdc0e5d1a96409e173b459a46f513d7255d0846a4d647a47dd4.
Contract reviewers /root/correctness, /root/adversarial and /root/architect all
approved with no findings. SAME3 source reviews APPROVED at b25e778ed1e8140a80ecd3d5fa9563ea5e82cfdb,
no actionable findings. Correctness and adversarial reviewers independently
checked schema/wire/budget/error behavior, frozen matrices/cuts/limits, exact
notice/expectation identities and actual Unit consumer path; architect approved
minimal scope/API and resource boundaries. Final exact-head/completed-original-CI
reviews remain pending. Source SHA256
58a12f02a650d710b3f6e7ac268dbc09e295325180d11c312ce0002d7b1ad82e;
test SHA256 0e0748d105aef9b8ce4fc3cf1ac1f97246ccf92304aac534e28b9b077c1a1a4f.
Child #197 is published under #68/#18; stable-base integration/merge remain held.

Scope: one allocation-free unframed Type scalar scan, complete borrowed raw,
optional presence, low32 signed integers/full64 Boolean, wrong-wire/duplicate
policy, unchanged wire budget/error propagation, one additive module export.
Twelve exclusive paths plus mod.rs only; no dependency/shared-helper changes.
Tests freeze all52 byte cuts, exact error/resource boundaries, all10 scalar wire
matrices and genuine existing Unit.class return Type3004 -> classifier4 ->
kotlin/String through actual Class/Function APIs and delivered resolver.

Local cargo fmt --all -- --check and compile-only cargo clippy --locked
--workspace --all-targets -- -D warnings passed at base b026508e plus this change.
No local tests/product/parser/worker/server,
benchmark, JVM/compiler tool or oracle executed. Original hosted CI runtime pending.
No semantic graph, nested initialization/merging, name/index validation, server
consumer, whole-pipeline memory/work or parity claim. #68/#18 stay open;
T008/T008a HOLD, broaderC13/B3/B4 GAP, B11 UNVERIFIED, #7/#83/#96 OPEN,
Darwin UNKNOWN. Inherited deterministic raw64 compatibility limit remains.
