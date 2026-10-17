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

    pub(super) fn verify(
        &self,
        key: SourceKey,
        budget: &mut Budget,
        cancelled: &AtomicBool,
    ) -> Result<(), source_inventory::Error> {
        self.projects[key.project].verify_source(key.entry, budget, cancelled)
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
}
