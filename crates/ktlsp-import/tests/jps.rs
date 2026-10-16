use ktlsp_import::jps::{self, Document, Error};
use ktlsp_project::SourceRootKind::{Production, Test};
use std::path::PathBuf;

const MANIFEST: &[u8] = include_bytes!("fixtures/jps/.idea/modules.xml");
const MODULE: &[u8] = include_bytes!("fixtures/jps/moduleB.iml");
fn base() -> PathBuf { if cfg!(windows) { PathBuf::from("C:/workspace") } else { PathBuf::from("/workspace") } }
fn manifest(paths: &[&str]) -> String {
    format!(r#"<project version="4"><component name="ProjectModuleManager"><modules>{}</modules></component></project>"#,
        paths.iter().map(|p| format!(r#"<module filepath="$PROJECT_DIR$/{p}"/>"#)).collect::<String>())
}
fn module(body: &str) -> String {
    format!(r#"<module type="JAVA_MODULE" version="4"><component name="NewModuleRootManager">{body}</component></module>"#)
}
fn content(roots: &str) -> String { format!(r#"<content url="file://$MODULE_DIR$">{roots}</content>"#) }
fn root(name: &str, attrs: &str) -> String { format!(r#"<sourceFolder url="file://$MODULE_DIR$/{name}" {attrs}/>"#) }
fn import(manifest: &str, module: &str) -> Result<jps::ImportedSources, Error> {
    jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: manifest.as_bytes() }, Document { path: "a.iml", bytes: module.as_bytes() }])
}
fn refusal(body: &str) {
    assert!(import(&manifest(&["a.iml"]), &module(body)).is_err(), "accepted {body}");
}

#[test]
fn genuine_original_xml_preserves_original_ranges_and_real_model_roundtrip() {
    let result = jps::source_projection(base(), &[Document { path: "moduleB.iml", bytes: MODULE }, Document { path: ".idea/modules.xml", bytes: MANIFEST }]).unwrap();
    let module = &result.modules[0];
    assert_eq!(module.id, "moduleB");
    assert_eq!(module.source.document, base().join("moduleB.iml"));
    assert_eq!(module.declaration.document, base().join(".idea/modules.xml"));
    assert_eq!(&MANIFEST[module.declaration.range.clone()], br#"<module fileurl="file://$PROJECT_DIR$/moduleB.iml" filepath="$PROJECT_DIR$/moduleB.iml" />"#);
    assert!(MODULE[module.source.range.clone()].starts_with(b"<module type="));
    assert!(MODULE[module.source.range.clone()].ends_with(b"</module>"));
    assert_eq!(module.source_roots.len(), 2);
    for (root, path, kind, literal) in [
        (&module.source_roots[0], "src", Production, br#"<sourceFolder url="file://$MODULE_DIR$/src" isTestSource="false" />"#.as_slice()),
        (&module.source_roots[1], "test", Test, br#"<sourceFolder url="file://$MODULE_DIR$/test" isTestSource="true" />"#.as_slice()),
    ] {
        assert_eq!(root.path, base().join(path)); assert_eq!(root.kind, kind); assert!(!root.generated);
        assert_eq!(root.source.document, base().join("moduleB.iml")); assert_eq!(&MODULE[root.source.range.clone()], literal);
    }
    let projection = result.declared_source_projection();
    let bytes = serde_json::to_vec(projection).unwrap();
    let reloaded = ktlsp_project::from_json(base().join("project.json"), &bytes).unwrap();
    assert_eq!(*projection, reloaded);
    assert_eq!(projection.modules[0].source_roots[0].path, base().join("src"));
    assert_eq!(projection.modules[0].source_roots[1].kind, Test);
    assert!(projection.modules[0].dependencies.is_empty());
}

#[test]
fn nested_generated_default_type_precedence_and_input_order_are_declared_order() {
    let m = manifest(&["nested/z.iml", "a.iml"]);
    let z = module(&content(&[
        root("default", ""), root("test", r#"isTestSource="true""#),
        root("explicit-prod", r#"type="java-source" isTestSource="true" generated="true""#),
        root("explicit-test", r#"type="java-test" isTestSource="false" packagePrefix="""#),
    ].concat()));
    let a = module("");
    let first = jps::source_projection(base(), &[Document { path: "a.iml", bytes: a.as_bytes() }, Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "nested/z.iml", bytes: z.as_bytes() }]).unwrap();
    let second = jps::source_projection(base(), &[Document { path: "nested/z.iml", bytes: z.as_bytes() }, Document { path: "./.idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a.iml", bytes: a.as_bytes() }]).unwrap();
    assert_eq!(first.modules, second.modules);
    assert_eq!(first.modules.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["z", "a"]);
    let roots = &first.modules[0].source_roots;
    assert_eq!(roots.iter().map(|r| r.kind).collect::<Vec<_>>(), [Production, Test, Production, Test]);
    assert!(roots[2].generated);
    assert_eq!(roots[0].path, base().join("nested/default"));
    assert!(first.modules[1].source_roots.is_empty());
    assert_eq!(serde_json::to_vec(first.declared_source_projection()).unwrap(), serde_json::to_vec(second.declared_source_projection()).unwrap());
}

#[test]
fn bom_unicode_xml_references_and_url_encoding_have_original_byte_provenance() {
    let m = "\u{feff}<project version=\"4\"><component name=\"ProjectModuleManager\"><modules><module filepath=\"$PROJECT_DIR$/a &amp; λ%.iml\" fileurl=\"file://$PROJECT_DIR$/a%20%26%20%CE%BB%25.iml\"/></modules></component></project>";
    let text = format!("\u{feff}{}", module(&content(&root("é%20&amp;&#x03bb;&#955;", r#"generated="true""#))));
    let result = jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a & λ%.iml", bytes: text.as_bytes() }]).unwrap();
    let module = &result.modules[0];
    assert_eq!(module.id, "a & λ%");
    assert_eq!(module.source.range.start, 3);
    let source_root = &module.source_roots[0];
    assert_eq!(source_root.path, base().join("é &λλ"));
    assert_eq!(&text.as_bytes()[source_root.source.range.clone()], root("é%20&amp;&#x03bb;&#955;", r#"generated="true""#).as_bytes());
    assert!(m.as_bytes()[module.declaration.range.clone()].starts_with(b"<module filepath="));
}

#[test]
fn all_snapshot_identity_and_late_failures_are_atomic() {
    let m = manifest(&["a.iml", "b.iml"]); let a = module(&content(&root("src", "")));
    assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a.iml", bytes: a.as_bytes() }]).is_err());
    for late in ["<module", "<!DOCTYPE x><module/>", "<module/>"] {
        assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a.iml", bytes: a.as_bytes() }, Document { path: "b.iml", bytes: late.as_bytes() }]).is_err());
    }
    let m = manifest(&["a.iml"]);
    assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a.iml", bytes: a.as_bytes() }, Document { path: "x/../a.iml", bytes: a.as_bytes() }]).is_err());
    assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a.iml", bytes: a.as_bytes() }, Document { path: "unused.iml", bytes: a.as_bytes() }]).is_err());
    let m = manifest(&["one/a.iml", "two/a.iml"]);
    assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "one/a.iml", bytes: a.as_bytes() }, Document { path: "two/a.iml", bytes: a.as_bytes() }]).is_err());
    assert!(import(&manifest(&["a.iml", "x/../a.iml"]), &a).is_err());
}

#[test]
fn unrepresentable_source_semantics_and_intersections_never_flatten() {
    for body in [
        content(&root("src", r#"packagePrefix="p""#)), content(&root("src", r#"type="java-resource""#)),
        content(&root("src", r#"type="java-test-resource""#)), content(&root("src", r#"type="custom""#)),
        content(r#"<excludeFolder url="file://$MODULE_DIR$/src"/>"#), content(r#"<excludePattern pattern="*.kt"/>"#),
        content(r#"<sourceFolder url="file://$MODULE_DIR$/src" strange=""/>"#),
        r#"<facet name="Kotlin"/>"#.into(), r#"<unknown/>"#.into(),
        format!("{}<output url=\"file://$MODULE_DIR$/src/out\"/><exclude-output/>", content(&root("src", ""))),
        format!("<exclude-output/>{}<output url=\"file://$MODULE_DIR$/src\"/>", content(&root("src/nested", ""))),
    ] { refusal(&body); }
    let m = manifest(&["a.iml"]);
    assert!(import(&m, &module(&content(&[root("src", ""), root("src/test", r#"isTestSource="true""#)].concat()))).is_err());
    assert!(import(&m, &module(&content(&[root("src", ""), root("src", "")].concat()))).is_err());
    assert!(import(&m, &module(&content(&[root("src", ""), root("src/generated", r#"generated="true""#)].concat()))).is_err());
    let m = manifest(&["a.iml", "b.iml"]); let a = module(&content(&root("src", "")));
    assert!(matches!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a.iml", bytes: a.as_bytes() }, Document { path: "b.iml", bytes: a.as_bytes() }]), Err(Error::Project(_))));
}

#[test]
fn malformed_xml_and_security_boundaries_refuse() {
    for xml in [
        "", "<module", "<module/><module/>", "<!DOCTYPE module><module/>",
        "<!DOCTYPE module [<!ENTITY x SYSTEM 'file:///secret'>]><module>&x;</module>",
        "<?xml version='1.0' encoding='UTF-16'?><module/>", "<?xml version='1.1'?><module/>",
        "<?xml encoding='UTF-8' version='1.0'?><module/>", " <?xml version='1.0'?><module/>",
        "<?xml version='&#49;.0'?><module/>", "<?foreign x?><module/>",
        "<module type='JAVA_MODULE' type='JAVA_MODULE' version='4'/>",
        "<module type='JAVA_MODULE'version='4'/>", "<module xmlns='foreign'/>",
        "<module xmlns:xi='http://www.w3.org/2001/XInclude'><xi:include href='x'/></module>",
        "<module><!-- bad -- comment --></module>", "<module><![CDATA[ ]]></module>",
        "<module>&amp;</module>", "<module>]]></module>", "<module>\u{1}</module>",
        "<module type='&unknown;'/>", "<module type='&#0;'/>", "<module type='&#xD800;'/>",
        "<module type='&#xFFFE;'/>", "<module type='&#-1;'/>", "<module type='&#x110000;'/>",
        "<module type='&#99999999999999999999999;'/>", "<module type='a<b'/>",
        "<module type='&amp'/>", "<module><x></module>",
    ] { assert!(import(&manifest(&["a.iml"]), xml).is_err(), "accepted {xml}"); }
    assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: MANIFEST }, Document { path: "moduleB.iml", bytes: &[0xff] }]).is_err());
}

#[test]
fn path_macros_urls_and_filepaths_are_not_interchangeable() {
    for value in ["file://$UNKNOWN$/src", "file://$MODULE_DIR$$PROJECT_DIR$/src", "file://$MODULE_DIR$/x/$UNKNOWN$", "file://$MODULE_DIR$evil/src", "file://$MODULE_DIR$//src", "jar://$MODULE_DIR$/x!/src", "file://server/share/src", "file:/$MODULE_DIR$/src", "file://$MODULE_DIR$/src?x", "file://$MODULE_DIR$/src#x", "file://$MODULE_DIR$/%2Fsrc", "file://$MODULE_DIR$/%5csrc", "file://$MODULE_DIR$/%00", "file://$MODULE_DIR$/%24UNKNOWN%24", "file://$MODULE_DIR$/%FF", "file://$MODULE_DIR$/%xx", "file://$MODULE_DIR$/%", "file://$MODULE_DIR$/%λ"] {
        refusal(&content(&format!(r#"<sourceFolder url="{value}"/>"#)));
    }
    let m = r#"<project version="4"><component name="ProjectModuleManager"><modules><module filepath="$PROJECT_DIR$/a.iml" fileurl="file://$PROJECT_DIR$/b.iml"/></modules></component></project>"#;
    assert!(import(m, &module("")).is_err());
    for key in ["$PROJECT_DIR$/a.iml", "file:///a.iml", "\\\\host\\a.iml"] {
        assert!(jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: MANIFEST }, Document { path: key, bytes: MODULE }]).is_err());
    }
    let m = manifest(&["a%20.iml"]);
    let result = jps::source_projection(base(), &[Document { path: ".idea/modules.xml", bytes: m.as_bytes() }, Document { path: "a%20.iml", bytes: module("").as_bytes() }]).unwrap();
    assert_eq!(result.modules[0].id, "a%20");
    assert!(jps::source_projection("relative", &[]).is_err());
}

#[test]
fn aggregate_event_budget_is_not_reset_between_documents() {
    // Compact nonempty manifest = 8 events; empty-body module = 5 events.
    let compact = manifest(&["a.iml"]); let a = module("");
    let exact = format!("{}{}", "<!---->".repeat(jps::MAX_EVENTS - 13), compact);
    assert!(import(&exact, &a).is_ok());
    assert!(matches!(import(&(exact + "<!--x-->"), &a), Err(Error::Budget("aggregate XML events"))));
}

#[test]
fn exact_document_and_aggregate_byte_limits_refuse_next_byte() {
    let m = manifest(&["a.iml"]); let mut a = module("");
    a.extend(std::iter::repeat_n(' ', jps::MAX_DOCUMENT_BYTES - a.len()));
    assert!(import(&m, &a).is_ok());
    assert!(matches!(import(&m, &(a + " ")), Err(Error::Budget("document bytes"))));
    let paths: Vec<_> = (0..15).map(|i| format!("m{i}.iml")).collect();
    let refs: Vec<_> = paths.iter().map(String::as_str).collect();
    let mut m = manifest(&refs); m.extend(std::iter::repeat_n(' ', jps::MAX_DOCUMENT_BYTES - m.len()));
    let mut a = module(""); a.extend(std::iter::repeat_n(' ', jps::MAX_DOCUMENT_BYTES - a.len()));
    let mut docs = vec![Document { path: ".idea/modules.xml", bytes: m.as_bytes() }];
    docs.extend(paths.iter().map(|path| Document { path, bytes: a.as_bytes() }));
    assert_eq!(docs.iter().map(|d| d.bytes.len()).sum::<usize>(), jps::MAX_TOTAL_BYTES);
    assert!(jps::source_projection(base(), &docs).is_ok());
    docs.push(Document { path: "extra.iml", bytes: b" " });
    assert!(matches!(jps::source_projection(base(), &docs), Err(Error::Budget("aggregate document bytes"))));
}

#[test]
fn exact_snapshot_reference_count_and_next_document_are_bounded() {
    let paths: Vec<_> = (0..jps::MAX_DOCUMENTS - 1).map(|i| format!("m{i}.iml")).collect();
    let refs: Vec<_> = paths.iter().map(String::as_str).collect();
    let m = manifest(&refs); let a = module("");
    let mut docs = vec![Document { path: ".idea/modules.xml", bytes: m.as_bytes() }];
    docs.extend(paths.iter().map(|path| Document { path, bytes: a.as_bytes() }));
    assert_eq!(jps::source_projection(base(), &docs).unwrap().modules.len(), jps::MAX_DOCUMENTS - 1);
    docs.push(Document { path: "extra.iml", bytes: a.as_bytes() });
    assert!(matches!(jps::source_projection(base(), &docs), Err(Error::Budget("documents"))));
}

#[test]
fn lexical_parent_macros_absolute_urls_and_same_kind_nested_roots_roundtrip() {
    let project = base();
    let absolute = project.join("src").to_str().unwrap().replace('\\', "/");
    let absolute_url = if cfg!(windows) { format!("file:///{absolute}") } else { format!("file://{absolute}") };
    let body = format!(
        r#"<content url="file://$MODULE_DIR$/../"><sourceFolder url="{absolute_url}"/><sourceFolder url="file://$MODULE_DIR$/../src/nested"/></content>"#,
    );
    let m = manifest(&["nested/a.iml"]);
    let a = module(&body);
    let result = jps::source_projection(&project, &[
        Document { path: ".idea/./modules.xml", bytes: m.as_bytes() },
        Document { path: "nested/a.iml", bytes: a.as_bytes() },
    ]).unwrap();
    assert_eq!(result.modules[0].source_roots[0].path, project.join("src"));
    assert_eq!(result.modules[0].source_roots[1].path, project.join("src/nested"));
    let bytes = serde_json::to_vec(result.declared_source_projection()).unwrap();
    assert_eq!(*result.declared_source_projection(), ktlsp_project::from_json(project.join("model.json"), &bytes).unwrap());
}

#[test]
fn oversized_invalid_snapshot_keys_refuse_before_diagnostic_retention() {
    let key = format!("${}", "x".repeat(jps::MAX_PATH_BYTES));
    assert!(matches!(jps::source_projection(base(), &[Document { path: &key, bytes: b"" }]), Err(Error::Budget("path bytes"))));
}

#[test]
fn trailing_separator_on_supported_macro_is_the_same_declared_path() {
    let body = r#"<content url="file://$MODULE_DIR$/"><sourceFolder url="file://$MODULE_DIR$/"/></content>"#;
    let result = import(&manifest(&["a.iml"]), &module(body)).unwrap();
    assert_eq!(result.modules[0].source_roots[0].path, base());
    assert_eq!(result.declared_source_projection().modules[0].source_roots[0].path, base());
}

#[cfg(windows)]
#[test]
fn macro_suffix_cannot_replace_project_with_an_absolute_drive() {
    let m = manifest(&["D:/a.iml"]);
    let a = module("");
    assert!(matches!(jps::source_projection(base(), &[
        Document { path: ".idea/modules.xml", bytes: m.as_bytes() },
        Document { path: "D:/a.iml", bytes: a.as_bytes() },
    ]), Err(Error::Unsupported { reason: "macro suffix must remain relative", .. })));
}
