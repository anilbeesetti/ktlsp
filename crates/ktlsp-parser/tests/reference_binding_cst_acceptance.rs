//! Production CST capture only; reference-site and semantic acceptance are separate.
use ktlsp_parser::{FileKind, Parsed, Parser, SourceSnapshot};
use std::{io::Write, sync::atomic::AtomicBool};

// Frozen capture ceilings; these do not bound parser memory, stack or cleanup time.
const MAX_ELEMENTS: usize = 512;
const MAX_DIAGNOSTICS: usize = 16;
const MAX_KIND_BYTES: usize = 64;
const MAX_MESSAGE_BYTES: usize = 4096;
const MAX_CAPTURE_BYTES: usize = 1_048_576;

fn main() {
    check_output_bound();
    let source = include_str!("../../../docs/specs/reference-binding-acceptance/same-block.kt");
    assert_eq!(source, "fun f() { val x = 1; val y = x }\n");
    let snapshot =
        SourceSnapshot::new("reference-binding/same-block", 1, FileKind::Source, source).unwrap();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    // One request, without retries or a prediction of the reference node kind.
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap_or_else(|error| panic!("same-block: retained parse failure: {error:?}"));
    assert_eq!(result.snapshot(), &snapshot);
    let tree = result.tree_for(&snapshot).unwrap();
    assert_eq!(tree.source(), snapshot.source());
    capture(tree);
}

struct BoundedOutput<W> {
    output: W,
    bytes: usize,
}

impl<W: Write> Write for BoundedOutput<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .bytes
            .checked_add(bytes.len())
            .filter(|&n| n <= MAX_CAPTURE_BYTES)
            .ok_or_else(|| std::io::Error::other("retained reference capture overflow"))?;
        self.output.write_all(bytes)?;
        self.bytes = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}

fn check_output_bound() {
    let mut output = BoundedOutput {
        output: std::io::sink(),
        bytes: MAX_CAPTURE_BYTES - 1,
    };
    output.write_all(b"x").unwrap();
    assert_eq!(output.bytes, MAX_CAPTURE_BYTES);
    assert!(output.write_all(b"x").is_err());
    assert_eq!(output.bytes, MAX_CAPTURE_BYTES);
    output.bytes = usize::MAX;
    assert!(output.write_all(b"x").is_err());
    assert_eq!(output.bytes, usize::MAX);
}

fn capture(tree: &Parsed) {
    assert!(
        tree.elements().len() <= MAX_ELEMENTS,
        "retained reference element capture overflow"
    );
    assert!(
        tree.diagnostics().len() <= MAX_DIAGNOSTICS,
        "retained reference diagnostic capture overflow"
    );
    let mut output = BoundedOutput {
        output: std::io::stdout().lock(),
        bytes: 0,
    };
    writeln!(
        output,
        "REFERENCE_BINDING_CST_BEGIN same-block document=reference-binding/same-block revision=1 kind=Source source_bytes={} elements={} diagnostics={}",
        tree.source().len(),
        tree.elements().len(),
        tree.diagnostics().len()
    )
    .unwrap();
    for (index, e) in tree.elements().iter().enumerate() {
        assert!(
            e.kind.len() <= MAX_KIND_BYTES,
            "retained reference kind capture overflow"
        );
        serde_json::to_writer(
            &mut output,
            &serde_json::json!({
                "element": index, "kind": e.kind, "token": e.token,
                "start": e.start, "end": e.end, "parent": e.parent, "subtree_end": e.subtree_end,
            }),
        )
        .unwrap();
        writeln!(output).unwrap();
    }
    for (index, d) in tree.diagnostics().iter().enumerate() {
        assert!(
            d.message.len() <= MAX_MESSAGE_BYTES,
            "retained reference message capture overflow"
        );
        serde_json::to_writer(
            &mut output,
            &serde_json::json!({
                "diagnostic": index, "start": d.start, "end": d.end, "message": d.message,
            }),
        )
        .unwrap();
        writeln!(output).unwrap();
    }
    writeln!(
        output,
        "REFERENCE_BINDING_CST_END same-block elements={} diagnostics={}",
        tree.elements().len(),
        tree.diagnostics().len()
    )
    .unwrap();
    output.flush().unwrap();
}
