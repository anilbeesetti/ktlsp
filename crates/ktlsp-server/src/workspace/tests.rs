//! Authored filesystem projects exercising the real importer and workspace state.
//! No Session, parser, worker or external build-tool runtime is used here.
use super::{Project, Snapshot, Workspace};
use crate::file_uri;
use ktlsp_import::source_inventory::Budget;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = parent.join(format!(
            "ktlsp-421-state-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Failure to create this exclusive child never authorizes deleting it.
        fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        fixture.model();
        fixture.put("src/A.kt", b"fun diskA() = 1\n");
        fixture.put("src/B.kt", b"fun diskB() = 2\n");
        fixture
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn model(&self) {
        let model = json!({
            "modules": [{
                "name": "app",
                "dependencies": [{"type": "moduleSource"}],
                "contentRoots": [{
                    "path": "<WORKSPACE>/",
                    "sourceRoots": [{"path": "<WORKSPACE>/src", "type": "kotlin-source"}]
                }]
            }],
            "libraries": []
        });
        self.put("workspace.json", &serde_json::to_vec(&model).unwrap());
    }

    fn uri(&self, name: &str) -> String {
        file_uri::encode(&self.0.join(name)).unwrap()
    }

    fn reload(&self, workspace: &mut Workspace, documents: &[(&str, u64)]) -> Result<(), String> {
        let (_, generation) = workspace.begin(false).map_err(str::to_owned)?;
        let snapshot = Snapshot::load(
            vec![Project::Json(self.0.clone())],
            generation.unwrap(),
            &AtomicBool::new(false),
        )?;
        workspace
            .commit(snapshot, documents.iter().copied())
            .map_err(str::to_owned)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn alias(uri: &str) -> String {
    // Both forms are native codec aliases on Unix and Windows; no physical
    // case-folding, symlink or foreign-platform path assumptions are involved.
    uri.replacen("file://", "FILE://localhost", 1)
        .replace("/A.kt", "/%41.kt")
}

#[test]
fn captured_historical_generation_refuses_third_until_released() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let (first, _, _, _) = workspace.capture(&uri).unwrap();
    let first_epoch = workspace.epoch;

    fixture.put("src/A.kt", b"fun second() = 2\n");
    fixture.reload(&mut workspace, &[]).unwrap();
    let (second, _, _, second_source) = workspace.capture(&uri).unwrap();
    let second_epoch = workspace.epoch;
    assert_eq!(second_source.as_ref(), "fun second() = 2\n");
    assert!(!workspace.fresh(&first, first_epoch));
    assert_eq!(first.capture(&uri).unwrap().2.as_ref(), "fun diskA() = 1\n");
    assert!(workspace.fresh(&second, second_epoch));

    fixture.put("src/A.kt", b"fun third() = 3\n");
    assert!(workspace.begin(false).is_err());
    assert!(workspace.capture(&uri).is_err());
    assert!(!workspace.fresh(&second, second_epoch));
    assert_eq!(
        second.capture(&uri).unwrap().2.as_ref(),
        "fun second() = 2\n"
    );

    drop(first);
    fixture.reload(&mut workspace, &[]).unwrap();
    let (third, _, _, source) = workspace.capture(&uri).unwrap();
    assert_eq!(source.as_ref(), "fun third() = 3\n");
    assert!(workspace.fresh(&third, workspace.epoch));
    // The second request is still retained: admission was restored by releasing
    // the historical whole model, not by discarding every in-flight capture.
    assert_eq!(
        second.capture(&uri).unwrap().2.as_ref(),
        "fun second() = 2\n"
    );
    assert!(!workspace.fresh(&second, workspace.epoch));
}

#[test]
fn known_alias_routes_to_overlay_and_accepted_edits_stale_closed_work() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let alias = alias(&uri);
    let other = fixture.uri("src/B.kt");
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let (before_open, _, _, _) = workspace.capture(&other).unwrap();
    let before_open_epoch = workspace.epoch;

    workspace.check_open(&alias).unwrap();
    workspace.opened(&alias, 11);
    assert!(!workspace.fresh(&before_open, before_open_epoch));
    assert_eq!(workspace.overlay_uri(&uri), Some(alias.as_str()));
    assert_eq!(workspace.overlay_uri(&alias), Some(alias.as_str()));
    assert!(workspace.check_open(&uri).is_err());
    assert!(workspace.check_open(&alias).is_err());
    assert_eq!(workspace.overlay_uri(&other), None);

    let (before_edit, _, _, _) = workspace.capture(&other).unwrap();
    let before_edit_epoch = workspace.epoch;
    workspace.changed(&alias);
    assert!(!workspace.fresh(&before_edit, before_edit_epoch));
    assert_eq!(workspace.overlay_uri(&uri), Some(alias.as_str()));
    let (after_edit, _, _, _) = workspace.capture(&other).unwrap();
    assert!(workspace.fresh(&after_edit, workspace.epoch));
}

#[test]
fn close_reopen_invalidates_incarnation_and_requires_original_disk_validation() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let alias = alias(&uri);
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    workspace.check_open(&alias).unwrap();
    workspace.opened(&alias, 21);
    let (before_close, _, _, _) = workspace.capture(&uri).unwrap();
    let before_close_epoch = workspace.epoch;
    workspace.closed(&alias, 21);
    assert!(!workspace.fresh(&before_close, before_close_epoch));
    assert_eq!(workspace.overlay_uri(&uri), None);
    let (disk, key, canonical, source) = workspace.capture(&alias).unwrap();
    let disk_epoch = workspace.epoch;
    assert_eq!(canonical.as_ref(), uri);
    assert_eq!(source.as_ref(), "fun diskA() = 1\n");
    disk.verify(key, &mut Budget::default(), &AtomicBool::new(false))
        .unwrap();

    workspace.check_open(&uri).unwrap();
    workspace.opened(&uri, 22);
    workspace.closed(&alias, 21);
    assert_eq!(workspace.overlay_uri(&alias), Some(uri.as_str()));
    assert!(!workspace.fresh(&disk, disk_epoch));
    fixture.put("src/A.kt", b"fun changedOnDisk() = 3\n");
    workspace.closed(&uri, 22);
    assert_eq!(workspace.overlay_uri(&alias), None);
    let (old_disk, old_key, _, _) = workspace.capture(&uri).unwrap();
    assert!(
        old_disk
            .verify(old_key, &mut Budget::default(), &AtomicBool::new(false))
            .is_err()
    );

    fixture.reload(&mut workspace, &[]).unwrap();
    let (reloaded, key, _, source) = workspace.capture(&uri).unwrap();
    assert_eq!(source.as_ref(), "fun changedOnDisk() = 3\n");
    reloaded
        .verify(key, &mut Budget::default(), &AtomicBool::new(false))
        .unwrap();
    assert!(workspace.fresh(&reloaded, workspace.epoch));
}

#[test]
fn competing_aliases_refuse_candidate_without_republishing_last_good() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let alias = alias(&uri);
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let (last_good, _, _, _) = workspace.capture(&uri).unwrap();
    let epoch = workspace.epoch;

    assert!(
        fixture
            .reload(&mut workspace, &[(&uri, 31), (&alias, 32)])
            .is_err()
    );
    assert!(workspace.capture(&uri).is_err());
    assert!(!workspace.fresh(&last_good, epoch));
    assert_eq!(
        workspace
            .snapshot
            .as_ref()
            .unwrap()
            .capture(&uri)
            .unwrap()
            .2
            .as_ref(),
        "fun diskA() = 1\n"
    );
    assert_eq!(workspace.overlay_uri(&uri), None);

    fixture.reload(&mut workspace, &[(&alias, 32)]).unwrap();
    assert_eq!(workspace.overlay_uri(&uri), Some(alias.as_str()));
    assert!(workspace.check_open(&uri).is_err());
    workspace.closed(&alias, 32);
    assert!(workspace.capture(&uri).is_ok());
}

#[test]
fn unknown_file_overlays_block_until_last_close_but_custom_documents_do_not() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let unknown = fixture.uri("outside/never-created.kt");
    let other_unknown = fixture.uri("outside/also-absent.kt");
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let (before_open, _, _, _) = workspace.capture(&uri).unwrap();
    let epoch = workspace.epoch;

    workspace.opened("untitled:Scratch", 41);
    workspace.opened("custom:Scratch", 42);
    assert!(workspace.fresh(&before_open, epoch));
    assert!(workspace.capture(&uri).is_ok());
    workspace.opened(&unknown, 43);
    workspace.opened(&other_unknown, 44);
    assert!(workspace.capture(&uri).is_err());
    assert!(!workspace.fresh(&before_open, epoch));

    workspace.closed(&unknown, 43);
    assert!(workspace.capture(&uri).is_err());
    workspace.closed(&other_unknown, 44);
    let (restored, key, _, source) = workspace.capture(&uri).unwrap();
    assert_eq!(source.as_ref(), "fun diskA() = 1\n");
    restored
        .verify(key, &mut Budget::default(), &AtomicBool::new(false))
        .unwrap();
    assert!(workspace.fresh(&restored, workspace.epoch));
    assert!(!workspace.fresh(&before_open, epoch));
}

#[test]
fn failed_real_import_keeps_historical_model_unavailable_until_successful_reload() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let (last_good, _, _, _) = workspace.capture(&uri).unwrap();
    let epoch = workspace.epoch;

    fixture.put("workspace.json", b"{broken model");
    fixture.put("src/A.kt", b"fun replacement() = 4\n");
    assert!(fixture.reload(&mut workspace, &[]).is_err());
    assert!(workspace.capture(&uri).is_err());
    assert!(!workspace.fresh(&last_good, epoch));
    assert_eq!(
        workspace
            .snapshot
            .as_ref()
            .unwrap()
            .capture(&uri)
            .unwrap()
            .2
            .as_ref(),
        "fun diskA() = 1\n"
    );

    fixture.model();
    fixture.reload(&mut workspace, &[]).unwrap();
    let (current, _, _, source) = workspace.capture(&uri).unwrap();
    assert_eq!(source.as_ref(), "fun replacement() = 4\n");
    assert!(workspace.fresh(&current, workspace.epoch));
    assert!(!workspace.fresh(&last_good, workspace.epoch));
}

#[test]
fn revision_overflow_refuses_publication_and_cannot_be_restored_by_close() {
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    workspace.opened(&uri, 51);
    let (capture, _, _, _) = workspace.capture(&uri).unwrap();
    // Reach the otherwise impractical counter boundary without replacing any
    // inventory, source or overlay with test-only fabricated authority.
    workspace.epoch = u64::MAX;
    assert!(workspace.fresh(&capture, u64::MAX));
    workspace.changed(&uri);
    assert!(!workspace.fresh(&capture, u64::MAX));
    assert!(workspace.capture(&uri).is_err());
    workspace.closed(&uri, 51);
    assert!(workspace.capture(&uri).is_err());
    assert!(workspace.begin(false).is_err());
    assert_eq!(workspace.epoch, u64::MAX);
}

#[test]
fn aggregate_capture_authenticates_alias_content_version_and_reopen_incarnation() {
    use std::{collections::HashMap, sync::Arc};
    let fixture = Fixture::new();
    let uri = fixture.uri("src/A.kt");
    let alias = alias(&uri);
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let mut documents = HashMap::new();
    let mut incarnations = HashMap::new();
    documents.insert(
        alias.clone(),
        ktlsp_text::Document::new(2, "fun unsaved() = 9\n".into()).unwrap(),
    );
    incarnations.insert(alias.clone(), 61);
    workspace.opened(&alias, 61);
    let cancelled = AtomicBool::new(false);
    let capture = workspace
        .capture_all(
            &documents,
            &incarnations,
            &mut crate::workspace_symbols::Budget::default(),
            &cancelled,
        )
        .unwrap();
    assert_eq!(capture.members.len(), 2);
    let member = capture
        .members
        .iter()
        .find(|member| member.source.document() == uri)
        .unwrap();
    assert_eq!(member.source.source(), "fun unsaved() = 9\n");
    assert_eq!(member.source.revision(), 2);
    assert_eq!(member.overlay, Some((Arc::from(alias.as_str()), 61)));
    assert_eq!(capture.closed.iter().map(Vec::len).sum::<usize>(), 1);
    assert!(workspace.fresh_all(&capture, &documents, &incarnations));
    documents.insert(
        alias.clone(),
        ktlsp_text::Document::new(3, "fun unsaved() = 9\n".into()).unwrap(),
    );
    assert!(!workspace.fresh_all(&capture, &documents, &incarnations));
    documents.insert(
        alias.clone(),
        ktlsp_text::Document::new(2, "fun different() = 9\n".into()).unwrap(),
    );
    assert!(!workspace.fresh_all(&capture, &documents, &incarnations));
    documents.insert(
        alias.clone(),
        ktlsp_text::Document::new(2, "fun unsaved() = 9\n".into()).unwrap(),
    );
    workspace.closed(&alias, 61);
    incarnations.insert(alias.clone(), 62);
    workspace.opened(&alias, 62);
    assert!(!workspace.fresh_all(&capture, &documents, &incarnations));
    let reopened = workspace
        .capture_all(
            &documents,
            &incarnations,
            &mut crate::workspace_symbols::Budget::default(),
            &cancelled,
        )
        .unwrap();
    assert!(workspace.fresh_all(&reopened, &documents, &incarnations));
}

#[test]
fn aggregate_models_are_checked_with_zero_sources_and_all_member_overlays() {
    use ktlsp_import::source_inventory::SnapshotBudget;
    use std::collections::HashMap;
    let fixture = Fixture::new();
    let cancelled = AtomicBool::new(false);
    let mut workspace = Workspace::default();
    fixture.reload(&mut workspace, &[]).unwrap();
    let mut documents = HashMap::new();
    let mut incarnations = HashMap::new();
    for (name, incarnation) in [("A.kt", 71), ("B.kt", 72)] {
        let uri = fixture.uri(&format!("src/{name}"));
        workspace.opened(&uri, incarnation);
        documents.insert(
            uri.clone(),
            ktlsp_text::Document::new(1, "class Overlay\n".into()).unwrap(),
        );
        incarnations.insert(uri, incarnation);
    }
    let capture = workspace
        .capture_all(
            &documents,
            &incarnations,
            &mut crate::workspace_symbols::Budget::default(),
            &cancelled,
        )
        .unwrap();
    assert!(capture.closed.iter().all(Vec::is_empty));
    let snapshot = capture.snapshot.as_ref().unwrap();
    let mut verification = SnapshotBudget::default();
    snapshot
        .verify_all(&capture.closed, &mut verification, &cancelled)
        .unwrap();
    fixture.put("workspace.json", b"{ changed original model");
    assert!(
        snapshot
            .verify_all(&capture.closed, &mut verification, &cancelled)
            .is_err()
    );
    assert!(
        snapshot
            .verify_all(&[], &mut SnapshotBudget::default(), &cancelled)
            .is_err()
    );
    drop(capture);
    fixture.model();
    fs::remove_file(fixture.0.join("src/A.kt")).unwrap();
    fs::remove_file(fixture.0.join("src/B.kt")).unwrap();
    fixture.reload(&mut workspace, &[]).unwrap();
    let empty = workspace
        .capture_all(
            &HashMap::new(),
            &HashMap::new(),
            &mut crate::workspace_symbols::Budget::default(),
            &cancelled,
        )
        .unwrap();
    assert!(empty.members.is_empty());
    let snapshot = empty.snapshot.as_ref().unwrap();
    let mut verification = SnapshotBudget::default();
    snapshot
        .verify_all(&empty.closed, &mut verification, &cancelled)
        .unwrap();
    fixture.put("workspace.json", b"{ changed empty model");
    assert!(
        snapshot
            .verify_all(&empty.closed, &mut verification, &cancelled)
            .is_err()
    );
}
