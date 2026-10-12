//! Compares every `KDoc` subtree in the PSI fixtures with `parse_kdoc` of the matching
//! `DOC_COMMENT` token (matched by order of appearance), independently of the Kotlin parser port.

use std::fs;
use std::path::{Path, PathBuf};

use ktrs_syntax::{Parse, SyntaxKind, Tree, TreeBuilder, psi_dump};

use super::parse_kdoc;

#[test]
fn kdoc_subtrees_match_fixtures() {
    let psi_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/kotlin/psi");
    let mut fixtures = Vec::new();
    collect_fixtures(&psi_dir, &mut fixtures);
    fixtures.sort();

    let (mut total, mut failures) = (0, Vec::new());
    for source in &fixtures {
        let expected_dump = fs::read_to_string(source.with_extension("txt")).unwrap().replace("\r\n", "\n");
        let expected = kdoc_subtrees(&expected_dump);
        if expected.is_empty() {
            continue;
        }
        let text = fs::read_to_string(source).unwrap().replace("\r\n", "\n");
        let comments = doc_comments(text.trim_end_matches('\n'));
        let name = source.strip_prefix(&psi_dir).unwrap().display().to_string();
        if comments.len() != expected.len() {
            failures.push(format!("{name}: {} DOC_COMMENT tokens vs {} KDoc nodes", comments.len(), expected.len()));
            continue;
        }
        for (i, (comment, expected)) in comments.iter().zip(&expected).enumerate() {
            total += 1;
            let actual = dump_kdoc(comment);
            if &actual != expected {
                failures.push(format!("{name} #{i}\n--- expected\n{expected}\n--- actual\n{actual}"));
            }
        }
    }
    println!("KDoc subtrees: {}/{total} match", total - failures.len());
    assert!(failures.is_empty(), "{} mismatch(es):\n{}", failures.len(), failures.join("\n\n"));
}

fn collect_fixtures(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_fixtures(&path, out);
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("kt" | "kts"))
            && path.with_extension("txt").is_file()
        {
            out.push(path);
        }
    }
}

fn doc_comments(text: &str) -> Vec<&str> {
    let mut offset = 0;
    let mut out = Vec::new();
    for token in ktrs_lexer::tokenize(text) {
        let end = offset + token.len as usize;
        if token.kind == SyntaxKind::DOC_COMMENT {
            out.push(&text[offset..end]);
        }
        offset = end;
    }
    out
}

/// Each `KDoc` node of a dump with its descendants, re-indented to depth 0.
fn kdoc_subtrees(dump: &str) -> Vec<String> {
    let lines: Vec<&str> = dump.trim_end().lines().collect();
    let indent_of = |line: &str| line.len() - line.trim_start_matches(' ').len();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start() != "KDoc" {
            continue;
        }
        let depth = indent_of(line);
        let end = lines[i + 1..].iter().position(|l| indent_of(l) <= depth).map_or(lines.len(), |p| i + 1 + p);
        out.push(lines[i..end].iter().map(|l| &l[depth..]).collect::<Vec<_>>().join("\n"));
    }
    out
}

fn dump_kdoc(text: &str) -> String {
    let parse = parse_kdoc(text);
    let mut tree = TreeBuilder::new();
    tree.start_node(SyntaxKind::DOC_COMMENT);
    tree.start_node(SyntaxKind::DOC_COMMENT);
    for child in parse.tree.children(Tree::ROOT) {
        tree.push_subtree(&parse.tree, child);
    }
    tree.finish_node();
    tree.finish_node();
    let dump = psi_dump(&Parse { tree: tree.finish().into(), error_messages: parse.error_messages, missed_tokens: Vec::new() }, "");
    dump.lines().skip(1).map(|l| &l[2..]).collect::<Vec<_>>().join("\n")
}
