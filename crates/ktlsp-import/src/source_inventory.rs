//! Explicit bounded filesystem admission, separate from the data-only JSON importer.
//!
//! Requires a trusted, quiescent local filesystem. Ancestor checks, native IDs and
//! metadata/open/read/rechecks are not a hostile replacement/mount sandbox; IDs
//! can be reused and OS I/O latency is not bounded. Borrowed projects are read only.
use crate::v2;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    fs::{self, File, Metadata},
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// Match the existing dependency-class boundary's Disk/VerbatimDisk equivalence,
// without case folding or accepting UNC/device paths.
fn native_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::Prefix;
        if let Some(Component::Prefix(prefix)) = path.components().next()
            && let Prefix::Disk(drive) = prefix.kind()
        {
            let mut native = PathBuf::from(format!("\\\\?\\{}:\\", char::from(drive)));
            native.extend(path.components().skip(2));
            return native;
        }
    }
    path.to_path_buf()
}

/// Aggregate candidate ceilings. Callers may reduce, but cannot raise, defaults.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub projects: usize,
    pub model_bytes: usize,
    pub entries: usize,
    pub directories: usize,
    pub depth: usize,
    pub sources: usize,
    pub source_bytes: usize,
    pub source_file_bytes: usize,
    pub path_bytes: usize,
    pub path_file_bytes: usize,
    pub memberships: usize,
    pub work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            projects: 16,
            model_bytes: 2 * 1024 * 1024,
            entries: 16384,
            directories: 4096,
            depth: 32,
            sources: 1024,
            source_bytes: 16 * 1024 * 1024,
            source_file_bytes: 64 * 1024,
            path_bytes: 2 * 1024 * 1024,
            path_file_bytes: 4096,
            memberships: 16384,
            work: 256 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Copy)]
enum Count {
    Projects,
    Model,
    Entries,
    Directories,
    Sources,
    SourceBytes,
    Paths,
    Memberships,
    Work,
}
impl Count {
    fn name(self) -> &'static str {
        match self {
            Self::Projects => "projects",
            Self::Model => "model bytes",
            Self::Entries => "entries",
            Self::Directories => "directories",
            Self::Sources => "sources",
            Self::SourceBytes => "source bytes",
            Self::Paths => "path bytes",
            Self::Memberships => "memberships",
            Self::Work => "work",
        }
    }
}
impl Limits {
    fn counters(self) -> [usize; 9] {
        [
            self.projects,
            self.model_bytes,
            self.entries,
            self.directories,
            self.sources,
            self.source_bytes,
            self.path_bytes,
            self.memberships,
            self.work,
        ]
    }
}
/// One monotonic budget across all candidate loads, or both request verifications.
/// Failed operations never refund charges. Logical work is charged for bytes,
/// path comparisons, validation and filesystem operations, not elapsed time/RSS.
#[derive(Debug, Default)]
pub struct Budget {
    limits: Limits,
    used: [usize; 9],
    physical_owners: HashMap<Identity, usize>,
}
impl Budget {
    pub fn new(limits: Limits) -> Result<Self, Error> {
        let defaults = Limits::default();
        if limits
            .counters()
            .iter()
            .zip(defaults.counters())
            .any(|(a, b)| *a > b)
            || limits.depth > defaults.depth
            || limits.source_file_bytes > defaults.source_file_bytes
            || limits.path_file_bytes > defaults.path_file_bytes
        {
            return Err(Error::Budget("invalid limits"));
        }
        Ok(Self {
            limits,
            used: [0; 9],
            physical_owners: HashMap::new(),
        })
    }
    /// Reserve retained consumer path/URI bytes before encoding or storing them.
    /// Uses this same aggregate path/work budget; no per-native-path ceiling is
    /// applied to encoded URIs. The caller owns cancellation around encoding.
    pub fn reserve_path_bytes(&mut self, bytes: usize) -> Result<(), Error> {
        self.charge(Count::Work, bytes)?;
        self.charge(Count::Paths, bytes)
    }
    fn charge(&mut self, count: Count, amount: usize) -> Result<(), Error> {
        let index = count as usize;
        let next = self.used[index]
            .checked_add(amount)
            .ok_or(Error::Budget(count.name()))?;
        // Saturate a refused resource at its ceiling: a failed attempt cannot refund work.
        self.used[index] = next.min(self.limits.counters()[index]);
        if next > self.limits.counters()[index] {
            return Err(Error::Budget(count.name()));
        }
        Ok(())
    }
    fn work(&mut self, amount: usize, cancelled: &AtomicBool) -> Result<(), Error> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        self.charge(Count::Work, amount)
    }
    fn physical(&mut self, id: Identity, owner: usize) -> Result<(), Error> {
        if self
            .physical_owners
            .get(&id)
            .is_some_and(|prior| *prior != owner)
        {
            return Err(Error::UnsafePath(
                "physical entry shared by configured projects",
            ));
        }
        self.physical_owners.insert(id, owner);
        Ok(())
    }
    fn path(&mut self, path: &Path, retain: bool, cancelled: &AtomicBool) -> Result<(), Error> {
        let text = path.to_str().ok_or(Error::UnsafePath("non-UTF8 path"))?;
        self.work(text.len(), cancelled)?;
        if text.len() > self.limits.path_file_bytes {
            return Err(Error::Budget("per-path bytes"));
        }
        if !path.is_absolute()
            || text.contains('\0')
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(Error::UnsafePath("requires an absolute normalized path"));
        }
        #[cfg(windows)]
        {
            use std::path::Prefix;
            if !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
                || path.components().any(|c| matches!(c, Component::Normal(s) if s.to_str().is_none_or(|s| s.contains(':') || s.ends_with(['.', ' '])))) {
                return Err(Error::UnsafePath("unsupported Windows path"));
            }
        }
        if retain {
            self.charge(Count::Paths, text.len())?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Budget(&'static str),
    Io(io::Error),
    Import(v2::Error),
    UnsafePath(&'static str),
    Changed,
    InvalidSource(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("source inventory cancelled"),
            Self::Budget(name) => write!(f, "source inventory budget exceeded: {name}"),
            Self::Io(e) => write!(f, "source inventory I/O: {e}"),
            Self::Import(e) => e.fmt(f),
            Self::UnsafePath(reason) => write!(f, "unsafe source inventory path: {reason}"),
            Self::Changed => f.write_str("source inventory changed; reload required"),
            Self::InvalidSource(reason) => write!(f, "invalid inventory source: {reason}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Import(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    KotlinSource,
    KotlinScript,
    Java,
    Other,
    Directory,
}
/// Original ordered indices into `Inventory::imported`, never a flattened root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Membership {
    module_index: usize,
    content_root_index: usize,
    source_root_index: usize,
}
impl Membership {
    pub fn module_index(&self) -> usize {
        self.module_index
    }
    pub fn content_root_index(&self) -> usize {
        self.content_root_index
    }
    pub fn source_root_index(&self) -> usize {
        self.source_root_index
    }
}
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Identity(u64, u64);
#[cfg(windows)]
type Identity = file_id::FileId;
#[cfg(not(any(unix, windows)))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Identity;

fn identity(path: &Path, metadata: &Metadata) -> Result<Identity, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = path;
        Ok(Identity(metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        Ok(file_id::get_high_res_file_id(path)?)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, metadata);
        Err(Error::UnsafePath("native physical identity unavailable"))
    }
}
fn safe(metadata: &Metadata) -> Result<(), Error> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(Error::UnsafePath("reparse point"));
        }
    }
    if metadata.file_type().is_symlink() || !(metadata.is_file() || metadata.is_dir()) {
        return Err(Error::UnsafePath("symlink or special entry"));
    }
    Ok(())
}
#[derive(Debug)]
pub struct Entry {
    path: PathBuf,
    kind: EntryKind,
    source: Option<Arc<str>>,
    memberships: Vec<Membership>,
    identity: Identity,
}
impl Entry {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn kind(&self) -> EntryKind {
        self.kind
    }
    pub fn source(&self) -> Option<&Arc<str>> {
        self.source.as_ref()
    }
    pub fn memberships(&self) -> &[Membership] {
        &self.memberships
    }
}
/// Immutable complete original model, model bytes and admitted native entries.
/// ```compile_fail
/// fn mutate(mut inventory: ktlsp_import::source_inventory::Inventory) {
///     inventory.entries.clear();
/// }
/// ```
#[derive(Debug)]
pub struct Inventory {
    project_directory: PathBuf,
    imported: v2::ImportedWorkspace,
    workspace_json: Vec<u8>,
    model_path: PathBuf,
    model_identity: Identity,
    entries: Vec<Entry>,
    ancestors: BTreeMap<PathBuf, Identity>,
}
impl Inventory {
    pub fn project_directory(&self) -> &Path {
        &self.project_directory
    }
    pub fn imported(&self) -> &v2::ImportedWorkspace {
        &self.imported
    }
    pub fn workspace_json(&self) -> &[u8] {
        &self.workspace_json
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// Revalidate the original model AND selected Kotlin source: original
    /// ancestor/native IDs and exact bytes, streaming through one stack buffer.
    /// Unknown/non-source indices perform no filesystem I/O. Model comparisons
    /// charge actual work, not retained-model storage a second time.
    pub fn verify_source(
        &self,
        entry_index: usize,
        budget: &mut Budget,
        cancelled: &AtomicBool,
    ) -> Result<(), Error> {
        budget.work(1, cancelled)?;
        let entry = self
            .entries
            .get(entry_index)
            .ok_or(Error::InvalidSource("unknown entry"))?;
        let expected = entry
            .source
            .as_ref()
            .ok_or(Error::InvalidSource("not an original Kotlin source"))?;
        if expected.len() > budget.limits.source_file_bytes {
            return Err(Error::Budget("per-source bytes"));
        }
        budget.charge(Count::Sources, 1)?;
        self.verify_file(
            &self.model_path,
            self.model_identity,
            &self.workspace_json,
            budget,
            cancelled,
        )?;
        budget.charge(Count::SourceBytes, expected.len())?;
        self.verify_file(
            &entry.path,
            entry.identity,
            expected.as_bytes(),
            budget,
            cancelled,
        )
    }
    fn verify_file(
        &self,
        path: &Path,
        id: Identity,
        expected: &[u8],
        budget: &mut Budget,
        cancelled: &AtomicBool,
    ) -> Result<(), Error> {
        let check_ancestor = |path: &Path, id, _: &mut Budget| {
            if self.ancestors.get(path) != Some(&id) {
                return Err(Error::Changed);
            }
            Ok(())
        };
        let metadata = inspect(path, budget, cancelled, check_ancestor)?;
        if !metadata.is_file()
            || identity(path, &metadata)? != id
            || metadata.len() != expected.len() as u64
        {
            return Err(Error::Changed);
        }
        budget.work(1, cancelled)?;
        let mut file = File::open(path)?;
        same(path, &metadata, id, &file.metadata()?)?;
        let mut buffer = [0; 8192];
        for chunk in expected.chunks(buffer.len()) {
            budget.work(
                chunk
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Error::Budget("work"))?,
                cancelled,
            )?;
            file.read_exact(&mut buffer[..chunk.len()])?;
            if &buffer[..chunk.len()] != chunk {
                return Err(Error::Changed);
            }
        }
        budget.work(1, cancelled)?;
        if file.read(&mut buffer[..1])? != 0 {
            return Err(Error::Changed);
        }
        same(path, &metadata, id, &file.metadata()?)?;
        let after = inspect(path, budget, cancelled, check_ancestor)?;
        same(path, &metadata, id, &after)
    }
}

/// Check every ancestor without following symlinks; callbacks retain/check IDs.
fn inspect(
    mut_path: &Path,
    budget: &mut Budget,
    cancelled: &AtomicBool,
    mut ancestor: impl FnMut(&Path, Identity, &mut Budget) -> Result<(), Error>,
) -> Result<Metadata, Error> {
    budget.path(mut_path, false, cancelled)?;
    let mut path = PathBuf::new();
    let mut components = mut_path.components().peekable();
    while let Some(component) = components.next() {
        path.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        budget.work(
            path.as_os_str().as_encoded_bytes().len().saturating_add(1),
            cancelled,
        )?;
        let metadata = fs::symlink_metadata(&path)?;
        safe(&metadata)?;
        let id = identity(&path, &metadata)?;
        if components.peek().is_none() {
            return Ok(metadata);
        }
        if !metadata.is_dir() {
            return Err(Error::UnsafePath("non-directory ancestor"));
        }
        ancestor(&path, id, budget)?;
    }
    Err(Error::UnsafePath("missing path components"))
}
fn same(path: &Path, before: &Metadata, id: Identity, after: &Metadata) -> Result<(), Error> {
    safe(after)?;
    if before.is_file() != after.is_file()
        || before.len() != after.len()
        || before.modified()? != after.modified()?
        || identity(path, after)? != id
    {
        return Err(Error::Changed);
    }
    Ok(())
}
fn read_file(
    path: &Path,
    before: &Metadata,
    id: Identity,
    source: bool,
    budget: &mut Budget,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, Error> {
    let length = usize::try_from(before.len()).map_err(|_| Error::Budget("file bytes"))?;
    let max = if source {
        budget.limits.source_file_bytes
    } else {
        budget.limits.model_bytes
    };
    if length > max {
        return Err(Error::Budget(if source {
            "per-source bytes"
        } else {
            "model bytes"
        }));
    }
    budget.charge(
        if source {
            Count::SourceBytes
        } else {
            Count::Model
        },
        length,
    )?;
    budget.work(length.saturating_add(1), cancelled)?;
    let mut file = File::open(path)?;
    same(path, before, id, &file.metadata()?)?;
    let mut bytes = vec![0; length];
    for chunk in bytes.chunks_mut(8192) {
        budget.work(1, cancelled)?;
        file.read_exact(chunk)?;
    }
    let mut extra = [0];
    budget.work(1, cancelled)?;
    if file.read(&mut extra)? != 0 {
        return Err(Error::Changed);
    }
    same(path, before, id, &file.metadata()?)?;
    let current = inspect(path, budget, cancelled, |_, _, _| Ok(()))?;
    same(path, before, id, &current)?;
    Ok(bytes)
}

// Accounting only: reuse the existing bounded JSON decoder, never its schema
// validation/projection. Reserve the pre-normalization expansion envelope BEFORE
// unchanged v2 allocates expanded paths. Normalization cannot enlarge this bound.
fn admit_model_paths(
    project: &Path,
    bytes: &[u8],
    budget: &mut Budget,
    cancelled: &AtomicBool,
) -> Result<(), Error> {
    use serde_json::Value;
    let value = crate::json::parse(bytes).map_err(|e| Error::Import(v2::Error::Import(e)))?;
    fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
        value
            .get(key)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }
    let mut admit = |value: &Value, copies: usize, expand: bool| -> Result<(), Error> {
        let Some(text) = value.as_str() else {
            return Ok(());
        };
        budget.work(text.len().saturating_add(1), cancelled)?;
        let length = if expand {
            if let Some(rest) = text.strip_prefix("<WORKSPACE>/") {
                project
                    .as_os_str()
                    .as_encoded_bytes()
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_add(rest.len()))
                    .ok_or(Error::Budget("path bytes"))?
            } else {
                text.len()
            }
        } else {
            text.len()
        };
        if length > budget.limits.path_file_bytes {
            return Err(Error::Budget("per-path bytes"));
        }
        budget.charge(
            Count::Paths,
            length
                .checked_mul(copies)
                .ok_or(Error::Budget("path bytes"))?,
        )
    };
    for module in array(&value, "modules") {
        for content in array(module, "contentRoots") {
            admit(&content["path"], 1, true)?;
            for source in array(content, "sourceRoots") {
                admit(&source["path"], 2, true)?;
            }
        }
    }
    for library in array(&value, "libraries") {
        for root in array(library, "roots") {
            admit(&root["path"], 2, true)?;
        }
    }
    for sdk in array(&value, "sdks") {
        admit(&sdk["homePath"], 1, false)?;
        for root in array(sdk, "roots") {
            admit(&root["url"], 1, false)?;
        }
    }
    Ok(())
}
struct Root {
    path: PathBuf,
    membership: Membership,
    test: bool,
}
struct Builder<'a> {
    budget: &'a mut Budget,
    cancelled: &'a AtomicBool,
    model: Option<(Identity, PathBuf)>,
    owner: usize,
    ancestors: BTreeMap<PathBuf, Identity>,
    identities: HashSet<Identity>,
    entries: Vec<Entry>,
    roots: Vec<Root>,
}
impl Builder<'_> {
    fn inspect(&mut self, path: &Path) -> Result<Metadata, Error> {
        let ancestors = &mut self.ancestors;
        let cancelled = self.cancelled;
        // Store each ancestor once; no descriptor per entry is retained.
        inspect(path, self.budget, cancelled, |path, id, budget| {
            if let Some(original) = ancestors.get(path) {
                if *original != id {
                    return Err(Error::Changed);
                }
            } else {
                budget.path(path, true, cancelled)?;
                ancestors.insert(path.to_path_buf(), id);
            }
            Ok(())
        })
    }
    fn walk(&mut self, path: PathBuf, depth: usize) -> Result<(), Error> {
        self.budget.work(1, self.cancelled)?;
        if depth > self.budget.limits.depth {
            return Err(Error::Budget("depth"));
        }
        self.budget.charge(Count::Entries, 1)?;
        self.budget.path(&path, true, self.cancelled)?;
        let metadata = self.inspect(&path)?;
        let id = identity(&path, &metadata)?;
        if !self.identities.insert(id) {
            return Err(Error::UnsafePath("physical alias"));
        }
        if self
            .model
            .as_ref()
            .is_some_and(|(model_id, model_path)| *model_id == id && *model_path != path)
        {
            return Err(Error::UnsafePath("model physical alias"));
        }
        self.budget.physical(id, self.owner)?;
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else {
            match path.extension().and_then(|s| s.to_str()) {
                Some("kt") => EntryKind::KotlinSource,
                Some("kts") => EntryKind::KotlinScript,
                Some("java") => EntryKind::Java,
                _ => EntryKind::Other,
            }
        };
        let mut memberships = Vec::new();
        for root in &self.roots {
            self.budget.work(
                path.as_os_str()
                    .as_encoded_bytes()
                    .len()
                    .saturating_add(root.path.as_os_str().as_encoded_bytes().len()),
                self.cancelled,
            )?;
            if path.starts_with(&root.path) {
                self.budget.charge(Count::Memberships, 1)?;
                memberships.push(root.membership);
            }
        }
        let source = if kind == EntryKind::KotlinSource {
            self.budget.charge(Count::Sources, 1)?;
            let bytes = read_file(&path, &metadata, id, true, self.budget, self.cancelled)?;
            self.budget.work(bytes.len(), self.cancelled)?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| Error::InvalidSource("non-UTF8 Kotlin source"))?;
            Some(Arc::<str>::from(text))
        } else {
            None
        };
        if kind == EntryKind::Directory {
            self.budget.charge(Count::Directories, 1)?;
        }
        self.entries.push(Entry {
            path: path.clone(),
            kind,
            source,
            memberships,
            identity: id,
        });
        if kind == EntryKind::Directory {
            self.budget.work(1, self.cancelled)?;
            for child in fs::read_dir(&path)? {
                self.budget.work(1, self.cancelled)?;
                let child = child?;
                self.walk(
                    child.path(),
                    depth.checked_add(1).ok_or(Error::Budget("depth"))?,
                )?;
            }
            let after = self.inspect(&path)?;
            same(&path, &metadata, id, &after)?;
        }
        Ok(())
    }
}

/// Atomically load actual bounded `workspace.json` and every complete source root.
/// No hidden/build/.git exclusions, fallback import, user writes or partial success.
/// Use the SAME budget for every project in a candidate. Roots must be directories
/// inside the canonical project; physical aliases and overlapping owner/test kinds refuse.
pub fn load(
    project_directory: impl AsRef<Path>,
    budget: &mut Budget,
    cancelled: &AtomicBool,
) -> Result<Inventory, Error> {
    budget.work(1, cancelled)?;
    budget.charge(Count::Projects, 1)?;
    let owner = budget.used[Count::Projects as usize];
    let input = project_directory.as_ref();
    budget.path(input, false, cancelled)?;
    let mut builder = Builder {
        budget,
        cancelled,
        owner,
        model: None,
        ancestors: BTreeMap::new(),
        identities: HashSet::new(),
        entries: Vec::new(),
        roots: Vec::new(),
    };
    if !builder.inspect(input)?.is_dir() {
        return Err(Error::UnsafePath("project is not a directory"));
    }
    builder.budget.work(1, cancelled)?;
    let project = fs::canonicalize(input)?;
    builder.budget.path(&project, true, cancelled)?;
    let project_metadata = builder.inspect(&project)?;
    let project_id = identity(&project, &project_metadata)?;
    if builder.budget.physical_owners.contains_key(&project_id) {
        return Err(Error::UnsafePath("duplicate or shared configured project"));
    }
    builder.budget.physical(project_id, owner)?;
    builder.budget.path(&project, true, cancelled)?;
    builder.ancestors.insert(project.clone(), project_id);
    let model_path = project.join("workspace.json");
    builder.budget.path(&model_path, true, cancelled)?;
    let model_metadata = builder.inspect(&model_path)?;
    if !model_metadata.is_file() {
        return Err(Error::UnsafePath("workspace.json is not a file"));
    }
    let model_id = identity(&model_path, &model_metadata)?;
    builder.budget.physical(model_id, owner)?;
    let bytes = read_file(
        &model_path,
        &model_metadata,
        model_id,
        false,
        builder.budget,
        cancelled,
    )?;
    // Each WORKSPACE macro consumes at least 12 input bytes (escaping cannot
    // shorten it). Bound caller-base amplification before unchanged v2 allocates.
    // Its own expanded-path cap and depth/collection bounds constrain the
    // normalization, projection, sorted ownership and validation passes.
    let expansion = (bytes.len() / 12)
        .checked_mul(project.as_os_str().as_encoded_bytes().len())
        .and_then(|n| n.checked_add(bytes.len()))
        .ok_or(Error::Budget("work"))?
        .min(crate::MAX_EXPANDED_PATH_BYTES);
    let delegated = bytes
        .len()
        .checked_add(expansion)
        .and_then(|n| n.checked_mul(32))
        .ok_or(Error::Budget("work"))?;
    builder.budget.work(delegated, cancelled)?;
    admit_model_paths(&project, &bytes, builder.budget, cancelled)?;
    let imported = v2::from_json(&project, &bytes).map_err(Error::Import)?;
    builder.model = Some((model_id, model_path));
    // Preflight reserved all original/projection path storage before import.
    // Keep physical-path syntax admission separate from data-only SDK URL/home.
    for module in imported.modules() {
        for content in &module.content_roots {
            builder.budget.path(&content.path, false, cancelled)?;
            for source in &content.source_roots {
                builder.budget.path(&source.path, false, cancelled)?;
            }
        }
    }
    for library in imported.libraries() {
        for root in &library.roots {
            builder.budget.path(&root.path, false, cancelled)?;
        }
    }
    for (module_index, module) in imported.modules().iter().enumerate() {
        for (content_root_index, content) in module.content_roots.iter().enumerate() {
            for (source_root_index, source) in content.source_roots.iter().enumerate() {
                builder.budget.path(&source.path, false, cancelled)?;
                let physical = native_path(&source.path);
                builder.budget.path(&physical, true, cancelled)?;
                let relative = physical
                    .strip_prefix(&project)
                    .map_err(|_| Error::UnsafePath("external source root"))?;
                if relative.components().count() > builder.budget.limits.depth {
                    return Err(Error::Budget("depth"));
                }
                if !builder.inspect(&physical)?.is_dir() {
                    return Err(Error::UnsafePath("source root is not a directory"));
                }
                builder
                    .budget
                    .work(physical.as_os_str().as_encoded_bytes().len(), cancelled)?;
                if fs::canonicalize(&physical)? != physical {
                    return Err(Error::UnsafePath("noncanonical source root alias"));
                }
                let test = source.root_type.ends_with("-test");
                for prior in &builder.roots {
                    builder.budget.work(
                        physical
                            .as_os_str()
                            .as_encoded_bytes()
                            .len()
                            .saturating_add(prior.path.as_os_str().as_encoded_bytes().len()),
                        cancelled,
                    )?;
                    if (physical.starts_with(&prior.path) || prior.path.starts_with(&physical))
                        && (prior.membership.module_index != module_index || prior.test != test)
                    {
                        return Err(Error::UnsafePath(
                            "conflicting source root ownership or kind",
                        ));
                    }
                }
                builder.roots.push(Root {
                    path: physical,
                    test,
                    membership: Membership {
                        module_index,
                        content_root_index,
                        source_root_index,
                    },
                });
            }
        }
    }
    // Walk each topmost physical root once, preserving original root order in memberships.
    for index in 0..builder.roots.len() {
        let mut nested = false;
        for other in 0..builder.roots.len() {
            builder.budget.work(
                builder.roots[index]
                    .path
                    .as_os_str()
                    .as_encoded_bytes()
                    .len()
                    .saturating_add(
                        builder.roots[other]
                            .path
                            .as_os_str()
                            .as_encoded_bytes()
                            .len(),
                    ),
                cancelled,
            )?;
            if other != index
                && builder.roots[index]
                    .path
                    .starts_with(&builder.roots[other].path)
                && (builder.roots[index].path != builder.roots[other].path || other < index)
            {
                nested = true;
                break;
            }
        }
        if !nested {
            let root = builder.roots[index].path.clone();
            let depth = root
                .strip_prefix(&project)
                .map_err(|_| Error::UnsafePath("external root"))?
                .components()
                .count();
            builder.walk(root, depth)?;
        }
    }
    builder.budget.work(1, cancelled)?;
    let (model_identity, model_path) = builder
        .model
        .ok_or(Error::UnsafePath("missing admitted model"))?;
    Ok(Inventory {
        project_directory: project,
        imported,
        workspace_json: bytes,
        model_path,
        model_identity,
        entries: builder.entries,
        ancestors: builder.ancestors,
    })
}
