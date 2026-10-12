# T002 bounded framing review evidence

Implementation reviewed on 2026-10-12 at commit `197c739e79fcffbdbba4d7146664a69a09061f56`.

The correctness/functionality reviewer independently inspected the implementation and exercised five temporary tests: 256 generated binary frame round trips across eight buffer sizes, exact reader offsets, every frame truncation point, ASCII header grammar and malformed delimiters, and an exactly 8 KiB header with a long zero-prefixed decimal. All independent tests, the 11 permanent transport tests, and protocol clippy passed. No actionable P0/P1/P2 findings.

The adversarial/edge/crash reviewer independently exercised five temporary tests: every single-byte mutation of a valid header with one-byte and 13-byte buffers, generated hostile headers and cap boundaries, subsequent-frame preservation, invalid headers rejected before payload consumption, repeated interrupted fragmented reads, and zero writes after partial header/body output. Independent and permanent transport tests passed. No actionable P0/P1/P2 findings.

Temporary reviewer tests were removed after execution. Permanent regressions remain in `tests/framing.rs`; local workspace format, clippy with warnings denied, and tests passed under Rust 1.99.0. Integration and hosted CI results are recorded in the PR.

This evidence covers bounded byte framing only. JSON-RPC parsing/session lifecycle, URI/document handling, and official Kotlin server differential parity are outside T002. Framing failures are fatal for the current stream; callers must close it. Writers must serialize output and choose flush timing. Payload bytes are not interpreted as JSON or UTF-8 by this layer.
