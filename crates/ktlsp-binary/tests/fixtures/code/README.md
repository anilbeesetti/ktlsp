# Code framing fixture data

`authored.json` is the unchanged, independently prepared four valid payloads and nineteen malformed patches. `EXPECTATIONS.md` freezes their interpretation, prefixes, error order, aggregate arithmetic and hostile-input cases. Tests copy these literal strings/patches directly; no JSON dependency or generated parser is needed. Expectations were prepared and independently reviewed before the implementation, not derived from decoder output.

Nonempty exception tables are independently authored data, never described as compiler output or a genuine observation. The one genuine fixture is reused unchanged from `../../../fixtures/t017c2/Unit.class`; its existing provenance.json, Apache-2.0 LICENSE.txt and NOTICE.txt remain with it. Its SHA-256 is `896588212a98f36148f2fbe1ea2878d6ba23c6376c6346c765cec5ac0c5716ce` (781 bytes). All three genuine exception tables are empty.

No new artifacts were downloaded, and no JVM/compiler/oracle created these inputs. Native default tests remain Rust-only. This corpus tests framing/data limits, not instruction validity, StackMapTable, JVM verification or semantic parity.
