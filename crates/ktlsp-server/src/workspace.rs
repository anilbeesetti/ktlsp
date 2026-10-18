//! Explicit JSON source snapshots, not a live index or an effective classpath.
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use ktlsp_import::source_inventory::{self, Budget, EntryKind, Inventory, Limits};
use serde_json::Value;

use crate::{MAX_URI_BYTES, file_uri, object, required, uri_value};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(super) enum Project {
    Json(PathBuf),
    Unsupported,
}

pub(super) fn projects(options: Option<&Value>) -> Result<Vec<Project>, &'static str> {
    let Some(raw) = options
        .and_then(Value::as_object)
        .and_then(|o| o.get("projects"))
    else {
        return Ok(Vec::new());
    };
    let values = raw
        .as_array()
        .ok_or("configured projects must be an array")?;
    if values.len() > Limits::default().projects {
        return Err("configured project count limit exceeded");
    }
    let mut projects = Vec::with_capacity(values.len());
    for value in values {
        let entry = object(value)?;
        let kind = required(entry, "type")?
            .as_str()
            .ok_or("project type must be a string")?;
        let uri = uri_value(required(entry, "path")?)?;
        projects.push(
            if kind == "json" && entry.keys().all(|k| matches!(k.as_str(), "type" | "path")) {
                Project::Json(file_uri::decode(uri)?)
            } else {
                Project::Unsupported
            },
        );
    }
    Ok(projects)
}

pub(super) fn is_file_uri(uri: &str) -> bool {
    uri.split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
}

pub(super) fn canonical_uri(uri: &str) -> Result<String, &'static str> {
    file_uri::encode(&file_uri::decode(uri)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SourceKey {
    project: usize,
    entry: usize,
}

type CapturedSource = (Arc<Snapshot>, SourceKey, Arc<str>, Arc<str>);

#[derive(Debug)]
pub(super) struct Snapshot {
    pub(super) generation: i64,
    projects: Vec<Inventory>,
    files: HashMap<Arc<str>, SourceKey>,
}

/// Closed production ownership over the original coherent capture, not a
/// projected classpath. Construction proves complete candidate coverage.
#[derive(Debug)]
pub(super) struct SourceModule<'a> {
    snapshot: &'a Snapshot,
    capture: &'a crate::workspace_symbols::Capture,
    owner: &'a SourceKey,
    module: &'a ktlsp_import::v2::ImportedModule,
}

impl SourceModule<'_> {
    /// Only actual members of this same capture can participate. Canonical
    /// member URIs are resolved through the original map, never path prefixes.
    pub(super) fn contains(
        &self,
        member: &crate::workspace_symbols::Member,
        budget: &mut crate::workspace_symbols::Budget,
        cancelled: &AtomicBool,
    ) -> Result<bool, &'static str> {
        budget.work(self.capture.members.len(), cancelled)?;
        if !self
            .capture
            .members
            .iter()
            .any(|original| std::ptr::eq(original, member))
        {
            return Err("foreign captured source member");
        }
        let key = self.snapshot.member_key(member, budget, cancelled)?;
        if key.project != self.owner.project {
            return Ok(false);
        }
        let entry = &self.snapshot.projects[key.project].entries()[key.entry];
        budget.work(entry.memberships().len(), cancelled)?;
        Ok(entry.memberships().iter().any(|membership| {
            std::ptr::eq(
                &self.snapshot.projects[key.project].imported().modules()
                    [membership.module_index()],
                self.module,
            )
        }))
    }
}

impl Snapshot {
    pub(super) fn load(
        projects: Vec<Project>,
        generation: i64,
        cancelled: &AtomicBool,
    ) -> Result<Self, String> {
        if projects.iter().any(|p| matches!(p, Project::Unsupported)) {
            return Err("only explicit JSON projects with type/path fields are supported".into());
        }
        let mut budget = Budget::default();
        let mut inventories = Vec::with_capacity(projects.len());
        let mut files = HashMap::new();
        for project in projects {
            if cancelled.load(Ordering::Relaxed) {
                return Err("workspace import cancelled".into());
            }
            let Project::Json(directory) = project else {
                unreachable!()
            };
            let inventory = source_inventory::load(directory, &mut budget, cancelled)
                .map_err(|e| e.to_string())?;
            for (entry, source) in inventory.entries().iter().enumerate() {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("workspace import cancelled".into());
                }
                if source.kind() != EntryKind::KotlinSource {
                    continue;
                }
                let path = source.path().to_str().ok_or("source path is not UTF-8")?;
                // ponytail: reserve the URI's bounded expansion before allocation;
                // exact reservation can replace this if path-budget refusals matter.
                let bytes = path
                    .len()
                    .saturating_mul(3)
                    .saturating_add(11)
                    .min(MAX_URI_BYTES);
                budget
                    .reserve_path_bytes(bytes)
                    .map_err(|e| e.to_string())?;
                let uri: Arc<str> = file_uri::encode(source.path())
                    .map_err(str::to_owned)?
                    .into();
                if source.source().is_none() {
                    return Err("Kotlin inventory entry has no authoritative source".into());
                }
                if files
                    .insert(
                        uri,
                        SourceKey {
                            project: inventories.len(),
                            entry,
                        },
                    )
                    .is_some()
                {
                    return Err("competing imported source URI identities".into());
                }
            }
            inventories.push(inventory);
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err("workspace import cancelled".into());
        }
        Ok(Self {
            generation,
            projects: inventories,
            files,
        })
    }

    fn lookup(&self, uri: &str) -> Option<(&Arc<str>, SourceKey)> {
        if let Some((key, source)) = self.files.get_key_value(uri) {
            return Some((key, *source));
        }
        let canonical = canonical_uri(uri).ok()?;
        self.files
            .get_key_value(canonical.as_str())
            .map(|(key, source)| (key, *source))
    }

    pub(super) fn capture(&self, uri: &str) -> Option<(SourceKey, Arc<str>, Arc<str>)> {
        let (canonical, key) = self.lookup(uri)?;
        let source = self
            .projects
            .get(key.project)?
            .entries()
            .get(key.entry)?
            .source()?;
        Some((key, Arc::clone(canonical), Arc::clone(source)))
    }

    fn member_key(
        &self,
        member: &crate::workspace_symbols::Member,
        budget: &mut crate::workspace_symbols::Budget,
        cancelled: &AtomicBool,
    ) -> Result<&SourceKey, &'static str> {
        budget.work(member.source.document().len(), cancelled)?;
        self.files
            .get(member.source.document())
            .ok_or("captured source is not an original canonical member")
    }

    /// Allocation-free original-model bridge for the finite imported-value
    /// consumer. Existing verify_all/fresh_all still gate execution/publication.
    pub(super) fn source_module<'a>(
        &'a self,
        capture: &'a crate::workspace_symbols::Capture,
        requester: &'a crate::workspace_symbols::Member,
        budget: &mut crate::workspace_symbols::Budget,
        cancelled: &AtomicBool,
    ) -> Result<SourceModule<'a>, &'static str> {
        budget.work(1, cancelled)?;
        if !capture
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| std::ptr::eq(self, snapshot.as_ref()))
            || capture.members.len() != self.files.len()
            || capture.closed.len() != self.projects.len()
        {
            return Err("foreign or incomplete workspace source capture");
        }
        // Strict original key order proves every canonical member exactly once,
        // including negative/nonmatching files; aliases cannot replace members.
        let mut previous = None;
        let mut found = false;
        for member in &capture.members {
            budget.work(1, cancelled)?;
            let key = self.member_key(member, budget, cancelled)?;
            if previous
                .is_some_and(|old: &SourceKey| (old.project, old.entry) >= (key.project, key.entry))
            {
                return Err("competing or unordered captured source members");
            }
            previous = Some(key);
            found |= std::ptr::eq(member, requester);
        }
        if !found {
            return Err("foreign requesting source member");
        }
        let owner = self.member_key(requester, budget, cancelled)?;
        let inventory = &self.projects[owner.project];
        let entry = &inventory.entries()[owner.entry];
        let membership = entry
            .memberships()
            .first()
            .ok_or("requesting source has no original module")?;
        let module_index = membership.module_index();
        let module = inventory
            .imported()
            .modules()
            .get(module_index)
            .ok_or("original source module unavailable")?;
        budget.work(module.dependencies.len(), cancelled)?;
        if module.dependencies.is_empty()
            || module
                .dependencies
                .iter()
                .any(|dependency| !matches!(dependency, ktlsp_import::v2::Dependency::ModuleSource))
        {
            return Err("source module has unsupported dependency competition");
        }
        for content in &module.content_roots {
            budget.work(1, cancelled)?;
            for root in &content.source_roots {
                budget.work(1, cancelled)?;
                if !matches!(root.root_type.as_str(), "kotlin-source" | "java-source") {
                    return Err("source module is not closed production source");
                }
            }
        }
        // ponytail: O(n) original inventory/capture scans per request; contains
        // additionally authenticates its borrowed member in O(n). Cache only
        // original producer authority if this bounded query cost matters.
        for candidate in inventory.entries() {
            budget.work(1, cancelled)?;
            let mut relevant = false;
            for membership in candidate.memberships() {
                budget.work(1, cancelled)?;
                relevant |= membership.module_index() == module_index;
            }
            if !relevant {
                continue;
            }
            for membership in candidate.memberships() {
                budget.work(1, cancelled)?;
                if membership.module_index() != module_index {
                    return Err("competing original source module ownership");
                }
                let root = module
                    .content_roots
                    .get(membership.content_root_index())
                    .and_then(|content| content.source_roots.get(membership.source_root_index()))
                    .ok_or("original source membership unavailable")?;
                if !matches!(root.root_type.as_str(), "kotlin-source" | "java-source") {
                    return Err("mixed production and test source ownership");
                }
            }
            if matches!(candidate.kind(), EntryKind::Java | EntryKind::KotlinScript) {
                return Err("unsupported source module semantic contributor");
            }
        }
        Ok(SourceModule {
            snapshot: self,
            capture,
            owner,
            module,
        })
    }

    pub(super) fn verify(
        &self,
        key: SourceKey,
        budget: &mut Budget,
        cancelled: &AtomicBool,
    ) -> Result<(), source_inventory::Error> {
        self.projects[key.project].verify_source(key.entry, budget, cancelled)
    }

    pub(super) fn verify_all(
        &self,
        closed: &[Vec<usize>],
        budget: &mut source_inventory::SnapshotBudget,
        cancelled: &AtomicBool,
    ) -> Result<(), source_inventory::Error> {
        if closed.len() != self.projects.len() {
            return Err(source_inventory::Error::InvalidSource(
                "incomplete captured project selection",
            ));
        }
        budget.begin_phase()?;
        for (project, entries) in self.projects.iter().zip(closed) {
            project.verify_inventory(entries, budget, cancelled)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Overlay {
    uri: Arc<str>,
    incarnation: u64,
}

#[derive(Debug, Default)]
pub(super) struct Workspace {
    pub(super) initial: Vec<Project>,
    pub(super) snapshot: Option<Arc<Snapshot>>,
    pub(super) available: bool,
    pub(super) epoch: u64,
    pub(super) attempt: u64,
    generation: i64,
    retained: Vec<Weak<Snapshot>>,
    overlays: HashMap<SourceKey, Overlay>,
    unknown: HashSet<u64>,
    exhausted: bool,
}

impl Workspace {
    fn advance(&mut self) -> Result<(), &'static str> {
        let Some(next) = self.epoch.checked_add(1).filter(|_| !self.exhausted) else {
            self.available = false;
            self.exhausted = true;
            return Err("workspace revision limit exceeded");
        };
        self.epoch = next;
        Ok(())
    }

    pub(super) fn invalidate(&mut self) -> Result<(), &'static str> {
        self.available = false;
        self.advance()?;
        let Some(attempt) = self.attempt.checked_add(1) else {
            self.exhausted = true;
            return Err("workspace attempt limit exceeded");
        };
        self.attempt = attempt;
        Ok(())
    }

    /// Invalidate old publication before admission, including failed reloads.
    pub(super) fn begin(&mut self, empty: bool) -> Result<(u64, Option<i64>), &'static str> {
        self.invalidate()?;
        self.retained
            .retain(|snapshot| snapshot.strong_count() != 0);
        if empty {
            self.snapshot = None;
            self.overlays.clear();
            self.available = true;
            self.unknown.clear();
            return Ok((self.attempt, None));
        }
        if self.retained.len() >= 2 {
            return Err("workspace generation capacity exhausted");
        }
        let Some(generation) = self.generation.checked_add(1) else {
            self.exhausted = true;
            return Err("workspace generation limit exceeded");
        };
        self.generation = generation;
        Ok((self.attempt, Some(self.generation)))
    }

    pub(super) fn commit<'a>(
        &mut self,
        snapshot: Snapshot,
        documents: impl Iterator<Item = (&'a str, u64)>,
    ) -> Result<(), &'static str> {
        let mut overlays = HashMap::new();
        for (uri, incarnation) in documents.filter(|(uri, _)| is_file_uri(uri)) {
            let (_, key) = snapshot
                .lookup(uri)
                .ok_or("unresolved file overlays block workspace publication")?;
            if overlays
                .insert(
                    key,
                    Overlay {
                        uri: Arc::from(uri),
                        incarnation,
                    },
                )
                .is_some()
            {
                return Err("competing file overlays block workspace publication");
            }
        }
        self.advance()?;
        let snapshot = Arc::new(snapshot);
        self.retained.retain(|old| old.strong_count() != 0);
        self.retained.push(Arc::downgrade(&snapshot));
        self.snapshot = Some(snapshot);
        self.overlays = overlays;
        self.unknown.clear();
        self.available = true;
        Ok(())
    }

    pub(super) fn check_open(&self, uri: &str) -> Result<(), &'static str> {
        if self.available
            && let Some(snapshot) = &self.snapshot
            && let Some((_, key)) = snapshot.lookup(uri)
            && self.overlays.contains_key(&key)
        {
            return Err("an alias of this imported document is already open");
        }
        Ok(())
    }

    pub(super) fn opened(&mut self, uri: &str, incarnation: u64) {
        if !is_file_uri(uri) || self.snapshot.is_none() {
            return;
        }
        let _ = self.advance();
        if self.available
            && let Some(snapshot) = &self.snapshot
            && let Some((_, key)) = snapshot.lookup(uri)
        {
            self.overlays.insert(
                key,
                Overlay {
                    uri: Arc::from(uri),
                    incarnation,
                },
            );
        } else {
            self.unknown.insert(incarnation);
        }
    }

    pub(super) fn changed(&mut self, uri: &str) {
        if is_file_uri(uri) && self.snapshot.is_some() {
            let _ = self.advance();
        }
    }

    pub(super) fn closed(&mut self, uri: &str, incarnation: u64) {
        if !is_file_uri(uri) || self.snapshot.is_none() {
            return;
        }
        let _ = self.advance();
        self.unknown.remove(&incarnation);
        if let Some(snapshot) = &self.snapshot
            && let Some((_, key)) = snapshot.lookup(uri)
            && self
                .overlays
                .get(&key)
                .is_some_and(|overlay| overlay.incarnation == incarnation)
        {
            self.overlays.remove(&key);
        }
    }

    pub(super) fn overlay_uri(&self, uri: &str) -> Option<&str> {
        if !self.available {
            return None;
        }
        let (_, key) = self.snapshot.as_ref()?.lookup(uri)?;
        self.overlays.get(&key).map(|overlay| overlay.uri.as_ref())
    }

    pub(super) fn capture(&self, uri: &str) -> Result<CapturedSource, &'static str> {
        if !self.available || self.exhausted {
            return Err("workspace source inventory unavailable; reload required");
        }
        // ponytail: unknown file overlays may alias a member; fail closed without
        // reading outside URIs. Scoped nonmembership proof can relax this ceiling.
        if !self.unknown.is_empty() {
            return Err("unresolved file overlays block workspace publication");
        }
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or("workspace source inventory unavailable")?;
        let (key, canonical, source) = snapshot
            .capture(uri)
            .ok_or("document is outside imported Kotlin sources")?;
        Ok((Arc::clone(snapshot), key, canonical, source))
    }

    pub(super) fn fresh(&self, snapshot: &Arc<Snapshot>, epoch: u64) -> bool {
        self.available
            && !self.exhausted
            && self.unknown.is_empty()
            && self.epoch == epoch
            && self
                .snapshot
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, snapshot))
    }

    /// One immutable generation and exact mapped overlay incarnation/source.
    /// Enumeration follows original project/entry order, never HashMap order.
    pub(super) fn capture_all(
        &self,
        documents: &HashMap<String, ktlsp_text::Document>,
        incarnations: &HashMap<String, u64>,
        budget: &mut crate::workspace_symbols::Budget,
        cancelled: &AtomicBool,
    ) -> Result<crate::workspace_symbols::Capture, &'static str> {
        use crate::workspace_symbols::{Capture, Member};
        if !self.available || self.exhausted || !self.unknown.is_empty() {
            return Err("workspace source inventory unavailable; reload required");
        }
        let mut capture = Capture {
            snapshot: self.snapshot.clone(),
            epoch: self.epoch,
            members: Vec::new(),
            closed: Vec::new(),
        };
        let Some(snapshot) = &self.snapshot else {
            return Ok(capture);
        };
        let mut files = Vec::new();
        budget.storage(
            snapshot
                .files
                .len()
                .checked_mul(std::mem::size_of::<(&Arc<str>, &SourceKey)>())
                .ok_or("workspace storage limit exceeded")?,
        )?;
        files
            .try_reserve_exact(snapshot.files.len())
            .map_err(|_| "workspace allocation failed")?;
        files.extend(snapshot.files.iter());
        let sort_work = files
            .len()
            .checked_mul(usize::BITS as usize - files.len().leading_zeros() as usize)
            .ok_or("workspace work limit exceeded")?;
        budget.work(sort_work, cancelled)?;
        files.sort_unstable_by_key(|(_, key)| (key.project, key.entry));
        budget.storage(
            snapshot
                .projects
                .len()
                .checked_mul(std::mem::size_of::<Vec<usize>>())
                .ok_or("workspace storage limit exceeded")?,
        )?;
        capture
            .closed
            .try_reserve_exact(snapshot.projects.len())
            .map_err(|_| "workspace allocation failed")?;
        capture
            .closed
            .resize_with(snapshot.projects.len(), Vec::new);
        for (uri, key) in files {
            budget.work(1, cancelled)?;
            let original = snapshot.projects[key.project].entries()[key.entry]
                .source()
                .ok_or("member source unavailable")?;
            let (source, revision, overlay) = if let Some(overlay) = self.overlays.get(key) {
                let document = documents
                    .get(overlay.uri.as_ref())
                    .ok_or("member overlay unavailable")?;
                if incarnations.get(overlay.uri.as_ref()) != Some(&overlay.incarnation) {
                    return Err("member overlay incarnation mismatch");
                }
                budget.source(document.text().len())?;
                // Precharge both the retained copy and publication's exact-byte check.
                budget.work(
                    document
                        .text()
                        .len()
                        .checked_mul(2)
                        .ok_or("workspace work limit exceeded")?,
                    cancelled,
                )?;
                // This is the one retained source copy for an authoritative open member.
                (
                    Arc::<str>::from(document.text()),
                    i64::from(document.version()),
                    Some((Arc::clone(&overlay.uri), overlay.incarnation)),
                )
            } else {
                budget.source(original.len())?;
                budget.storage(std::mem::size_of::<usize>())?;
                capture.closed[key.project]
                    .try_reserve(1)
                    .map_err(|_| "workspace allocation failed")?;
                capture.closed[key.project].push(key.entry);
                (Arc::clone(original), snapshot.generation, None)
            };
            budget.storage(std::mem::size_of::<Member>())?;
            capture
                .members
                .try_reserve(1)
                .map_err(|_| "workspace allocation failed")?;
            let source = ktlsp_parser::SourceSnapshot::new(
                Arc::clone(uri),
                revision,
                ktlsp_parser::FileKind::Source,
                source,
            )
            .map_err(|_| "workspace source exceeds parser limits")?;
            capture.members.push(Member { source, overlay });
        }
        Ok(capture)
    }

    pub(super) fn fresh_all(
        &self,
        capture: &crate::workspace_symbols::Capture,
        documents: &HashMap<String, ktlsp_text::Document>,
        incarnations: &HashMap<String, u64>,
    ) -> bool {
        let authority = match &capture.snapshot {
            Some(snapshot) => self.fresh(snapshot, capture.epoch),
            None => {
                self.available
                    && !self.exhausted
                    && self.snapshot.is_none()
                    && self.epoch == capture.epoch
            }
        };
        authority
            && capture.members.iter().all(|member| {
                member.overlay.as_ref().is_none_or(|(uri, incarnation)| {
                    incarnations.get(uri.as_ref()) == Some(incarnation)
                        && documents.get(uri.as_ref()).is_some_and(|document| {
                            i64::from(document.version()) == member.source.revision()
                                && document.text() == member.source.source()
                        })
                })
            })
    }
}
