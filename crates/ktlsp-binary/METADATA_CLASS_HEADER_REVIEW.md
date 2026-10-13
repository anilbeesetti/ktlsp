# #146 source-stage review evidence

Frozen clean independent clone base882028224a252d728dd479f2403fd2f2d97f3f67,
tree350124dfd59e3f6bc895674591d60bf28744f3a4. Owned branch
codex/metadata-class-header; issue https://github.com/anilbeesetti/ktlsp/issues/146.
Root read actual preparation approvals and all14 original manifest entries before
admitting only finite source and issue publication. Shared mod.rs change is one
additive class_header export. Existing wire helpers/budgets/errors remain unchanged.

Original stage1 receipts outside repository preserve tool-returned canonical IDs;
this addendum records actual IDs subsequently independently recovered by root:
- correctness01a134a0-55a2-77a1-b1cb-163b41a28c0d,
  final prep turn01a134a0-55e1-7aa0-9ee1-5bf40298e055;
- adversarial01a134a0-822d-7712-8827-df899b51e806,
  final prep turn01a134a3-cf9b-7c03-b9a5-131b77faae99;
- own architect01a1349d-c963-7922-9c2a-d209ab1689a6,
  final prep turn01a134a5-f79f-7fd1-a5cc-f1bf60097984.
SAME agents retained for actual combined source and eventual completed original CI.
All approved stage1 CONTRACTb21babf1602a42561391f12d090b6d61e5a85c7033248155bd8c73f1f93780af,
FIXTURESc44057b6cf511c01d3eb2025bbadf2861aba14f149ce6347fbc819179fe39f23,
DRAFTcc5fb821d3757980a1358540e3884e1ab10a60b949b236e038b4e0558251fd02,
and the four hex hashes recorded in fixtures/metadata_class_header/README.md.

Source implements only full top-level Class scalar-header scanning, borrowing raw
bytes and retaining signed int32/presence/last-recognized semantics. Missing fq_name
is checked at clean EOF; wire and dispatch errors precede it. One reused budget
charges bytes+2*fields; no root/envelope/nested scan or output allocation. Nested,
packed, wrong-wire and extension contents remain opaque. No compiler/Class full
initialization, names, declarations, linkage, server consumer or semantic parity.
Caller k1/mv[2,3,0] gating and header/d1/payload/name budgets remain separate.

Ten integration tests authored for future unchanged Linux Native Rust CI: fixture
identity/exact limits; presence/default/duplicates/wrongwire; signed/nonminimal bits;
unknown wires/opaque bodies; malformed tails; every-byte cuts; work/error precedence;
hard clamps; Unit composition; deterministic hostile probes. No runtime result yet.
Unchanged header/wire/table/payload/resolver tests remain regression coverage.

Local permitted checks completed: cargo fmt --all, then
`cargo fmt --all -- --check` exited0; compile-only
`cargo clippy --locked --workspace --all-targets -- -D warnings` exited0,
dev check finished4.69s. Logs retained outside repository at
/private/tmp/ktlsp-class-header-contract-01a1349d/stage2-fmt-check.log and
stage2-clippy.log. fmt log is empty; actual tool completion reports exit0.
All SAME3 held-source readbacks found no actionable source/test findings. Each
manually checked all10 tests, wire helper/accounting and exact frozen bytes/notices.
Final committed-identity approvals are intentionally outside this precommit record;
root must read actual final messages/receipts before admitting any integration.
No Cargo test/parser/worker/server/native runtime/bench/JVM/oracle/Gradle/Maven.
Push/PR/CI/final integration/merge held for root's future single assigned base after
enum144 delivery. No fresh executor, budget relaxation or retries to green.
Parents#68/#18 OPEN, #7/#83 OPEN, T008 HOLD, B3/B4 GAP, B11 UNVERIFIED,
#96 UNKNOWN, historical audit45/32/10/3 unchanged; no RSS/performance/full parity claim.
