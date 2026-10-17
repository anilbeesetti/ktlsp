use ktlsp_import::maven::{self, Error, Pom};
use ktlsp_project::{DependencyScope, SourceRootKind};
use std::path::PathBuf;

const SIMPLE: &str = "<project><modelVersion>4.0.0</modelVersion><groupId>org.example</groupId><artifactId>app</artifactId><version>1.0</version></project>";
fn base() -> PathBuf {
    std::env::current_dir().unwrap().join("maven-snapshot")
}
fn import(text: &str) -> Result<maven::ImportedMaven, Error> {
    maven::from_poms(
        base(),
        &[Pom {
            path: "pom.xml",
            bytes: text.as_bytes(),
        }],
    )
}
fn with(body: &str) -> String {
    SIMPLE.replace("</project>", &format!("{body}</project>"))
}
fn unsupported(text: &str) {
    assert!(
        matches!(import(text), Err(Error::Unsupported { .. })),
        "{text}"
    );
}

#[test]
fn reactor_preserves_order_scopes_provenance_and_source_only_projection() {
    let root = include_bytes!("fixtures/maven/pom.xml");
    let core = include_bytes!("fixtures/maven/core/pom.xml");
    let app = include_bytes!("fixtures/maven/app/pom.xml");
    // Caller order is deliberately unrelated to declared reactor order.
    let result = maven::from_poms(
        base(),
        &[
            Pom {
                path: "app/pom.xml",
                bytes: app,
            },
            Pom {
                path: "pom.xml",
                bytes: root,
            },
            Pom {
                path: "core/pom.xml",
                bytes: core,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        result
            .modules()
            .iter()
            .map(|m| m.coordinate.artifact.as_str())
            .collect::<Vec<_>>(),
        ["reactor", "core", "app"]
    );
    let modules = result.modules();
    assert_eq!(modules[0].packaging, "pom");
    assert!(modules[0].source_roots.is_empty());
    assert_eq!(
        modules[1].coordinate,
        maven::Coordinate {
            group: "org.ktlsp.fixture".into(),
            artifact: "core".into(),
            version: "1.0.0".into()
        }
    );
    assert_eq!(
        modules[1].source_roots[0].path,
        base().join("core/src/main/java")
    );
    assert_eq!(modules[1].source_roots[1].kind, SourceRootKind::Test);
    assert!(
        modules[1]
            .source_roots
            .iter()
            .all(|r| r.source.range.is_none())
    );
    assert_eq!(
        modules[2].source_roots[0].path,
        base().join("app/sources/日本")
    );
    assert_eq!(modules[2].source_roots[1].path, base().join("app/tests"));
    let source = &modules[2].source_roots[0].source;
    assert_eq!(source.pom, base().join("app/pom.xml"));
    assert_eq!(
        &app[source.range.clone().unwrap()],
        "<sourceDirectory>sources/日本</sourceDirectory>".as_bytes()
    );
    let deps = &modules[1].dependencies;
    assert_eq!(
        deps.iter().map(|d| d.scope).collect::<Vec<_>>(),
        [
            DependencyScope::Compile,
            DependencyScope::Test,
            DependencyScope::Runtime,
            DependencyScope::Provided
        ]
    );
    assert_eq!(
        deps.iter().map(|d| d.scope_defaulted).collect::<Vec<_>>(),
        [true, false, false, false]
    );
    assert_eq!(deps[0].coordinate, deps[3].coordinate);
    for dep in deps {
        assert_eq!(dep.source.pom, base().join("core/pom.xml"));
        let raw = &core[dep.source.range.clone().unwrap()];
        assert!(raw.starts_with(b"<dependency>") && raw.ends_with(b"</dependency>"));
    }
    assert_eq!(modules[2].dependencies[0].coordinate, modules[1].coordinate);
    assert!(!modules[2].dependencies[0].scope_defaulted);
    let projection = result.source_projection();
    assert!(projection.libraries.is_empty() && projection.sdk_archives.is_empty());
    assert!(projection.modules.iter().all(|m| m.dependencies.is_empty()
        && m.libraries.is_empty()
        && m.language_settings == Default::default()));
    assert_eq!(projection.modules[1].id, "org.ktlsp.fixture:core:1.0.0");
    assert_eq!(
        projection.modules[2].source_roots[0].path,
        modules[2].source_roots[0].path
    );
    let json = serde_json::to_vec(projection).unwrap();
    assert_eq!(
        *projection,
        ktlsp_project::from_json(base().join("unused.json"), &json).unwrap()
    );
}

#[test]
fn defaults_empty_lists_and_literal_comments() {
    for text in [
        SIMPLE.to_owned(),
        format!("\u{feff}{SIMPLE}"),
        with("<build/><dependencies/><modules/>"),
        format!("\u{feff}<?xml version='1.0' encoding='utf-8' standalone='yes'?>{SIMPLE}"),
        SIMPLE.replace("org.example", "org.<!-- note -->example"),
    ] {
        let result = import(&text).unwrap();
        assert_eq!(result.modules()[0].packaging, "jar");
        assert!(result.modules()[0].dependencies.is_empty());
        assert_eq!(
            result.modules()[0].source_roots[0].path,
            base().join("src/main/java")
        );
        let range = result.modules()[0].source.range.clone().unwrap();
        assert!(text[range].starts_with("<project>"));
    }
    let result = import(&with("<packaging>pom</packaging><build/>")).unwrap();
    assert!(result.modules()[0].source_roots.is_empty());
}

#[test]
fn unsupported_maven_semantics_never_return_partial_model() {
    for field in [
        "parent",
        "properties",
        "dependencyManagement",
        "profiles",
        "repositories",
        "pluginRepositories",
        "reporting",
        "name",
        "description",
        "url",
    ] {
        unsupported(&with(&format!("<{field}/>")));
    }
    for field in [
        "plugins",
        "pluginManagement",
        "extensions",
        "resources",
        "testResources",
        "directory",
        "outputDirectory",
        "finalName",
    ] {
        unsupported(&with(&format!("<build><{field}/></build>")));
    }
    for body in [
        "<classifier>tests</classifier>",
        "<exclusions/>",
        "<systemPath>/outside</systemPath>",
        "<type>pom</type>",
        "<optional>true</optional>",
        "<scope>system</scope>",
        "<scope>import</scope>",
        "<scope>Compile</scope>",
    ] {
        unsupported(&with(&format!(
            "<dependencies><dependency><groupId>x</groupId><artifactId>y</artifactId><version>1</version>{body}</dependency></dependencies>"
        )));
    }
    unsupported(&with("<packaging>war</packaging>"));
    unsupported(&with(
        "<packaging>pom</packaging><build><sourceDirectory>src</sourceDirectory></build>",
    ));
    unsupported(&with("<modules><module>child</module></modules>"));
    for version in [
        "${revision}",
        "1.0-SNAPSHOT",
        "SNAPSHOT",
        "1SNAPSHOT",
        "1.0-snapshot",
        "1.0-SnApShOt",
        "latest",
        "Release",
        "LATEST",
        "RELEASE",
        "[1,2)",
        "(1)",
        "1+",
        ".",
        "..",
    ] {
        unsupported(&SIMPLE.replace(
            "<version>1.0</version>",
            &format!("<version>{version}</version>"),
        ));
        unsupported(&with(&format!(
            "<dependencies><dependency><groupId>x</groupId><artifactId>y</artifactId><version>{version}</version></dependency></dependencies>"
        )));
    }
    unsupported(&with(
        "<build><sourceDirectory>${basedir}/src</sourceDirectory></build>",
    ));
}

#[test]
fn malformed_schema_xml_namespaces_attributes_and_declarations_reject() {
    for text in [
        String::new(), "<project>".into(), SIMPLE.replace("</groupId>", "</artifactId>"), format!("{SIMPLE}{SIMPLE}"), format!("text{SIMPLE}"),
        with("<version>2</version>"), SIMPLE.replace("<version>1.0</version>", ""), SIMPLE.replace("1.0</version>", "</version>"),
        with("<build>text</build>"), SIMPLE.replace("org.example", "org.<x/>example"),
        SIMPLE.replace("<project>", "<project xmlns='other'>"), SIMPLE.replace("<project>", "<p:project>"),
        SIMPLE.replace("<project>", "<project xmlns='http://maven.apache.org/POM/4.0.0' xmlns='http://maven.apache.org/POM/4.0.0'>"),
        SIMPLE.replace("<groupId>", "<groupId xmlns='other'>"), SIMPLE.replace("<groupId>", "<groupId attr='x'>"),
        SIMPLE.replace("<project>", "<project xsi:schemaLocation='http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd'>"),
        format!(" <?xml version='1.0'?>{SIMPLE}"), format!("<!-- comment --><?xml version='1.0'?>{SIMPLE}"),
        format!("<?xml encoding='UTF-8' version='1.0'?>{SIMPLE}"), format!("<?xml version='1.0' version='1.0'?>{SIMPLE}"),
        format!("<?xml version='1.0'encoding='UTF-8'?>{SIMPLE}"),
        SIMPLE.replace("<project>", "<project xmlns='http://maven.apache.org/POM/4.0.0'xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>"),
        format!("<?xml version='1.0' standalone='yes' encoding='UTF-8'?>{SIMPLE}"),
        format!("<?xml version='1.1'?>{SIMPLE}"), format!("<?xml version='1.0' encoding='UTF-16'?>{SIMPLE}"),
        format!("<?xml version='1.0' unknown='yes'?>{SIMPLE}"), format!("{SIMPLE}<?xml version='1.0'?>"),
        format!("<!-- a--b -->{SIMPLE}"), SIMPLE.replace("org.example", "org.]]>example"),
        SIMPLE.replace("<project>", "<project xmlns='http://maven.apache.org/POM/4.0.0&amp;'>"),
    ] {
        assert!(import(&text).is_err(), "accepted {text:?}");
    }
    for character in ['\0', '\u{1}', '\u{b}', '\u{fffe}', '\u{ffff}'] {
        for text in [
            format!("<!-- {character} -->{SIMPLE}"),
            SIMPLE.replace("org.example", &format!("org.{character}example")),
        ] {
            assert!(import(&text).is_err());
        }
    }
    for character in ['\u{85}', '\u{a0}'] {
        assert!(import(&with(&format!("<build>{character}</build>"))).is_err());
    }
    assert!(
        maven::from_poms(
            base(),
            &[Pom {
                path: "pom.xml",
                bytes: &[0xff]
            }]
        )
        .is_err()
    );
}

#[test]
fn dtd_external_entities_and_all_references_are_refused() {
    for prefix in [
        "<!DOCTYPE project>",
        "<!DOCTYPE project SYSTEM 'file:///etc/passwd'>",
        "<!DOCTYPE project [<!ENTITY x 'value'>]>",
        "<!DOCTYPE project [<!ENTITY x SYSTEM 'https://example.invalid/secret'>]>",
        "<?tool action?>",
    ] {
        unsupported(&format!("{prefix}{SIMPLE}"));
    }
    for value in [
        "&amp;",
        "&unknown;",
        "&#65;",
        "&#x41;",
        "<![CDATA[org.example]]>",
    ] {
        unsupported(&SIMPLE.replace("org.example", value));
    }
    assert!(import(&SIMPLE.replace("org.example", "org.&example")).is_err());
}

#[test]
fn relative_paths_missing_reused_unreachable_and_duplicate_poms_reject() {
    for path in [
        "",
        ".",
        "..",
        "../escape",
        "a/../b",
        "a/./b",
        "/outside",
        "a//b",
        "a/",
        "C:/outside",
        "a\\b",
        "https://example.invalid",
        "a\0b",
        "a\nb",
        "*.xml",
        "${root}",
    ] {
        assert!(
            import(&with(&format!(
                "<build><sourceDirectory>{path}</sourceDirectory></build>"
            )))
            .is_err(),
            "{path}"
        );
        assert!(
            import(&with(&format!(
                "<packaging>pom</packaging><modules><module>{path}</module></modules>"
            )))
            .is_err(),
            "{path}"
        );
    }
    assert!(
        maven::from_poms(
            "relative",
            &[Pom {
                path: "pom.xml",
                bytes: SIMPLE.as_bytes()
            }]
        )
        .is_err()
    );
    assert!(
        maven::from_poms(
            base().join(".."),
            &[Pom {
                path: "pom.xml",
                bytes: SIMPLE.as_bytes()
            }]
        )
        .is_err()
    );
    for path in [
        "../pom.xml",
        "/pom.xml",
        "other.xml",
        "a/../pom.xml",
        "a\\pom.xml",
    ] {
        assert!(
            maven::from_poms(
                base(),
                &[Pom {
                    path,
                    bytes: SIMPLE.as_bytes()
                }]
            )
            .is_err()
        );
    }
    assert!(maven::from_poms(base(), &[]).is_err());
    let root = with("<packaging>pom</packaging><modules><module>child</module></modules>");
    assert!(import(&root).is_err());
    assert!(
        maven::from_poms(
            base(),
            &[
                Pom {
                    path: "pom.xml",
                    bytes: SIMPLE.as_bytes()
                },
                Pom {
                    path: "pom.xml",
                    bytes: SIMPLE.as_bytes()
                }
            ]
        )
        .is_err()
    );
    assert!(
        maven::from_poms(
            base(),
            &[
                Pom {
                    path: "pom.xml",
                    bytes: SIMPLE.as_bytes()
                },
                Pom {
                    path: "child/pom.xml",
                    bytes: SIMPLE.as_bytes()
                }
            ]
        )
        .is_err()
    );
    assert!(
        maven::from_poms(
            base(),
            &[
                Pom {
                    path: "pom.xml",
                    bytes: root.as_bytes()
                },
                Pom {
                    path: "child/pom.xml",
                    bytes: SIMPLE.as_bytes()
                }
            ]
        )
        .is_err()
    ); // duplicate GAV
    let repeated = root.replace(
        "<module>child</module>",
        "<module>child</module><module>child</module>",
    );
    let child = SIMPLE.replace("<artifactId>app", "<artifactId>child");
    assert!(
        maven::from_poms(
            base(),
            &[
                Pom {
                    path: "pom.xml",
                    bytes: repeated.as_bytes()
                },
                Pom {
                    path: "child/pom.xml",
                    bytes: child.as_bytes()
                }
            ]
        )
        .is_err()
    );
}

#[test]
fn nested_reactor_preorder_is_independent_of_input_order() {
    let root =
        with("<packaging>pom</packaging><modules><module>a</module><module>b</module></modules>");
    let a = with("<packaging>pom</packaging><modules><module>nested</module></modules>")
        .replace("<artifactId>app", "<artifactId>a");
    let nested = SIMPLE.replace("<artifactId>app", "<artifactId>nested");
    let b = SIMPLE.replace("<artifactId>app", "<artifactId>b");
    let result = maven::from_poms(
        base(),
        &[
            Pom {
                path: "a/nested/pom.xml",
                bytes: nested.as_bytes(),
            },
            Pom {
                path: "b/pom.xml",
                bytes: b.as_bytes(),
            },
            Pom {
                path: "a/pom.xml",
                bytes: a.as_bytes(),
            },
            Pom {
                path: "pom.xml",
                bytes: root.as_bytes(),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        result
            .modules()
            .iter()
            .map(|m| m.coordinate.artifact.as_str())
            .collect::<Vec<_>>(),
        ["app", "a", "nested", "b"]
    );
}

#[test]
fn source_projection_runs_existing_overlap_validation() {
    assert!(matches!(
        import(&with(
            "<build><sourceDirectory>src</sourceDirectory><testSourceDirectory>src/tests</testSourceDirectory></build>"
        )),
        Err(Error::Project(
            ktlsp_project::ProjectError::SourceRootOverlap { .. }
        ))
    ));
    assert!(matches!(
        import(&with(
            "<build><sourceDirectory>src</sourceDirectory><testSourceDirectory>src</testSourceDirectory></build>"
        )),
        Err(Error::Project(
            ktlsp_project::ProjectError::SourceRootOverlap { .. }
        ))
    ));
}

#[test]
fn byte_event_depth_and_path_limits_are_checked() {
    let exact = format!(
        "{SIMPLE}{}",
        " ".repeat(maven::MAX_POM_BYTES - SIMPLE.len())
    );
    assert!(import(&exact).is_ok());
    assert!(matches!(
        import(&(exact + " ")),
        Err(Error::Budget("POM bytes"))
    ));
    let poms: Vec<_> = (0..=maven::MAX_POMS)
        .map(|_| Pom {
            path: "pom.xml",
            bytes: b"",
        })
        .collect();
    assert!(matches!(
        maven::from_poms(base(), &poms),
        Err(Error::Budget("POM count"))
    ));
    let names: Vec<_> = (0..17).map(|i| format!("{i}/pom.xml")).collect();
    let large = vec![b' '; maven::MAX_POM_BYTES];
    let poms: Vec<_> = names
        .iter()
        .map(|path| Pom {
            path,
            bytes: &large,
        })
        .collect();
    assert!(matches!(
        maven::from_poms(base(), &poms),
        Err(Error::Budget("total POM bytes"))
    ));
    for depth in [maven::MAX_DEPTH, maven::MAX_DEPTH + 1, 5000] {
        let text = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
        let error = import(&text).unwrap_err();
        if depth == maven::MAX_DEPTH {
            assert!(!matches!(error, Error::Budget("XML depth")));
        } else {
            assert!(matches!(error, Error::Budget("XML depth")));
        }
    }
    // SIMPLE has fifteen events: project pair, four scalar triples, EOF.
    // Compute the fixed count independently from its authored element/text structure.
    let events = 2 + 4 * 3 + 1;
    let exact = format!("{}{}", "<!---->".repeat(maven::MAX_EVENTS - events), SIMPLE);
    assert!(import(&exact).is_ok());
    assert!(matches!(
        import(&("<!---->".to_owned() + &exact)),
        Err(Error::Budget("XML events"))
    ));
    let long = "a".repeat(maven::MAX_PATH_BYTES + 1);
    assert!(matches!(
        import(&with(&format!(
            "<build><sourceDirectory>{long}</sourceDirectory></build>"
        ))),
        Err(Error::Budget("path bytes"))
    ));
    let huge_base = base().join(&long);
    assert!(matches!(
        maven::from_poms(
            huge_base,
            &[Pom {
                path: "pom.xml",
                bytes: SIMPLE.as_bytes()
            }]
        ),
        Err(Error::Budget("caller path bytes"))
    ));
}

#[test]
fn deterministic_truncated_and_mutated_xml_corpus_is_controlled() {
    for end in 0..SIMPLE.len() {
        assert!(import(&SIMPLE[..end]).is_err());
    }
    for (offset, _) in SIMPLE.char_indices() {
        let mut bytes = SIMPLE.as_bytes().to_vec();
        bytes[offset] = b'\0';
        assert!(
            maven::from_poms(
                base(),
                &[Pom {
                    path: "pom.xml",
                    bytes: &bytes
                }]
            )
            .is_err()
        );
    }
}

#[test]
fn retained_provenance_paths_share_the_aggregate_budget() {
    let path = base().join("a".repeat(3500));
    let dependency = "<dependency><groupId>x</groupId><artifactId>y</artifactId><version>1</version></dependency>";
    let text = with(&format!(
        "<dependencies>{}</dependencies>",
        dependency.repeat(350)
    ));
    assert!(text.len() < maven::MAX_POM_BYTES);
    assert!(matches!(
        maven::from_poms(
            path,
            &[Pom {
                path: "pom.xml",
                bytes: text.as_bytes()
            }]
        ),
        Err(Error::Budget("total expanded path bytes"))
    ));
    let text = with(&format!(
        "<dependencies>{}</dependencies>",
        dependency.repeat(200)
    ));
    assert!(
        maven::from_poms(
            base().join("a".repeat(3500)),
            &[Pom {
                path: "pom.xml",
                bytes: text.as_bytes()
            }]
        )
        .is_ok()
    );
}

#[test]
fn bom_provenance_ranges_address_the_original_snapshot() {
    let body = "<build><sourceDirectory>sources/日本</sourceDirectory></build><dependencies><dependency><groupId>x</groupId><artifactId>y</artifactId><version>1</version></dependency></dependencies>";
    for prefix in [
        "",
        "\u{feff}",
        "<?xml version='1.0'?>",
        "\u{feff}<?xml version='1.0'?>",
    ] {
        let xml = format!("{prefix}{}", with(body));
        let result = import(&xml).unwrap();
        let module = &result.modules()[0];
        assert_eq!(module.source.range, Some(prefix.len()..xml.len()));
        let root = &module.source_roots[0].source;
        assert_eq!(
            &xml[root.range.clone().unwrap()],
            "<sourceDirectory>sources/日本</sourceDirectory>"
        );
        let dependency = &module.dependencies[0].source;
        assert_eq!(
            &xml[dependency.range.clone().unwrap()],
            "<dependency><groupId>x</groupId><artifactId>y</artifactId><version>1</version></dependency>"
        );
    }
    for prefix in ["\u{feff} <?xml version='1.0'?>", "\u{feff}\u{feff}"] {
        assert!(import(&format!("{prefix}{SIMPLE}")).is_err());
    }
}
