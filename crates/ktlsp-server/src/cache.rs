use crate::structure::{self, Analysis};
use ktlsp_text::Document;

/// One most-recent analysis: bounded outputs, no retained source or tokens.
#[derive(Debug, Default)]
pub(crate) struct Cache {
    entry: Option<Entry>,
    #[cfg(test)]
    analyses: usize,
}

#[derive(Debug)]
struct Entry {
    uri: String,
    version: i32,
    fold_limit: usize,
    result: Result<Analysis, &'static str>,
}

impl Cache {
    pub fn invalidate(&mut self, uri: &str) {
        if self.entry.as_ref().is_some_and(|entry| entry.uri == uri) {
            self.entry = None;
        }
    }

    pub fn get(
        &mut self,
        uri: &str,
        document: &Document,
        fold_limit: usize,
    ) -> Result<&Analysis, &'static str> {
        // Session invalidates only after committed edits and on close/reopen;
        // its private document store makes URI/version an exact content identity.
        if !self.entry.as_ref().is_some_and(|entry| {
            entry.uri == uri
                && entry.version == document.version()
                && entry.fold_limit == fold_limit
        }) {
            self.entry = None;
            let result = structure::analyze(document.text(), fold_limit);
            #[cfg(test)]
            {
                self.analyses += 1;
            }
            self.entry = Some(Entry {
                uri: uri.into(),
                version: document.version(),
                fold_limit,
                result,
            });
        }
        self.entry
            .as_ref()
            .expect("cached analysis")
            .result
            .as_ref()
            .map_err(|error| *error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_OPEN_BYTES, MAX_OPEN_DOCUMENTS, Session};
    use serde_json::{Value, json};

    const URI: &str = "untitled:cached";

    fn notify(session: &mut Session, method: &str, params: Value) -> Vec<Value> {
        session.handle(json!({"jsonrpc":"2.0","method":method,"params":params}))
    }
    fn running() -> Session {
        let mut session = Session::new();
        session.handle(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "capabilities":{"textDocument":{"publishDiagnostics":{"versionSupport":true}}}
            }}),
        );
        notify(&mut session, "initialized", json!({}));
        session
    }
    fn open(session: &mut Session, uri: &str, text: &str) -> Vec<Value> {
        notify(
            session,
            "textDocument/didOpen",
            json!({"textDocument":{
                "uri":uri,"languageId":"kotlin","version":1,"text":text
            }}),
        )
    }
    fn change(session: &mut Session, version: i32, changes: Value) -> Vec<Value> {
        notify(
            session,
            "textDocument/didChange",
            json!({
                "textDocument":{"uri":URI,"version":version},"contentChanges":changes
            }),
        )
    }
    fn folding(session: &mut Session, uri: &str) -> Value {
        session
            .handle(
                json!({"jsonrpc":"2.0","id":2,"method":"textDocument/foldingRange",
                    "params":{"textDocument":{"uri":uri}}
                }),
            )
            .remove(0)
    }

    #[test]
    fn diagnostics_and_folding_share_one_analysis_until_a_committed_change() {
        let mut session = running();
        let diagnostics = open(&mut session, URI, "\"😀\"; {\r\n}\r]");
        assert_eq!(diagnostics[0]["params"]["version"], 1);
        assert_eq!(
            diagnostics[0]["params"]["diagnostics"][0]["range"],
            json!({
                "start":{"line":2,"character":0},"end":{"line":2,"character":1}
            })
        );
        let expected = json!([{"startLine":0,"endLine":1,"startCharacter":6,
            "endCharacter":1,"kind":"region"}]);
        assert_eq!(folding(&mut session, URI)["result"], expected);
        assert_eq!(folding(&mut session, URI)["result"], expected);
        assert_eq!(session.analysis_cache.analyses, 1);

        let diagnostics = change(&mut session, 2, json!([{"text":"(\n)"}]));
        assert_eq!(diagnostics[0]["params"]["diagnostics"], json!([]));
        assert_eq!(diagnostics[0]["params"]["version"], 2);
        assert_eq!(
            folding(&mut session, URI)["result"],
            json!([
                {"startLine":0,"endLine":1,"startCharacter":0,"endCharacter":1,"kind":"region"}
            ])
        );
        assert_eq!(session.analysis_cache.analyses, 2);

        // Even a version-only accepted edit establishes a new exact document identity.
        change(&mut session, 3, json!([]));
        folding(&mut session, URI);
        assert_eq!(session.analysis_cache.analyses, 3);
    }

    #[test]
    fn failed_edits_and_duplicate_open_preserve_the_cached_document() {
        let mut session = running();
        open(&mut session, URI, "{\n}");
        let expected = folding(&mut session, URI);
        assert!(open(&mut session, URI, "different").is_empty());
        assert!(change(&mut session, 1, json!([{"text":"stale"}])).is_empty());
        assert!(change(&mut session, 2, json!([
            {"text":"\"😀\""},
            {"range":{"start":{"line":0,"character":2},"end":{"line":0,"character":2}},"text":"x"}
        ])).is_empty());
        assert_eq!(session.document(URI).unwrap().text(), "{\n}");
        assert_eq!(session.document(URI).unwrap().version(), 1);
        assert_eq!(folding(&mut session, URI), expected);
        assert_eq!(session.analysis_cache.analyses, 1);
        assert_eq!(session.drain_errors().len(), 3);
    }

    #[test]
    fn aggregate_limit_failure_preserves_the_cached_document() {
        let mut session = running();
        let full = " ".repeat(ktlsp_text::MAX_DOCUMENT_BYTES);
        for index in 0..MAX_OPEN_BYTES / full.len() {
            assert_eq!(
                open(&mut session, &format!("untitled:full{index}"), &full).len(),
                1
            );
        }
        open(&mut session, URI, "");
        let count = session.analysis_cache.analyses;
        assert!(change(&mut session, 2, json!([{"text":"x"}])).is_empty());
        assert_eq!(session.open_bytes(), MAX_OPEN_BYTES);
        assert_eq!(session.document(URI).unwrap().version(), 1);
        assert_eq!(session.document(URI).unwrap().text(), "");
        assert_eq!(folding(&mut session, URI)["result"], json!([]));
        assert_eq!(session.analysis_cache.analyses, count);
        assert_eq!(
            session.drain_errors(),
            ["aggregate open document size limit exceeded"]
        );
    }

    #[test]
    fn close_and_reopen_same_version_never_reuse_old_offsets() {
        let mut session = running();
        open(&mut session, URI, "{\n}");
        let closed = notify(
            &mut session,
            "textDocument/didClose",
            json!({"textDocument":{"uri":URI}}),
        );
        assert_eq!(closed[0]["params"]["diagnostics"], json!([]));
        assert!(closed[0]["params"].get("version").is_none());
        assert!(session.analysis_cache.entry.is_none());
        assert_eq!(folding(&mut session, URI)["error"]["code"], -32602);
        let reopened = open(&mut session, URI, "]");
        assert_eq!(
            reopened[0]["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(folding(&mut session, URI)["result"], json!([]));
        assert_eq!(session.analysis_cache.analyses, 2);
    }

    #[test]
    fn analysis_configuration_keys_and_render_configuration_stay_current() {
        let mut session = running();
        open(&mut session, URI, "{\n}\n[\n]");
        assert_eq!(
            folding(&mut session, URI)["result"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        session.configuration.folding_range_limit = 1;
        assert_eq!(
            folding(&mut session, URI)["result"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(session.analysis_cache.analyses, 2);
        session.configuration.line_folding_only = true;
        assert_eq!(
            folding(&mut session, URI)["result"],
            json!([
                {"startLine":0,"endLine":1,"kind":"region"}
            ])
        );
        assert_eq!(session.analysis_cache.analyses, 2);
        session.configuration.folding_range_limit = 0;
        assert_eq!(folding(&mut session, URI)["result"], json!([]));
        assert_eq!(session.analysis_cache.analyses, 3);
    }

    #[test]
    fn many_documents_evict_to_one_bounded_result_and_errors_are_cached() {
        let mut session = running();
        let text =
            "{\n}\n".repeat(structure::MAX_FOLDS + 1) + &")".repeat(structure::MAX_DIAGNOSTICS + 1);
        open(&mut session, URI, &text);
        let entry = session.analysis_cache.entry.as_ref().unwrap();
        let analysis = entry.result.as_ref().unwrap();
        let (folds, diagnostics) = analysis.render(&text, false);
        assert_eq!(folds.len(), structure::MAX_FOLDS);
        assert_eq!(diagnostics.len(), structure::MAX_DIAGNOSTICS);
        assert_eq!(analysis.diagnostics(&text), diagnostics);
        for index in 1..MAX_OPEN_DOCUMENTS {
            let uri = format!("untitled:other{index}");
            open(&mut session, &uri, "{\n}");
            folding(&mut session, &uri);
            assert_eq!(session.analysis_cache.entry.as_ref().unwrap().uri, uri);
        }
        assert_eq!(session.open_document_count(), MAX_OPEN_DOCUMENTS);
        assert_eq!(session.analysis_cache.analyses, MAX_OPEN_DOCUMENTS);
        assert_eq!(
            folding(&mut session, URI)["result"]
                .as_array()
                .unwrap()
                .len(),
            structure::MAX_FOLDS
        );
        assert_eq!(session.analysis_cache.analyses, MAX_OPEN_DOCUMENTS + 1);
        assert_eq!(session.analysis_cache.entry.as_ref().unwrap().uri, URI);

        let output = change(
            &mut session,
            2,
            json!([{"text":"{".repeat(structure::MAX_DEPTH + 1)}]),
        );
        assert_eq!(
            output[0]["params"]["diagnostics"][0]["code"],
            "analysis-limit"
        );
        assert_eq!(folding(&mut session, URI)["error"]["code"], -32803);
        assert_eq!(session.analysis_cache.analyses, MAX_OPEN_DOCUMENTS + 2);
        change(&mut session, 3, json!([{"text":"{}"}]));
        assert_eq!(folding(&mut session, URI)["result"], json!([]));
        assert_eq!(session.analysis_cache.analyses, MAX_OPEN_DOCUMENTS + 3);
    }
}
