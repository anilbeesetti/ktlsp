# LineNumberTable fixture data

`authored.json` and `EXPECTATIONS.md` freeze independent literal payloads, malformed cases, all proper prefixes, exact per-call arithmetic, public forged-model and hostile cases. They predate the decoder implementation. Broader table shapes are authored, never compiler observations.

The genuine fixture is reused unchanged from `../../../fixtures/t017c2/Unit.class`, with its existing `provenance.json`, Apache-2.0 `LICENSE.txt` and `NOTICE.txt`. SHA256 `896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce`, 781 bytes. The distribution is Kotlin stdlib2.3.20; compiler provenance is not independently verified. It contains just two single-entry tables: init(0,13), toString(0,14). No table occurs in clinit.

No new artifact was downloaded; no JVM/compiler/oracle generated expectations. Native default tests remain Rust-only. This subset checks encoded data/count/bounds and resource accounting, without instruction-boundary/PC lookup/source association, stepping or semantic parity. Parsing and Code framing remain separately budgeted. No whole-pipeline/RSS guarantee follows.
