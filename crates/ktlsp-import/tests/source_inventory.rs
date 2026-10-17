//! Native data-only authored fixtures; no parser, server, worker or build-tool runtime.
use ktlsp_import::{
    source_inventory::{
        self, Budget, EntryKind, Error, Inventory, Limits, SnapshotBudget, SnapshotLimits,
    },
    v2,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        // Own only a newly created child; never clean borrowed roots. Unix
        // sockets need a short parent (Darwin sun_path is only 104 bytes).
        #[cfg(unix)]
        let parent = fs::canonicalize("/tmp").unwrap();
        #[cfg(not(unix))]
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = parent.join(format!(
            "ktlsp-421-inventory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
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
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn model(roots: &[(&str, &str)]) -> Value {
    json!({"modules":[{"name":"app", "dependencies":[{"type":"moduleSource"}],
        "contentRoots":[{"path":"<WORKSPACE>/", "sourceRoots": roots.iter().map(|(path, kind)|
            json!({"path":format!("<WORKSPACE>/{path}"),"type":kind})).collect::<Vec<_>>()}]}],
        "libraries":[]})
}
fn source_index(inventory: &Inventory) -> usize {
    inventory
        .entries()
        .iter()
        .position(|entry| entry.kind() == EntryKind::KotlinSource)
        .unwrap()
}
fn with_limits(project: &Project, limits: Limits) -> Result<Inventory, Error> {
    source_inventory::load(
        &project.0,
        &mut Budget::new(limits)?,
        &AtomicBool::new(false),
    )
}

#[test]
fn complete_original_model_memberships_classifications_and_immutable_source() {
    let project = Project::new();
    let original = project.model(&[("src", "kotlin-source"), ("src/nested", "java-source")]);
    for (name, bytes) in [
        ("src/nested/空 😀.kt", "fun original() = 1\r\n"),
        ("src/script.kts", "println(1)"),
        ("src/J.java", "class J {}"),
        ("src/.git/hidden.txt", "not ignored"),
        ("src/build/resource", "not ignored"),
    ] {
        project.put(name, bytes.as_bytes());
    }
    let inventory = project.load().unwrap();
    let index = source_index(&inventory);
    let entry = &inventory.entries()[index];
    assert_eq!(entry.source().unwrap().as_ref(), "fun original() = 1\r\n");
    assert_eq!(entry.memberships().len(), 2);
    for (expected, membership) in entry.memberships().iter().enumerate() {
        assert_eq!(membership.module_index(), 0);
        assert_eq!(membership.content_root_index(), 0);
        assert_eq!(membership.source_root_index(), expected);
        let root = &inventory.imported().modules()[membership.module_index()].content_roots
            [membership.content_root_index()]
        .source_roots[membership.source_root_index()];
        assert_eq!(root.root_type, ["kotlin-source", "java-source"][expected]);
    }
    for kind in [
        EntryKind::Directory,
        EntryKind::KotlinScript,
        EntryKind::Java,
        EntryKind::Other,
    ] {
        assert!(inventory.entries().iter().any(|entry| entry.kind() == kind));
    }
    assert_eq!(
        inventory
            .entries()
            .iter()
            .filter(|e| e.kind() == EntryKind::KotlinSource)
            .count(),
        1
    );
    assert!(
        inventory
            .entries()
            .iter()
            .filter(|e| e.kind() != EntryKind::KotlinSource)
            .all(|e| e.source().is_none())
    );
    let mut detached = inventory.imported().project_projection().clone();
    detached.modules.clear();
    assert_eq!(inventory.imported().modules().len(), 1);
    project.put("src/nested/空 😀.kt", b"fun changed() = 2\r\n");
    assert_eq!(entry.source().unwrap().as_ref(), "fun original() = 1\r\n");
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &AtomicBool::new(false)),
        Err(Error::Changed)
    ));
    assert_eq!(
        fs::read(project.0.join("workspace.json")).unwrap(),
        original
    );
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
    assert_eq!(
        fs::read(project.0.join("workspace.json")).unwrap(),
        original
    );
}

#[test]
fn empty_roots_complete_and_late_root_failure_never_returns_prefix() {
    let project = Project::new();
    fs::create_dir(project.0.join("empty")).unwrap();
    project.model(&[("empty", "kotlin-source")]);
    assert_eq!(project.load().unwrap().entries().len(), 1);
    project.put("first/A.kt", b"fun a() = 1");
    project.model(&[
        ("first", "kotlin-source"),
        ("missing-last", "kotlin-source"),
    ]);
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
    let external_path = external.0.join("src");
    let external_text = external_path.to_str().unwrap();
    value["modules"][0]["contentRoots"][0]["sourceRoots"][0]["path"] = json!(
        external_text
            .strip_prefix("\\\\?\\")
            .unwrap_or(external_text)
            .replace('\\', "/")
    );
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    project.model(&[("src", "kotlin-source")]);
    fs::hard_link(
        project.0.join("src/nested/A.kt"),
        project.0.join("src/Alias.kt"),
    )
    .unwrap();
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
    assert!(matches!(
        source_inventory::load(&second.0, &mut budget, &cancelled),
        Err(Error::UnsafePath(_))
    ));
    let mut budget = Budget::new(Limits {
        projects: 1,
        ..Limits::default()
    })
    .unwrap();
    assert!(source_inventory::load(first.0.join("missing"), &mut budget, &cancelled).is_err());
    assert!(matches!(
        source_inventory::load(&first.0, &mut budget, &cancelled),
        Err(Error::Budget("projects"))
    ));
    let mut budget = Budget::default();
    source_inventory::load(&first.0, &mut budget, &cancelled).unwrap();
    assert!(matches!(
        source_inventory::load(&first.0, &mut budget, &cancelled),
        Err(Error::UnsafePath(_))
    ));
}

#[test]
fn verification_uses_original_identity_exact_bytes_and_one_caller_budget() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    let inventory = project.load().unwrap();
    let index = source_index(&inventory);
    let cancel = AtomicBool::new(false);
    let mut budget = Budget::new(Limits {
        sources: 1,
        ..Limits::default()
    })
    .unwrap();
    inventory
        .verify_source(index, &mut budget, &cancel)
        .unwrap();
    assert!(matches!(
        inventory.verify_source(index, &mut budget, &cancel),
        Err(Error::Budget("sources"))
    ));
    fs::rename(project.0.join("src/A.kt"), project.0.join("src/original")).unwrap();
    project.put("src/A.kt", b"fun a() = 1"); // Same bytes, different native identity.
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &cancel),
        Err(Error::Changed)
    ));
    fs::remove_file(project.0.join("src/A.kt")).unwrap();
    fs::rename(project.0.join("src"), project.0.join("old-src")).unwrap();
    fs::create_dir(project.0.join("src")).unwrap();
    fs::rename(
        project.0.join("old-src/original"),
        project.0.join("src/A.kt"),
    )
    .unwrap();
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &cancel),
        Err(Error::Changed)
    ));
    fs::remove_dir_all(project.0.join("src")).unwrap();
    assert!(matches!(
        inventory.verify_source(usize::MAX, &mut Budget::default(), &cancel),
        Err(Error::InvalidSource(_))
    ));
    let directory = inventory
        .entries()
        .iter()
        .position(|e| e.kind() == EntryKind::Directory)
        .unwrap();
    assert!(matches!(
        inventory.verify_source(directory, &mut Budget::default(), &cancel),
        Err(Error::InvalidSource(_))
    ));
    cancel.store(true, Ordering::Relaxed);
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &cancel),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        source_inventory::load(&project.0, &mut Budget::default(), &cancel),
        Err(Error::Cancelled)
    ));
}

#[test]
fn exact_frozen_defaults_and_every_reducible_boundary() {
    let limits = Limits::default();
    assert_eq!(
        [
            limits.projects,
            limits.model_bytes,
            limits.entries,
            limits.directories,
            limits.depth,
            limits.sources,
            limits.source_bytes,
            limits.source_file_bytes,
            limits.path_bytes,
            limits.path_file_bytes,
            limits.memberships,
            limits.work
        ],
        [
            16,
            2 * 1024 * 1024,
            16384,
            4096,
            32,
            1024,
            16 * 1024 * 1024,
            64 * 1024,
            2 * 1024 * 1024,
            4096,
            16384,
            256 * 1024 * 1024
        ]
    );
    assert!(
        Budget::new(Limits {
            work: usize::MAX,
            ..limits
        })
        .is_err()
    );
    let project = Project::new();
    let model = project.model(&[("src", "kotlin-source")]);
    let source = b"fun a() = 1";
    project.put("src/A.kt", source);
    // Each check admits the exact boundary and refuses one below, not a prefix.
    let setters: [fn(&mut Limits, usize); 12] = [
        |l, v| l.projects = v,
        |l, v| l.model_bytes = v,
        |l, v| l.entries = v,
        |l, v| l.directories = v,
        |l, v| l.depth = v,
        |l, v| l.sources = v,
        |l, v| l.source_bytes = v,
        |l, v| l.source_file_bytes = v,
        |l, v| l.path_bytes = v,
        |l, v| l.path_file_bytes = v,
        |l, v| l.memberships = v,
        |l, v| l.work = v,
    ];
    let maxima = [
        limits.projects,
        limits.model_bytes,
        limits.entries,
        limits.directories,
        limits.depth,
        limits.sources,
        limits.source_bytes,
        limits.source_file_bytes,
        limits.path_bytes,
        limits.path_file_bytes,
        limits.memberships,
        limits.work,
    ];
    let expected = [
        Some(1),
        Some(model.len()),
        Some(2),
        Some(1),
        Some(2),
        Some(1),
        Some(source.len()),
        Some(source.len()),
        None,
        None,
        Some(2),
        None,
    ];
    for (index, set) in setters.into_iter().enumerate() {
        let mut low = 0;
        let mut high = maxima[index];
        while low < high {
            let mid = low + (high - low) / 2;
            let mut candidate = limits;
            set(&mut candidate, mid);
            if with_limits(&project, candidate).is_ok() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        if let Some(expected) = expected[index] {
            assert_eq!(low, expected, "boundary {index}");
        }
        assert!(low > 0);
        let mut exact = limits;
        set(&mut exact, low);
        assert!(
            with_limits(&project, exact).is_ok(),
            "exact boundary {index}"
        );
        set(&mut exact, low - 1);
        assert!(
            matches!(with_limits(&project, exact), Err(Error::Budget(_))),
            "over boundary {index}"
        );
    }
    project.put("src/A.kt", &vec![b'a'; 64 * 1024]);
    assert!(project.load().is_ok());
    project.put("src/A.kt", &vec![b'a'; 64 * 1024 + 1]);
    assert!(matches!(
        project.load(),
        Err(Error::Budget("per-source bytes"))
    ));
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
        (first_bytes.len() + second_bytes.len(), 8, true),
        (first_bytes.len() + second_bytes.len() - 1, 8, false),
        (first_bytes.len() + second_bytes.len(), 7, false),
    ] {
        let mut budget = Budget::new(Limits {
            model_bytes,
            source_bytes,
            ..Limits::default()
        })
        .unwrap();
        source_inventory::load(&first.0, &mut budget, &cancelled).unwrap();
        assert_eq!(
            source_inventory::load(&second.0, &mut budget, &cancelled).is_ok(),
            succeeds
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_ancestor_leaf_cycle_special_and_nonunicode_paths_refuse() {
    use std::os::unix::{
        ffi::OsStringExt,
        fs::{FileTypeExt, symlink},
        net::UnixListener,
    };
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
    assert!(
        fs::symlink_metadata(project.0.join("src/socket"))
            .unwrap()
            .file_type()
            .is_socket()
    );
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    drop(listener);
    fs::remove_file(project.0.join("src/socket")).unwrap();
    let bad = project
        .0
        .join("src")
        .join(std::ffi::OsString::from_vec(vec![0xff]));
    assert!(matches!(
        source_inventory::load(&bad, &mut Budget::default(), &AtomicBool::new(false)),
        Err(Error::UnsafePath(_))
    ));
    // APFS refuses invalid UTF-8 filenames before an entry exists.
    #[cfg(not(target_os = "macos"))]
    {
        fs::write(&bad, b"other").unwrap();
        assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
        fs::remove_file(bad).unwrap();
    }
    fs::rename(project.0.join("src"), project.0.join("actual")).unwrap();
    symlink(project.0.join("actual"), project.0.join("src")).unwrap();
    assert!(matches!(project.load(), Err(Error::UnsafePath(_))));
    // A project path with a symlink ancestor refuses before workspace.json I/O.
    assert!(matches!(
        source_inventory::load(
            project.0.join("src/sub"),
            &mut Budget::default(),
            &AtomicBool::new(false)
        ),
        Err(Error::UnsafePath(_))
    ));
}

#[test]
fn oversized_or_malformed_model_and_missing_original_source_refuse_read_only() {
    let project = Project::new();
    project.put("workspace.json", &vec![b' '; 2 * 1024 * 1024 + 1]);
    assert!(matches!(project.load(), Err(Error::Budget("model bytes"))));
    project.put("workspace.json", b"{\"modules\":[] trailing");
    assert!(matches!(project.load(), Err(Error::Import(_))));
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    let inventory = project.load().unwrap();
    fs::remove_file(project.0.join("src/A.kt")).unwrap();
    assert!(matches!(
        inventory.verify_source(
            source_index(&inventory),
            &mut Budget::default(),
            &AtomicBool::new(false)
        ),
        Err(Error::Io(_))
    ));
    assert!(!project.0.join("src/A.kt").exists());
}

fn minimum_path_bytes(project: &Project) -> usize {
    let mut low = 0;
    let mut high = Limits::default().path_bytes;
    while low < high {
        let mid = low + (high - low) / 2;
        if with_limits(
            project,
            Limits {
                path_bytes: mid,
                ..Limits::default()
            },
        )
        .is_ok()
        {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
}

#[test]
fn source_free_content_library_and_projection_paths_are_all_charged() {
    let project = Project::new();
    project.model(&[]);
    let baseline = minimum_path_bytes(&project);
    let mut value = model(&[]);
    let extra = project.0.join("unopened-content");
    value["modules"][0]["contentRoots"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path":"<WORKSPACE>/unopened-content","sourceRoots":[]}));
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    assert_eq!(
        minimum_path_bytes(&project),
        baseline + extra.to_str().unwrap().len()
    );
    let with_content = minimum_path_bytes(&project);
    let archive = project.0.join("not-opened.jar");
    value["libraries"] =
        json!([{"name":"data","type":null,"roots":[{"path":"<WORKSPACE>/not-opened.jar"}]}]);
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    assert_eq!(
        minimum_path_bytes(&project),
        with_content + 2 * archive.to_str().unwrap().len()
    );
    let second = Project::new();
    second.model(&[]);
    let second_cost = minimum_path_bytes(&second);
    let first_cost = minimum_path_bytes(&project);
    for (path_bytes, success) in [
        (first_cost + second_cost, true),
        (first_cost + second_cost - 1, false),
    ] {
        let mut budget = Budget::new(Limits {
            path_bytes,
            ..Limits::default()
        })
        .unwrap();
        let cancelled = AtomicBool::new(false);
        source_inventory::load(&project.0, &mut budget, &cancelled).unwrap();
        assert_eq!(
            source_inventory::load(&second.0, &mut budget, &cancelled).is_ok(),
            success
        );
    }
    let long = format!("<WORKSPACE>/{}", "x".repeat(4097));
    value["modules"][0]["contentRoots"][1]["path"] = json!(long);
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    assert!(matches!(
        project.load(),
        Err(Error::Budget("per-path bytes"))
    ));
    value["modules"][0]["contentRoots"]
        .as_array_mut()
        .unwrap()
        .pop();
    value["libraries"][0]["roots"][0]["path"] = json!(long);
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    assert!(matches!(
        project.load(),
        Err(Error::Budget("per-path bytes"))
    ));
}

#[test]
fn model_source_and_model_model_hardlinks_refuse_in_both_load_orders() {
    let first = Project::new();
    first.model(&[("src", "kotlin-source")]);
    first.put("src/Model.kt", b"{}");
    let second = Project::new();
    fs::hard_link(
        first.0.join("src/Model.kt"),
        second.0.join("workspace.json"),
    )
    .unwrap();
    for reverse in [false, true] {
        let mut budget = Budget::default();
        let cancelled = AtomicBool::new(false);
        let (a, b) = if reverse {
            (&second, &first)
        } else {
            (&first, &second)
        };
        source_inventory::load(&a.0, &mut budget, &cancelled).unwrap();
        assert!(matches!(
            source_inventory::load(&b.0, &mut budget, &cancelled),
            Err(Error::UnsafePath(_))
        ));
    }
    let third = Project::new();
    fs::hard_link(
        second.0.join("workspace.json"),
        third.0.join("workspace.json"),
    )
    .unwrap();
    for reverse in [false, true] {
        let mut budget = Budget::default();
        let cancelled = AtomicBool::new(false);
        let (a, b) = if reverse {
            (&third, &second)
        } else {
            (&second, &third)
        };
        source_inventory::load(&a.0, &mut budget, &cancelled).unwrap();
        assert!(matches!(
            source_inventory::load(&b.0, &mut budget, &cancelled),
            Err(Error::UnsafePath(_))
        ));
    }
    // The same actual model pathname is legitimate when the root is the project.
    first.model(&[("", "kotlin-source")]);
    assert!(first.load().is_ok());
    fs::hard_link(
        first.0.join("workspace.json"),
        first.0.join("src/ModelAlias.kt"),
    )
    .unwrap();
    assert!(matches!(first.load(), Err(Error::UnsafePath(_))));
}

#[test]
fn selected_root_depth_is_admitted_before_any_missing_root_io() {
    let project = Project::new();
    project.model(&[("a/b/missing", "kotlin-source")]);
    assert!(matches!(
        with_limits(
            &project,
            Limits {
                depth: 2,
                ..Limits::default()
            }
        ),
        Err(Error::Budget("depth"))
    ));
    project.model(&[("a/b", "kotlin-source")]);
    fs::create_dir_all(project.0.join("a/b")).unwrap();
    assert!(
        with_limits(
            &project,
            Limits {
                depth: 2,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    assert!(matches!(
        with_limits(
            &project,
            Limits {
                depth: 1,
                ..Limits::default()
            }
        ),
        Err(Error::Budget("depth"))
    ));
}

#[test]
fn delegated_work_accounts_for_long_base_macro_amplification() {
    let project = Project::new();
    let mut deep = project.0.clone();
    // Stay below Darwin's 1024-byte PATH_MAX while exceeding the work budget.
    for _ in 0..8 {
        deep.push("a".repeat(95));
    }
    fs::create_dir_all(&deep).unwrap();
    let roots = (0..512)
        .map(|_| json!({"path":"<WORKSPACE>/","sourceRoots":[]}))
        .collect::<Vec<_>>();
    let value = json!({"modules":[{"name":"app","contentRoots":roots}]});
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(deep.join("workspace.json"), bytes).unwrap();
    let mut budget = Budget::new(Limits {
        work: 1024 * 1024,
        ..Limits::default()
    })
    .unwrap();
    assert!(matches!(
        source_inventory::load(&deep, &mut budget, &AtomicBool::new(false)),
        Err(Error::Budget("work"))
    ));
}

#[cfg(windows)]
#[test]
fn ordinary_absolute_drive_roots_match_canonical_verbatim_projects() {
    let project = Project::new();
    project.put("src/A.kt", b"fun a() = 1");
    let canonical = project.0.join("src");
    let ordinary = canonical
        .to_str()
        .unwrap()
        .strip_prefix("\\\\?\\")
        .unwrap()
        .replace('\\', "/");
    let mut value = model(&[]);
    value["modules"][0]["contentRoots"][0]["sourceRoots"] =
        json!([{"path":ordinary,"type":"kotlin-source"}]);
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    let inventory = project.load().unwrap();
    assert_eq!(
        inventory.entries()[source_index(&inventory)].path(),
        canonical.join("A.kt")
    );
    let original = &inventory.imported().modules()[0].content_roots[0].source_roots[0].path;
    assert!(
        matches!(original.components().next(), Some(std::path::Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_)))
    );
}

#[test]
fn expanded_path_reservation_precedes_allocating_schema_import() {
    let project = Project::new();
    let mut value = model(&[]);
    value["unknownMeaningfulField"] = json!(true);
    value["modules"][0]["contentRoots"][0]["path"] =
        json!(format!("<WORKSPACE>/{}", "x".repeat(4097)));
    project.put("workspace.json", &serde_json::to_vec(&value).unwrap());
    // Unchanged v2 would refuse this meaningful field, but inventory must first
    // refuse the known over-ceiling expansion, before handing bytes to v2.
    assert!(
        v2::from_json(
            &project.0,
            &fs::read(project.0.join("workspace.json")).unwrap()
        )
        .is_err()
    );
    assert!(matches!(
        project.load(),
        Err(Error::Budget("per-path bytes"))
    ));
}

#[test]
fn original_model_identity_and_exact_bytes_are_verified_without_storage_recharge() {
    let project = Project::new();
    let original = project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    let inventory = project.load().unwrap();
    let index = source_index(&inventory);
    let cancelled = AtomicBool::new(false);
    let mut request = Budget::new(Limits {
        model_bytes: 0,
        ..Limits::default()
    })
    .unwrap();
    inventory
        .verify_source(index, &mut request, &cancelled)
        .unwrap();
    inventory
        .verify_source(index, &mut request, &cancelled)
        .unwrap();
    // A model edit with no client event cannot leave old membership current.
    let changed = String::from_utf8(original.clone())
        .unwrap()
        .replace("\"app\"", "\"new\"");
    assert_eq!(changed.len(), original.len());
    project.put("workspace.json", changed.as_bytes());
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &cancelled),
        Err(Error::Changed)
    ));
    project.put("workspace.json", &original);
    fs::rename(
        project.0.join("workspace.json"),
        project.0.join("original-model"),
    )
    .unwrap();
    project.put("workspace.json", &original);
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &cancelled),
        Err(Error::Changed)
    ));
    fs::remove_file(project.0.join("workspace.json")).unwrap();
    assert!(matches!(
        inventory.verify_source(index, &mut Budget::default(), &cancelled),
        Err(Error::Io(_))
    ));
}

#[test]
fn consumer_uri_reservations_share_the_candidate_path_and_work_counters() {
    let mut budget = Budget::new(Limits {
        path_bytes: 10,
        ..Limits::default()
    })
    .unwrap();
    budget.reserve_path_bytes(4).unwrap();
    budget.reserve_path_bytes(6).unwrap();
    assert!(matches!(
        budget.reserve_path_bytes(1),
        Err(Error::Budget("path bytes"))
    ));
    let mut budget = Budget::new(Limits {
        work: 10,
        ..Limits::default()
    })
    .unwrap();
    budget.reserve_path_bytes(10).unwrap();
    assert!(matches!(
        budget.reserve_path_bytes(1),
        Err(Error::Budget("work"))
    ));
    let project = Project::new();
    project.model(&[]);
    let exact = minimum_path_bytes(&project);
    let mut budget = Budget::new(Limits {
        path_bytes: exact + 7,
        ..Limits::default()
    })
    .unwrap();
    source_inventory::load(&project.0, &mut budget, &AtomicBool::new(false)).unwrap();
    budget.reserve_path_bytes(7).unwrap();
    assert!(matches!(
        budget.reserve_path_bytes(1),
        Err(Error::Budget("path bytes"))
    ));
}

fn kotlin_indices(inventory: &Inventory) -> Vec<usize> {
    inventory
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry.kind() == EntryKind::KotlinSource).then_some(index))
        .collect()
}

fn verify_two_phases(inventories: &[Inventory], limits: SnapshotLimits) -> Result<(), Error> {
    let cancelled = AtomicBool::new(false);
    let mut budget = SnapshotBudget::new(limits)?;
    for _ in 0..2 {
        budget.begin_phase()?;
        for inventory in inventories {
            inventory.verify_inventory(&kotlin_indices(inventory), &mut budget, &cancelled)?;
        }
    }
    Ok(())
}

#[test]
fn snapshot_models_remain_authoritative_without_any_closed_sources() {
    let cancelled = AtomicBool::new(false);
    for all_overlaid in [false, true] {
        let project = Project::new();
        let original = if all_overlaid {
            project.put("src/A.kt", b"fun disk() = 1");
            project.model(&[("src", "kotlin-source")])
        } else {
            project.model(&[])
        };
        let inventory = project.load().unwrap();
        let mut budget = SnapshotBudget::default();
        budget.begin_phase().unwrap();
        inventory
            .verify_inventory(&[], &mut budget, &cancelled)
            .unwrap();
        if all_overlaid {
            // An overlay selection does not depend on its retained disk bytes.
            fs::remove_file(project.0.join("src/A.kt")).unwrap();
        }
        budget.begin_phase().unwrap();
        inventory
            .verify_inventory(&[], &mut budget, &cancelled)
            .unwrap();

        let mut budget = SnapshotBudget::default();
        budget.begin_phase().unwrap();
        inventory
            .verify_inventory(&[], &mut budget, &cancelled)
            .unwrap();
        let changed = String::from_utf8(original.clone())
            .unwrap()
            .replace("\"app\"", "\"new\"");
        assert_eq!(changed.len(), original.len());
        project.put("workspace.json", changed.as_bytes());
        budget.begin_phase().unwrap();
        assert!(matches!(
            inventory.verify_inventory(&[], &mut budget, &cancelled),
            Err(Error::Changed)
        ));
        // Even the first phase must verify a zero-source/all-overlay model.
        let mut budget = SnapshotBudget::default();
        budget.begin_phase().unwrap();
        assert!(matches!(
            inventory.verify_inventory(&[], &mut budget, &cancelled),
            Err(Error::Changed)
        ));
    }
}

#[test]
fn snapshot_verifies_nonmatching_closed_sources_and_original_membership_only() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    project.put("src/Match.kt", b"fun wanted() = 1");
    project.put("src/Other.kt", b"fun absent() = 2");
    let inventory = project.load().unwrap();
    let indices = kotlin_indices(&inventory);
    let cancelled = AtomicBool::new(false);
    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    // Additions are not discovered until reload: no directory membership rescan.
    project.put("src/New.kt", b"fun added() = 3");
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();

    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    project.put("src/Other.kt", b"fun hidden() = 2");
    budget.begin_phase().unwrap();
    assert!(matches!(
        inventory.verify_inventory(&indices, &mut budget, &cancelled),
        Err(Error::Changed)
    ));
    let inventory = project.load().unwrap();
    let indices = kotlin_indices(&inventory);
    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    fs::remove_file(project.0.join("src/Other.kt")).unwrap();
    budget.begin_phase().unwrap();
    assert!(matches!(
        inventory.verify_inventory(&indices, &mut budget, &cancelled),
        Err(Error::Io(_))
    ));
}

#[test]
fn snapshot_selection_is_validated_before_io_and_cancellation_is_preserved() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    project.put("src/script.kts", b"println(1)");
    let inventory = project.load().unwrap();
    let source = source_index(&inventory);
    let nonsource = inventory
        .entries()
        .iter()
        .position(|entry| entry.kind() == EntryKind::KotlinScript)
        .unwrap();
    fs::remove_file(project.0.join("workspace.json")).unwrap();
    let cancelled = AtomicBool::new(false);
    for selection in [vec![usize::MAX], vec![nonsource], vec![source, source]] {
        let mut budget = SnapshotBudget::default();
        budget.begin_phase().unwrap();
        assert!(matches!(
            inventory.verify_inventory(&selection, &mut budget, &cancelled),
            Err(Error::InvalidSource(_))
        ));
    }
    assert!(matches!(
        inventory.verify_inventory(&[], &mut SnapshotBudget::default(), &cancelled),
        Err(Error::Budget("verification phase not begun"))
    ));
    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    cancelled.store(true, Ordering::Relaxed);
    assert!(matches!(
        inventory.verify_inventory(&[], &mut budget, &cancelled),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        budget.reserve_work(1, &cancelled),
        Err(Error::Cancelled)
    ));
}

#[test]
fn snapshot_exact_and_exceeded_aggregate_limits_are_reducible_not_raiseable() {
    let defaults = SnapshotLimits::default();
    assert_eq!(
        [
            defaults.phases,
            defaults.project_visits,
            defaults.source_visits,
            defaults.source_bytes,
            defaults.work
        ],
        [2, 32, 2048, 32 * 1024 * 1024, 256 * 1024 * 1024]
    );
    let setters: [fn(&mut SnapshotLimits, usize); 5] = [
        |limits, value| limits.phases = value,
        |limits, value| limits.project_visits = value,
        |limits, value| limits.source_visits = value,
        |limits, value| limits.source_bytes = value,
        |limits, value| limits.work = value,
    ];
    let maxima = [
        defaults.phases,
        defaults.project_visits,
        defaults.source_visits,
        defaults.source_bytes,
        defaults.work,
    ];
    let first = Project::new();
    let second = Project::new();
    let sources: [&[u8]; 2] = [b"fun first() = 1", b"fun second() = 2"];
    for project in [&first, &second] {
        project.model(&[("src", "kotlin-source")]);
        project.put("src/A.kt", sources[0]);
        project.put("src/B.kt", sources[1]);
    }
    let inventories = [first.load().unwrap(), second.load().unwrap()];
    let expected = [
        Some(2),
        Some(4),
        Some(8),
        Some(4 * (sources[0].len() + sources[1].len())),
        None,
    ];
    for (index, set) in setters.into_iter().enumerate() {
        let mut raised = defaults;
        set(&mut raised, maxima[index] + 1);
        assert!(matches!(
            SnapshotBudget::new(raised),
            Err(Error::Budget("invalid snapshot limits"))
        ));
        let mut low = 0;
        let mut high = maxima[index];
        while low < high {
            let mid = low + (high - low) / 2;
            let mut limits = defaults;
            set(&mut limits, mid);
            if verify_two_phases(&inventories, limits).is_ok() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        if let Some(expected) = expected[index] {
            assert_eq!(low, expected, "snapshot boundary {index}");
        }
        assert!(low > 0);
        let mut exact = defaults;
        set(&mut exact, low);
        verify_two_phases(&inventories, exact).unwrap();
        set(&mut exact, low - 1);
        assert!(matches!(
            verify_two_phases(&inventories, exact),
            Err(Error::Budget(_))
        ));
    }
}

#[test]
fn snapshot_both_full_admission_passes_fit_without_raising_old_limits() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    let full_source = vec![b'a'; Limits::default().source_file_bytes];
    for index in 0..Limits::default().sources {
        project.put(
            &format!("src/File{index}.kt"),
            if index < 256 { &full_source } else { b"" },
        );
    }
    let inventory = project.load().unwrap();
    let indices = kotlin_indices(&inventory);
    assert_eq!(indices.len(), 1024);
    assert_eq!(
        indices
            .iter()
            .map(|&index| inventory.entries()[index].source().unwrap().len())
            .sum::<usize>(),
        16 * 1024 * 1024
    );
    verify_two_phases(&[inventory], SnapshotLimits::default()).unwrap();
}

#[test]
fn snapshot_per_phase_project_and_source_visits_do_not_borrow_the_second_pass() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    project.put("src/A.kt", b"fun a() = 1");
    let inventory = project.load().unwrap();
    let cancelled = AtomicBool::new(false);
    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    for _ in 0..Limits::default().projects {
        inventory
            .verify_inventory(&[], &mut budget, &cancelled)
            .unwrap();
    }
    assert!(matches!(
        inventory.verify_inventory(&[], &mut budget, &cancelled),
        Err(Error::Budget("per-phase verification projects"))
    ));

    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    for index in 0..513 {
        project.put(&format!("src/F{index}.kt"), b"");
    }
    let inventory = project.load().unwrap();
    let indices = kotlin_indices(&inventory);
    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    assert!(matches!(
        inventory.verify_inventory(&indices, &mut budget, &cancelled),
        Err(Error::Budget("per-phase verification sources"))
    ));
}

#[test]
fn snapshot_consumer_work_and_verification_share_one_nonrefundable_allowance() {
    let project = Project::new();
    project.model(&[]);
    let inventory = project.load().unwrap();
    let cancelled = AtomicBool::new(false);
    let mut low = 0;
    let mut high = SnapshotLimits::default().work;
    while low < high {
        let mid = low + (high - low) / 2;
        let mut budget = SnapshotBudget::new(SnapshotLimits {
            work: mid,
            ..SnapshotLimits::default()
        })
        .unwrap();
        let result = budget
            .begin_phase()
            .and_then(|()| inventory.verify_inventory(&[], &mut budget, &cancelled));
        if result.is_ok() {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    let mut budget = SnapshotBudget::new(SnapshotLimits {
        work: low + 7,
        ..SnapshotLimits::default()
    })
    .unwrap();
    budget.reserve_work(7, &cancelled).unwrap();
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&[], &mut budget, &cancelled)
        .unwrap();
    assert!(matches!(
        budget.reserve_work(1, &cancelled),
        Err(Error::Budget("work"))
    ));
    // Beginning another phase cannot refund the exhausted shared allowance.
    assert!(matches!(budget.begin_phase(), Err(Error::Budget("work"))));
}

#[test]
fn snapshot_per_phase_bytes_and_phase_count_are_independently_bounded() {
    let project = Project::new();
    project.model(&[("src", "kotlin-source")]);
    let source = vec![b'a'; Limits::default().source_file_bytes];
    for index in 0..128 {
        project.put(&format!("src/F{index}.kt"), &source);
    }
    let inventory = project.load().unwrap();
    let mut indices = kotlin_indices(&inventory);
    let cancelled = AtomicBool::new(false);
    let mut budget = SnapshotBudget::default();
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    // Selection order is unrestricted; uniqueness, not sorting, is required.
    indices.reverse();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    assert!(matches!(
        inventory.verify_inventory(&indices[..1], &mut budget, &cancelled),
        Err(Error::Budget("per-phase verification source bytes"))
    ));
    // A refused byte visit is not refunded into the aggregate second phase.
    budget.begin_phase().unwrap();
    inventory
        .verify_inventory(&indices, &mut budget, &cancelled)
        .unwrap();
    assert!(matches!(
        inventory.verify_inventory(&indices, &mut budget, &cancelled),
        Err(Error::Budget("verification source bytes"))
    ));
    assert!(matches!(
        budget.begin_phase(),
        Err(Error::Budget("verification phases"))
    ));
}

#[test]
fn snapshot_two_complete_project_passes_fit_the_original_project_admission() {
    let projects: Vec<_> = (0..Limits::default().projects)
        .map(|_| {
            let project = Project::new();
            project.model(&[]);
            project
        })
        .collect();
    let mut admission = Budget::default();
    let cancelled = AtomicBool::new(false);
    let inventories: Vec<_> = projects
        .iter()
        .map(|project| source_inventory::load(&project.0, &mut admission, &cancelled).unwrap())
        .collect();
    verify_two_phases(&inventories, SnapshotLimits::default()).unwrap();
}
