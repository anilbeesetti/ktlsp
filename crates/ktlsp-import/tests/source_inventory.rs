//! Native data-only authored fixtures; no parser, server, worker or build-tool runtime.
use ktlsp_import::{source_inventory::{self, Budget, EntryKind, Error, Inventory, Limits}, v2};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        // Own only a newly created directory. Canonicalize the temp parent before
        // admission (macOS /var may itself be a symlink); never clean borrowed roots.
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = parent.join(format!("ktlsp-421-inventory-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, bytes: &[u8]) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn model(&self, roots: &[(&str, &str)]) -> Vec<u8> {
        let value = model(roots);
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        self.put("workspace.json", &bytes);
        bytes
    }
    fn load(&self) -> Result<Inventory, Error> {
        source_inventory::load(&self.0, &mut Budget::default(), &AtomicBool::new(false))
    }
}
impl Drop for Project {
    fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
}
fn model(roots: &[(&str, &str)]) -> Value {
    json!({"modules":[{"name":"app", "dependencies":[{"type":"moduleSource"}],
        "contentRoots":[{"path":"<WORKSPACE>/", "sourceRoots": roots.iter().map(|(path, kind)|
            json!({"path":format!("<WORKSPACE>/{path}"),"type":kind})).collect::<Vec<_>>()}]}],
        "libraries":[]})
}
fn source_index(inventory: &Inventory) -> usize {
    inventory.entries().iter().position(|entry| entry.kind() == EntryKind::KotlinSource).unwrap()
}
fn with_limits(project: &Project, limits: Limits) -> Result<Inventory, Error> {
    source_inventory::load(&project.0, &mut Budget::new(limits)?, &AtomicBool::new(false))
}

#[test]
fn complete_original_model_memberships_classifications_and_immutable_source() {
    let project = Project::new();
    let original = project.model(&[("src", "kotlin-source"), ("src/nested", "java-source")]);
    for (name, bytes) in [("src/nested/空 😀.kt", "fun original() = 1\r\n"),
        ("src/script.kts", "println(1)"), ("src/J.java", "class J {}"),
        ("src/.git/hidden.txt", "not ignored"), ("src/build/resource", "not ignored")] {
        project.put(name, bytes.as_bytes());
    }
    let inventory = project.load().unwrap();
    assert_eq!(inventory.project_directory(), project.0);
    assert_eq!(inventory.workspace_json(), original);
    assert_eq!(inventory.imported().modules()[0].dependencies.len(), 1);
    let index = source_index(&inventory);
    let entry = &inventory.entries()[index];
    assert_eq!(entry.source().unwrap().as_ref(), "fun original() = 1\r\n");
    let borrowed = Arc::clone(entry.source().unwrap());
    assert!(Arc::ptr_eq(&borrowed, entry.source().unwrap()));
    assert_eq!(entry.memberships().len(), 2);
    for (expected, membership) in entry.memberships().iter().enumerate() {
        assert_eq!(membership.module_index(), 0);
        assert_eq!(membership.content_root_index(), 0);
        assert_eq!(membership.source_root_index(), expected);
        let root = &inventory.imported().modules()[membership.module_index()]
            .content_roots[membership.content_root_index()].source_roots[membership.source_root_index()];
        assert_eq!(root.root_type, ["kotlin-source", "java-source"][expected]);
    }
    for kind in [EntryKind::Directory, EntryKind::KotlinScript, EntryKind::Java, EntryKind::Other] {
        assert!(inventory.entries().iter().any(|entry| entry.kind() == kind));
    }
    assert_eq!(inventory.entries().iter().filter(|e| e.kind() == EntryKind::KotlinSource).count(), 1);
    assert!(inventory.entries().iter().filter(|e| e.kind() != EntryKind::KotlinSource).all(|e| e.source().is_none()));
    let mut detached = inventory.imported().project_projection().clone();
    detached.modules.clear();
    assert_eq!(inventory.imported().modules().len(), 1);
    project.put("src/nested/空 😀.kt", b"fun changed() = 2\r\n");
    assert_eq!(entry.source().unwrap().as_ref(), "fun original() = 1\r\n");
    assert!(matches!(inventory.verify_source(index, &mut Budget::default(), &AtomicBool::new(false)), Err(Error::Changed)));
    assert_eq!(fs::read(project.0.join("workspace.json")).unwrap(), original);
}

#[test]
fn importer_stays_data_only_and_meaningful_unsupported_inputs_refuse() {
    let project = Project::new();
    let bytes = project.model(&[("missing", "kotlin-source")]);
    assert!(v2::from_json(&project.0, &bytes).is_ok()); // No implicit I/O.
    assert!(matches!(project.load(), Err(Error::Io(_))));
    let mut unsupported = model(&[]);
    unsupported["modules"][0]["externalProjectPath"] = json!("/meaningful/original");
    unsupported["modules"][0]["javaSettings"] = json!({"languageLevel":"JDK_17"});
    let original = serde_json::to_vec(&unsupported).unwrap();
    project.put("workspace.json", &original);
    assert!(matches!(project.load(), Err(Error::Import(_))));
    assert_eq!(fs::read(project.0.join("workspace.json")).unwrap(), original);
}

#[test]
fn empty_roots_complete_and_late_root_failure_never_returns_prefix() {
    let project = Project::new();
    fs::create_dir(project.0.join("empty")).unwrap();
    project.model(&[("empty", "kotlin-source")]);
    assert_eq!(project.load().unwrap().entries().len(), 1);
    project.put("first/A.kt", b"fun a() = 1");
    project.model(&[("first", "kotlin-source"), ("missing-last", "kotlin-source")]);
    assert!(matches!(project.load(), Err(Error::Io(_))));
    project.put("missing-last/Bad.kt", &[0xff]);
    assert!(matches!(project.load(), Err(Error::InvalidSource(_))));
}

#[test]
fn conflicts_external_paths_and_full_physical_aliases_refuse() {
    let project = Project::new();
    project.put("src/nested/A.kt", b"fun a() = 1");
    project.model(&[("src", "kotlin-source"), ("src/nested", "kotlin-test")]);
    assert!(project.load().is_err());
    let mut conflicting = model(&[("src", "kotlin-source")]);
    let mut second = conflicting["modules"][0].clone();
    second["name"] = json!("other");
    second["contentRoots"][0]["sourceRoots"][0]["path"] = json!("<WORKSPACE>/src/nested");
    conflicting["modules"].as_array_mut().unwrap().push(second);
    project.put("workspace.json", &serde_json::to_vec(&conflicting).unwrap());
    assert!(project.load().is_err());
    let external = Project::new();
    fs::create_dir(external.0.join("src")).unwrap();
    let mut value = model(&[("src", "kotlin-source")]);
    value["modules"][0]["contentRoots"][0]["sourceRoots"][0]["path"] = json!(external.0.join("src"));
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    project.model(&[("src", "kotlin-source")]);
    fs::hard_link(project.0.join("src/nested/A.kt"), project.0.join("src/Alias.kt")).unwrap();
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
}

#[test]
fn shared_budget_refuses_cross_project_identity_and_does_not_refund_failed_loads() {
    let first = Project::new();
    let second = Project::new();
    first.model(&[("src", "kotlin-source")]);
    second.model(&[("src", "kotlin-source")]);
    first.put("src/A.kt", b"fun a() = 1");
    fs::create_dir(second.0.join("src")).unwrap();
    fs::hard_link(first.0.join("src/A.kt"), second.0.join("src/A.kt")).unwrap();
    let cancelled = AtomicBool::new(false);
    let mut budget = Budget::default();
    source_inventory::load(&first.0, &mut budget, &cancelled).unwrap();
    assert!(matches!(source_inventory::load(&second.0, &mut budget, &cancelled), Err(Error::UnsafePath(_))));
    let mut budget = Budget::new(Limits { projects: 1, ..Limits::default() }).unwrap();
    assert!(source_inventory::load(first.0.join("missing"), &mut budget, &cancelled).is_err());
    assert!(matches!(source_inventory::load(&first.0, &mut budget, &cancelled), Err(Error::Budget("projects"))));
    let mut budget = Budget::default();
    source_inventory::load(&first.0, &mut budget, &cancelled).unwrap();
    assert!(matches!(source_inventory::load(&first.0, &mut budget, &cancelled), Err(Error::UnsafePath(_))));
}

#[test]
fn verification_uses_original_identity_exact_bytes_and_one_caller_budget() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    let inventory = project.load().unwrap();
    let index = source_index(&inventory);
    let cancel = AtomicBool::new(false);
    let mut budget = Budget::new(Limits { sources: 1, ..Limits::default() }).unwrap();
    inventory.verify_source(index, &mut budget, &cancel).unwrap();
    assert!(matches!(inventory.verify_source(index, &mut budget, &cancel), Err(Error::Budget("sources"))));
    fs::rename(project.0.join("src/A.kt"), project.0.join("src/original")).unwrap();
    project.put("src/A.kt", b"fun a() = 1"); // Same bytes, different native identity.
    assert!(matches!(inventory.verify_source(index, &mut Budget::default(), &cancel), Err(Error::Changed)));
    fs::remove_file(project.0.join("src/A.kt")).unwrap();
    fs::rename(project.0.join("src"), project.0.join("old-src")).unwrap();
    fs::create_dir(project.0.join("src")).unwrap();
    fs::rename(project.0.join("old-src/original"), project.0.join("src/A.kt")).unwrap();
    assert!(matches!(inventory.verify_source(index, &mut Budget::default(), &cancel), Err(Error::Changed)));
    fs::remove_dir_all(project.0.join("src")).unwrap();
    assert!(matches!(inventory.verify_source(usize::MAX, &mut Budget::default(), &cancel), Err(Error::InvalidSource(_))));
    let directory = inventory.entries().iter().position(|e| e.kind() == EntryKind::Directory).unwrap();
    assert!(matches!(inventory.verify_source(directory, &mut Budget::default(), &cancel), Err(Error::InvalidSource(_))));
    cancel.store(true, Ordering::Relaxed);
    assert!(matches!(inventory.verify_source(index, &mut Budget::default(), &cancel), Err(Error::Cancelled)));
    assert!(matches!(source_inventory::load(&project.0, &mut Budget::default(), &cancel), Err(Error::Cancelled)));
}

#[test]
fn exact_frozen_defaults_and_every_reducible_boundary() {
    let limits = Limits::default();
    assert_eq!([limits.projects, limits.model_bytes, limits.entries, limits.directories, limits.depth,
        limits.sources, limits.source_bytes, limits.source_file_bytes, limits.path_bytes,
        limits.path_file_bytes, limits.memberships, limits.work],
        [16, 2*1024*1024, 16384, 4096, 32, 1024, 16*1024*1024, 64*1024,
            2*1024*1024, 4096, 16384, 256*1024*1024]);
    assert!(Budget::new(Limits { work: usize::MAX, ..limits }).is_err());
    let project = Project::new();
    let model = project.model(&[("src", "kotlin-source")]);
    let source = b"fun a() = 1";
    project.put("src/A.kt", source);
    // Each check admits the exact boundary and refuses one below, not a prefix.
    let setters: [fn(&mut Limits, usize); 12] = [
        |l,v| l.projects=v, |l,v| l.model_bytes=v, |l,v| l.entries=v, |l,v| l.directories=v,
        |l,v| l.depth=v, |l,v| l.sources=v, |l,v| l.source_bytes=v, |l,v| l.source_file_bytes=v,
        |l,v| l.path_bytes=v, |l,v| l.path_file_bytes=v, |l,v| l.memberships=v, |l,v| l.work=v,
    ];
    let maxima = [limits.projects, limits.model_bytes, limits.entries, limits.directories, limits.depth,
        limits.sources, limits.source_bytes, limits.source_file_bytes, limits.path_bytes,
        limits.path_file_bytes, limits.memberships, limits.work];
    let expected = [Some(1), Some(model.len()), Some(2), Some(1), Some(2), Some(1),
        Some(source.len()), Some(source.len()), None, None, Some(2), None];
    for (index, set) in setters.into_iter().enumerate() {
        let mut low = 0;
        let mut high = maxima[index];
        while low < high {
            let mid = low + (high-low)/2;
            let mut candidate = limits;
            set(&mut candidate, mid);
            if with_limits(&project, candidate).is_ok() { high = mid; } else { low = mid+1; }
        }
        if let Some(expected) = expected[index] { assert_eq!(low, expected, "boundary {index}"); }
        assert!(low > 0);
        let mut exact = limits;
        set(&mut exact, low);
        assert!(with_limits(&project, exact).is_ok(), "exact boundary {index}");
        set(&mut exact, low-1);
        assert!(matches!(with_limits(&project, exact), Err(Error::Budget(_))), "over boundary {index}");
    }
    project.put("src/A.kt", &vec![b'a'; 64*1024]);
    assert!(project.load().is_ok());
    project.put("src/A.kt", &vec![b'a'; 64*1024+1]);
    assert!(matches!(project.load(), Err(Error::Budget("per-source bytes"))));
}

#[test]
fn aggregate_model_and_source_bytes_span_distinct_projects() {
    let first = Project::new();
    let second = Project::new();
    let first_bytes = first.model(&[("src", "kotlin-source")]);
    let second_bytes = second.model(&[("src", "kotlin-source")]);
    first.put("src/A.kt", b"abcd");
    second.put("src/A.kt", b"efgh");
    let cancelled = AtomicBool::new(false);
    for (model_bytes, source_bytes, succeeds) in [
        (first_bytes.len()+second_bytes.len(), 8, true),
        (first_bytes.len()+second_bytes.len()-1, 8, false),
        (first_bytes.len()+second_bytes.len(), 7, false),
    ] {
        let mut budget = Budget::new(Limits { model_bytes, source_bytes, ..Limits::default() }).unwrap();
        source_inventory::load(&first.0, &mut budget, &cancelled).unwrap();
        assert_eq!(source_inventory::load(&second.0, &mut budget, &cancelled).is_ok(), succeeds);
    }
}

#[cfg(unix)]
#[test]
fn symlink_ancestor_leaf_cycle_special_and_nonunicode_paths_refuse() {
    use std::os::unix::{fs::{symlink, FileTypeExt}, net::UnixListener, ffi::OsStringExt};
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    symlink(project.0.join("src/A.kt"), project.0.join("src/link.kt")).unwrap();
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    fs::remove_file(project.0.join("src/link.kt")).unwrap();
    symlink(&project.0, project.0.join("src/cycle")).unwrap();
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    fs::remove_file(project.0.join("src/cycle")).unwrap();
    let listener = UnixListener::bind(project.0.join("src/socket")).unwrap();
    assert!(fs::symlink_metadata(project.0.join("src/socket")).unwrap().file_type().is_socket());
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    drop(listener);
    fs::remove_file(project.0.join("src/socket")).unwrap();
    let bad = project.0.join("src").join(std::ffi::OsString::from_vec(vec![0xff]));
    fs::write(&bad, b"other").unwrap();
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    fs::remove_file(bad).unwrap();
    fs::rename(project.0.join("src"), project.0.join("actual")).unwrap();
    symlink(project.0.join("actual"), project.0.join("src")).unwrap();
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    // A project path with a symlink ancestor refuses before workspace.json I/O.
    assert!(matches!(source_inventory::load(project.0.join("src/sub"), &mut Budget::default(), &AtomicBool::new(false)), Err(Error::UnsafePath(_))));
}

#[test]
fn oversized_or_malformed_model_and_missing_original_source_refuse_read_only() {
    let project = Project::new();
    project.put("workspace.json", &vec![b' '; 2*1024*1024+1]);
    assert!(matches!(project.load(), Err(Error::Budget("model bytes"))));
    project.put("workspace.json", b"{\"modules\":[] trailing");
    assert!(matches!(project.load(), Err(Error::Import(_))));
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    let inventory = project.load().unwrap();
    fs::remove_file(project.0.join("src/A.kt")).unwrap();
    assert!(matches!(inventory.verify_source(source_index(&inventory), &mut Budget::default(), &AtomicBool::new(false)), Err(Error::Io(_))));
    assert!(!project.0.join("src/A.kt").exists());
}
