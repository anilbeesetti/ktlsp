# Native replay evidence

T045a child [#59](https://github.com/anilbeesetti/ktlsp/issues/59) compares the existing decoded observations using [the optional standalone Rust tool](../../../../tools/oracle/README.md). Original transcripts/source/metadata are untouched. No official JVM ran on this macOS host and no new archive was downloaded. The original archive is Linux amd64; its recorded official run was separate.

`lifecycle-folding-native-baseline.json`: native source revision `0d54973ce66f59362829360261fde6e46b4fc420`, built with pinned Rust 1.99.0, macOS, 2026-10-12. Exact command appears in the report; reproduce using the tool README. The declared revision identifies the native source only; tool source is this child PR. No URI/ID/semantic normalization.

| Recorded row | Native comparison |
| --- | --- |
| initialize ID 1 | Mismatch: capability/provider omissions, save advertisement, explicit UTF-16 field, server identity; all JSON Pointer differences retained |
| folding ID 2 | Mismatch: native method-not-found `-32601`; official one region at lines 0–2, characters 11–1 with collapsedText `{...}` |
| shutdown ID 3 | Exact response match (`result: null`) |
| exit status | Recorded official 0 equals native 0 |
| notifications | Seven captured official notifications versus none native; original complete shutdown tail unverified |

Matching shutdown/exit establishes only those recorded rows. It does not certify lifecycle, framing, document edits or feature parity. The native server's bounded framing is native-tested; original wire bytes were not recorded. Standalone declaration/hover/hierarchy observations are accepted by the loader but not published as supported native semantic comparisons. No filesystem imports, severity/range/edit/ranking corpus, cancellation, async interactions, performance or fresh official run was verified. Folding remains baseline mismatch until T006 is merged and separately replayed.

SHA256 provenance for this baseline:

| Input/artifact | SHA256 |
| --- | --- |
| original lifecycle transcript.json | `53a4b7eff2abf1ca9741e0d3538590f3a0244e83873366b98a9b8aada9cc07a7` |
| original lifecycle metadata.json | `f2147c77bef1106a6306f5d871bfaf741cc0a3b51c462485c56a261c8aedb8f9` |
| original lifecycle Main.kt | `f1a8b52ae5177312536d5f54c662faf932b06b3708825a3098faeaa49e26a974` |
| native debug executable on this host | `b58fdc87f321eecc59e779479b6610122dac804c6a341e651c4a3982e33d15eb` |

Hashes were measured with `shasum -a 256`; native executable hash is host/build-specific and not a reproducible-build claim. The tool validates metadata shape; it does not authenticate a release download or compute these hashes itself. The original official archive checksum/provenance remains in fixture metadata.
