# Historical benchmark provenance erratum — 2026-10-13

This additive correction concerns [PR #77](https://github.com/anilbeesetti/ktlsp/pull/77) / [child #64](https://github.com/anilbeesetti/ktlsp/issues/64), delivered as `3bd6fe67adc8156b96c22afddc4bbf36f0fd3937`. The original measurement directory was a mixed, incomplete source archive. The [result JSON](results/macos-arm64-0d54973.json) `metadata.source_preparation` wording (“copied reviewed tools/bench source0b43c7f”), the benchmark/results READMEs' unqualified “harness source” attribution, and the coordinator's earlier whole-archive-match statement were too broad. This note supersedes their whole-workspace interpretations; it does not rewrite the original evidence.

All **53 product files** match product checkpoint `0d54973ce66f59362829360261fde6e46b4fc420` byte-for-byte. The measured harness `src/main.rs`, `src/process.rs`, `Cargo.toml` and `Cargo.lock`, plus its `.gitignore`, match `0b43c7f15db440437e6f1cd08d8affa79d701358`. Thus that harness revision identifies the measured executable inputs, not every file in the measurement directory. Source correspondence and saved binary hashes do not establish reproducible builds or independently verify the binaries' build provenance.

Exactly three ancillary files instead match earlier commit `c922a9fd68e4a370d795d094bcb222f35eed3765`:

| Archived path | Git blob | SHA-256 |
| --- | --- | --- |
| `tools/bench/README.md` | `bfa40748552acf344c9451bad1e29448d631209e` | `111d40f051d608cb19003f34995024c3e92c984d7e5c545a18f65be20e7ea8ec` |
| `tools/bench/src/bin/bench-test-peer.rs` | `29051f08437d85c1724ca22bdee323d21a9c76d5` | `f8fe017a02393e7b51c43672cfe09ef16546992478078e774b673f230117e782` |
| `tools/bench/tests/subprocess.rs` | `9ecbb4cb6e6a454dc604cd0d93b3de2a8d7eeeb6` | `c32b83e2b697f03ea6c8cacd4d935983adcc1693045a3af731aeaf5d609e9d9e` |

The exact Git inventories contain **62 intended files**: 53 product files at `0d54973` and nine harness files at `0b43c7f`. The original directory and faithful preservation tar contain **61 unique files**, no extras: **58 intended-revision matches**, the three earlier-revision matches above, and missing `tools/bench/REVIEW.md` (intended blob `1cb1ddeade0071b05fc2fc85da2c0934807b6560`). Present-file matching alone did not establish completeness. Neither archive is a complete, coherent `0b43c7f` harness workspace.

The recorded build selected `--bin ktlsp-baseline`. Its [main source](https://github.com/anilbeesetti/ktlsp/blob/0b43c7f15db440437e6f1cd08d8affa79d701358/tools/bench/src/main.rs) imports `process.rs`; the peer under `src/bin/` is a separate binary, and `tests/subprocess.rs` is a separate integration-test target. Neither older Rust file is linked into the measured executable. The older tests lack the later RSS/ECHILD regressions and retain an earlier reaping assertion, so the mixed archive cannot inherit the complete later suite's passing claim.

Historical standalone eight-test evidence belongs to `/Users/anil/.codex/worktrees/d729/ktlsp`: initial RSS checks preceded committing `0b43c7f`, and the later full check ran at clean integrated checkout `b4836508d686dcc7bd190a84621aacc29fde19ae`. Recorded command output shows three harness tests and five subprocess tests passing there; immediate Git readback identifies that checkout and no benchmark diff from `0b43c7f`. Those suite results do not validate the mixed measurement archive. The recorded measurement's built-in correctness checks are separate from the standalone suite.

The prerequisite-free preservation Git bundle was independently recovered into an empty object database. It supplies all 62 intended source files, including the missing review note and intended versions of the three ancillary files, and retains commits `c922a9fd68e4a370d795d094bcb222f35eed3765`, `0d54973ce66f59362829360261fde6e46b4fc420`, `0b43c7f15db440437e6f1cd08d8affa79d701358`, `b4836508d686dcc7bd190a84621aacc29fde19ae`, reviewed head `b2df7b0741435ead12e271bdab79f0886c407882`, and merged `3bd6fe67adc8156b96c22afddc4bbf36f0fd3937`. The mixed archive remains original evidence; the bundle is separate preservation, not a repaired measurement archive.

Original SHA-256 identities remain:

| Evidence | SHA-256 |
| --- | --- |
| [Result JSON](results/macos-arm64-0d54973.json) | `291a83aeeefd65b0d39b3a8d8fd520b0ea85e6ed6ec72683e67c3cfc50dba93e` |
| [Window JSON](results/macos-arm64-0d54973.window.json) | `47b4e6a4cfe22065721958d09e861db6200e8fcd323df004d87f9d15742fd5c1` |
| Product binary | `034c1111ea538dc57da2765b35475ed9c14a47d00854949e9ea94bd055f72aad` |
| Harness binary | `60eeb21565a1c73ff9798ee4cb6c89347a2d36e002dff1ca89024268000a2c3d` |

The original JSON, samples, window observations, binaries, archive and historical Git records are unchanged. No measurement was rerun or retargeted. PR #77's [exact merged-main Linux CI](https://github.com/anilbeesetti/ktlsp/actions/runs/38331585370/job/115082290551) passed the three pinned native workspace commands at `3bd6fe67`: 226 passed, zero failed, two intentionally ignored subprocess helpers, plus 19 separate worker fault cases. That workflow excludes the standalone benchmark and optional oracle workspaces; it does not validate Darwin RSS or the mixed archive's standalone tests. This correction establishes no current-main performance, official-server superiority, reliability fix or full parity. Parent [#44](https://github.com/anilbeesetti/ktlsp/issues/44) stays open; [#96](https://github.com/anilbeesetti/ktlsp/issues/96) stays open/unknown.
