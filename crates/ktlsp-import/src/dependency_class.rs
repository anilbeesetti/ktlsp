//! Finite source-owner-scoped dependency class selection, not Kotlin resolution.

use crate::v2::ImportedWorkspace;
use ktlsp_artifacts::{
    JmodDirectoryIndex, JmodIndexError, VerifiedArtifact, ZipDirectoryIndex, ZipIndexError,
    index_jmod_directory, index_zip_directory,
};
use ktlsp_binary::class_path::{self, ClasspathClass, ClasspathError, ClasspathRoot};
use ktlsp_project::{DependencyScope, Library, SourceRoot, SourceRootKind, v2::{Dependency, Module}};
use std::{collections::HashMap, path::{Component, Path, PathBuf}};

/// Explicit decoder choice; imported filenames do not establish archive kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveKind {
    Jar,
    Jmod,
}

/// Caller assertion connecting retained bytes to a library's CLASSES root.
/// The artifact's actual retained provenance path must agree lexically with the
/// immutable projection. Digest equality does not authenticate that assertion.
#[derive(Clone, Copy, Debug)]
pub struct ArtifactBinding<'a> {
    pub library: &'a str,
    pub binary_root: usize,
    pub kind: ArchiveKind,
    pub artifact: &'a VerifiedArtifact,
}

#[derive(Clone, Copy, Debug)]
pub struct SourceOwner<'a> {
    pub module: &'a Module,
    pub source_root: &'a SourceRoot,
}

#[derive(Debug)]
pub struct SelectedClass<'model, 'data> {
    /// Ordinal in the projection's mixed dependency sequence (no ModuleSource).
    pub dependency: usize,
    pub exported: bool,
    pub library: &'model Library,
    pub binary_root: usize,
    pub import_path: &'model Path,
    /// Retains immutable digest, bytes, and caller-supplied provenance together.
    pub artifact: &'data VerifiedArtifact,
    pub selected: ClasspathClass<'data>,
}

#[derive(Debug)]
pub struct DependencyClass<'model, 'data> {
    /// Present even when the fully bound dependency archives contain no match.
    pub owner: SourceOwner<'model>,
    pub class: Option<SelectedClass<'model, 'data>>,
}

#[derive(Debug)]
pub enum Error {
    InvalidSourcePath,
    UnownedSource,
    TestSource,
    UnsupportedDependency { dependency: usize },
    SdkArchives,
    Limit(&'static str),
    MissingBinding { library: usize, binary_root: usize },
    DuplicateBinding { library: usize, binary_root: usize },
    BindingPath { library: usize, binary_root: usize },
    UnboundBinding { binding: usize },
    Jar { root: usize, error: ZipIndexError },
    Jmod { root: usize, error: JmodIndexError },
    Classpath(ClasspathError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dependency class: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jar { error, .. } => Some(error),
            Self::Jmod { error, .. } => Some(error),
            Self::Classpath(error) => Some(error),
            _ => None,
        }
    }
}

struct Root<'model, 'data> {
    dependency: usize,
    exported: bool,
    library: &'model Library,
    library_index: usize,
    binary_root: usize,
    binding: Option<ArtifactBinding<'data>>,
}
enum Index<'a> {
    Jar(ZipDirectoryIndex<'a>),
    Jmod(JmodDirectoryIndex<'a>),
}
impl Index<'_> {
    fn zip(&self) -> &ZipDirectoryIndex<'_> {
        match self {
            Self::Jar(index) => index,
            Self::Jmod(index) => index.zip_index(),
        }
    }
}

fn normalized(path: &Path) -> bool {
    let Some(text) = path.to_str() else { return false };
    if text.len() > crate::MAX_EXPANDED_PATH_BYTES
        || text.contains(['\0', '\\', '<'])
        || text.contains("://")
        || !path.is_absolute()
        || path.components().any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return false;
    }
    let canonical: PathBuf = path.components().collect();
    canonical.as_os_str() == path.as_os_str()
}

fn charge(total: &mut usize, amount: usize, max: usize, field: &'static str) -> Result<(), Error> {
    *total = total.checked_add(amount).filter(|&n| n <= max).ok_or(Error::Limit(field))?;
    Ok(())
}

/// Select an explicitly named dependency class for an absolute, normalized source
/// path. Ownership and dependencies use only the private validated projection;
/// public importer sidecar mutation cannot grant visibility or change bindings.
///
/// Only production owners and direct COMPILE libraries are admitted. All owner
/// edges and every binary-root binding/index are checked before class lookup or
/// absence. ModuleSource is intentionally irrelevant to this dependency-only
/// query: no source shadowing, transitive/export propagation, SDK selection,
/// access checking, name resolution, or filesystem/symlink identity is claimed.
///
/// Artifact bytes stay borrowed. Limits are 64 ordered root occurrences, 65,536
/// aggregate directory entries, 16 MiB directory names, and the underlying index
/// and lookup limits. Caller binding scans additionally cap at 65,536 records and
/// 16 MiB library-key bytes, including unrelated bindings (which never add roots).
/// Scratch/result lifetime follows lookup_class; explicit release is never SDK
/// metadata, and the class reader still stops at Java 23/no preview.
pub fn select_class<'model, 'data>(
    workspace: &'model ImportedWorkspace,
    source: &Path,
    name: &str,
    java_release: u16,
    bindings: &[ArtifactBinding<'data>],
    scratch: &'data mut Vec<u8>,
) -> Result<DependencyClass<'model, 'data>, Error> {
    scratch.clear();
    if !normalized(source) {
        return Err(Error::InvalidSourcePath);
    }
    let project = workspace.project_projection();
    let owner = project.modules.iter().find_map(|module| {
        module.source_roots.iter()
            .filter(|root| source.starts_with(&root.path))
            .max_by_key(|root| root.path.components().count())
            .map(|source_root| SourceOwner { module, source_root })
    }).ok_or(Error::UnownedSource)?;
    if owner.source_root.kind != SourceRootKind::Production {
        return Err(Error::TestSource);
    }
    if !project.sdk_archives.is_empty() {
        return Err(Error::SdkArchives);
    }
    // Validate the whole policy before admitting any archive or successful miss.
    for (dependency, edge) in owner.module.dependencies.iter().enumerate() {
        if !matches!(edge, Dependency::Library { scope: DependencyScope::Compile, .. }) {
            return Err(Error::UnsupportedDependency { dependency });
        }
    }
    let mut roots = Vec::new();
    let mut selected_libraries = HashMap::new();
    for (dependency, edge) in owner.module.dependencies.iter().enumerate() {
        let Dependency::Library { library, exported, .. } = edge else { unreachable!() };
        // The immutable projection already validated every reference.
        let (library_index, library) = project.libraries.iter().enumerate()
            .find(|(_, candidate)| candidate.id == *library).expect("validated library reference");
        selected_libraries.insert(library.id.as_str(), library.binaries.len());
        for binary_root in 0..library.binaries.len() {
            if roots.len() == class_path::MAX_ROOTS {
                return Err(Error::Limit("roots"));
            }
            roots.push(Root { dependency, exported: *exported, library, library_index, binary_root, binding: None });
        }
    }
    if bindings.len() > class_path::MAX_ENTRIES {
        return Err(Error::Limit("bindings"));
    }
    let mut key_bytes = 0;
    for (ordinal, binding) in bindings.iter().enumerate() {
        charge(&mut key_bytes, binding.library.len(), class_path::MAX_SCAN_BYTES, "binding key bytes")?;
        if selected_libraries.get(binding.library).is_some_and(|&count| binding.binary_root >= count) {
            return Err(Error::UnboundBinding { binding: ordinal });
        }
        for root in &mut roots {
            if binding.library != root.library.id || binding.binary_root != root.binary_root {
                continue;
            }
            if root.binding.replace(*binding).is_some() {
                return Err(Error::DuplicateBinding { library: root.library_index, binary_root: root.binary_root });
            }
        }
    }
    // Completeness precedes even indexing, including late roots after a winner.
    for root in &roots {
        let binding = root.binding.ok_or(Error::MissingBinding { library: root.library_index, binary_root: root.binary_root })?;
        let path = &binding.artifact.provenance().artifact_path;
        if !normalized(path) || path.as_os_str() != root.library.binaries[root.binary_root].as_os_str() {
            return Err(Error::BindingPath { library: root.library_index, binary_root: root.binary_root });
        }
    }
    let mut indexes = Vec::with_capacity(roots.len());
    let (mut entries, mut names) = (0, 0);
    for (root, input) in roots.iter().enumerate() {
        let binding = input.binding.expect("complete bindings");
        let index = match binding.kind {
            ArchiveKind::Jar => Index::Jar(index_zip_directory(binding.artifact).map_err(|error| Error::Jar { root, error })?),
            ArchiveKind::Jmod => Index::Jmod(index_jmod_directory(binding.artifact).map_err(|error| Error::Jmod { root, error })?),
        };
        charge(&mut entries, index.zip().entries().len(), class_path::MAX_ENTRIES, "entries")?;
        for entry in index.zip().entries() {
            charge(&mut names, entry.raw_name().len(), class_path::MAX_SCAN_BYTES, "scan bytes")?;
        }
        indexes.push(index);
    }
    let classpath: Vec<_> = indexes.iter().map(|index| match index {
        Index::Jar(index) => ClasspathRoot::Jar(index),
        Index::Jmod(index) => ClasspathRoot::Jmod(index),
    }).collect();
    let selected = class_path::lookup_class(&classpath, name, java_release, scratch).map_err(Error::Classpath)?;
    let class = selected.map(|selected| {
        let root = &roots[selected.origin.root];
        SelectedClass {
            dependency: root.dependency,
            exported: root.exported,
            library: root.library,
            binary_root: root.binary_root,
            import_path: &root.library.binaries[root.binary_root],
            artifact: root.binding.expect("complete bindings").artifact,
            selected,
        }
    });
    Ok(DependencyClass { owner, class })
}
