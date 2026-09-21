/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! The three resolvers: an in-memory map, the filesystem, and package
//! folders.
//!
//! Each one is the sandbox for the references it serves. A resolver only
//! ever reports what it can see, so a caller chooses what a document can
//! reach by choosing the resolver and the filesystem behind it.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::vfs::{within_root, OsFs, SharedFs, SourceFs};
use crate::{
    build_potentials, ext_kind, resolve_path_spec, source_dir, MultiSourceOptions, Resolution,
    Resolver, ResolverInput, NONE,
};

/// How many parent folders a reference is chased up through when looking
/// for a linked package, matching the canonical TypeScript file
/// resolver's heuristic.
const NODE_MODULES_LEVELS: usize = 7;

/// The base a reference resolves against: the directory of the source
/// that contains it, else the configured base path.
fn base_for(
    input: &ResolverInput<'_>,
    is_file: &dyn Fn(&str) -> bool,
    dir: &dyn Fn(&str) -> String,
) -> String {
    let raw = match input.source_path() {
        Some(path) => path,
        None => input.options.path.clone(),
    };
    if raw.is_empty() {
        return String::new();
    }
    if is_file(&raw) {
        dir(&raw)
    } else {
        raw
    }
}

// ---------------------------------------------------------------------
// The in-memory resolver
// ---------------------------------------------------------------------

/// Resolve references against a map of path to content.
///
/// Keys are used exactly as given, so a map is a closed world: nothing
/// outside it can be reached, whatever a document asks for. This is the
/// resolver the shared fixtures run against.
#[derive(Clone, Debug, Default)]
pub struct MapResolver {
    files: Arc<BTreeMap<String, String>>,
}

impl MapResolver {
    /// A resolver over no sources.
    pub fn new() -> Self {
        Self::default()
    }

    /// A resolver over `files`.
    pub fn from_map(files: BTreeMap<String, String>) -> Self {
        MapResolver {
            files: Arc::new(files),
        }
    }

    /// Every path this resolver can serve, in sorted order.
    pub fn paths(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }
}

impl<K, V> FromIterator<(K, V)> for MapResolver
where
    K: Into<String>,
    V: Into<String>,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(entries: T) -> Self {
        MapResolver::from_map(
            entries
                .into_iter()
                .map(|(path, content)| (path.into(), content.into()))
                .collect(),
        )
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for MapResolver
where
    K: Into<String>,
    V: Into<String>,
{
    fn from(entries: [(K, V); N]) -> Self {
        entries.into_iter().collect()
    }
}

impl Resolver for MapResolver {
    fn resolve(&self, reference: Option<&str>, input: &ResolverInput<'_>) -> Resolution {
        // Only a path that is itself a key names a source, and only that
        // one is reduced to its containing folder, so a relative
        // reference resolves against the source that contains it. A bare
        // key such as `a.jsonic` has no folder, which is why an empty
        // base has to stay empty rather than become `.`.
        let files = Arc::clone(&self.files);
        let is_file = move |path: &str| files.contains_key(path);
        let base = base_for(input, &is_file, &|path| source_dir(path));

        let spec = resolve_path_spec(reference, &base);
        let Some(full) = spec.full.clone() else {
            return Resolution::not_found(spec);
        };

        let potentials = if spec.kind == NONE {
            build_potentials(&full, &input.options.implicit_ext)
        } else {
            vec![full]
        };

        for candidate in &potentials {
            if let Some(src) = self.files.get(candidate) {
                return Resolution::found(spec, candidate, src.clone())
                    .with_search(potentials.clone());
            }
        }

        Resolution::not_found(spec).with_search(potentials)
    }
}

// ---------------------------------------------------------------------
// The file resolver
// ---------------------------------------------------------------------

/// Transform a raw reference before it is resolved.
pub type PathFinder = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Resolve references against a filesystem.
///
/// By default that is the real filesystem, with references resolved to
/// absolute paths. Give it a [`MapFs`] and it touches no disk at all;
/// give it a root and no reference can name anything outside that
/// directory, however many `..` segments it carries.
#[derive(Clone, Default)]
pub struct FileResolver {
    pathfinder: Option<PathFinder>,
    preload: Arc<BTreeMap<String, String>>,
    fs: Option<SharedFs>,
    root: Option<String>,
}

impl std::fmt::Debug for FileResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileResolver")
            .field(
                "pathfinder",
                &self.pathfinder.as_ref().map(|_| "<function>"),
            )
            .field("preload", &self.preload.len())
            .field("fs", &self.fs.as_ref().map(|_| "<SourceFs>"))
            .field("root", &self.root)
            .finish()
    }
}

impl FileResolver {
    /// A resolver over the real filesystem.
    pub fn new() -> Self {
        Self::default()
    }

    /// Transform each raw reference before resolving it.
    pub fn with_pathfinder(
        mut self,
        pathfinder: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> Self {
        self.pathfinder = Some(Arc::new(pathfinder));
        self
    }

    /// Serve these paths from memory before touching the filesystem.
    /// The map [`crate::preload_files`] returns goes here.
    pub fn with_preload(mut self, preload: BTreeMap<String, String>) -> Self {
        self.preload = Arc::new(preload);
        self
    }

    /// Read from `filesystem` instead of the one the plugin options name.
    pub fn with_fs(mut self, filesystem: impl SourceFs + 'static) -> Self {
        self.fs = Some(Arc::new(filesystem));
        self
    }

    /// Read from an already shared filesystem handle.
    pub fn with_shared_fs(mut self, filesystem: SharedFs) -> Self {
        self.fs = Some(filesystem);
        self
    }

    /// Confine every candidate path to `root`.
    ///
    /// Paths are compared after `.` and `..` are resolved lexically and
    /// by whole segment, so neither `../../etc/passwd` nor a sibling
    /// directory whose name merely starts with the root's can be
    /// reached. Symbolic links are deliberately not followed when
    /// resolving, so a link inside the root cannot smuggle a target
    /// outside it past this check.
    ///
    /// Neither the canonical TypeScript nor the Go port has this: they
    /// resolve wherever the reference points. It is additive, so a
    /// resolver without a root behaves exactly as they do.
    pub fn with_root(mut self, root: impl Into<String>) -> Self {
        self.root = Some(root.into());
        self
    }

    fn filesystem(&self, options: &MultiSourceOptions) -> SharedFs {
        self.fs
            .clone()
            .or_else(|| options.fs.clone())
            .unwrap_or_else(|| Arc::new(OsFs))
    }

    fn allowed(&self, candidate: &str, os_paths: bool) -> bool {
        match &self.root {
            Some(root) => within_root(candidate, root, os_paths),
            None => true,
        }
    }

    fn load(&self, filesystem: &dyn SourceFs, path: &str, os_paths: bool) -> Option<String> {
        if !self.allowed(path, os_paths) {
            return None;
        }
        if let Some(src) = self.preload.get(path) {
            return Some(src.clone());
        }
        filesystem.read_file(path)
    }
}

impl Resolver for FileResolver {
    fn resolve(&self, reference: Option<&str>, input: &ResolverInput<'_>) -> Resolution {
        let filesystem = self.filesystem(input.options);
        let os_paths = filesystem.native_paths();

        let found = reference.map(|reference| match &self.pathfinder {
            Some(pathfinder) => pathfinder(reference),
            None => reference.to_string(),
        });

        let is_file = |path: &str| filesystem.read_file(path).is_some();
        let dir = |path: &str| filesystem.dir(path);
        let base = base_for(input, &is_file, &dir);

        let mut spec = resolve_path_spec(found.as_deref(), &base);
        let Some(full) = spec.full.clone() else {
            return Resolution::not_found(spec);
        };

        let full = filesystem.canon(&full);
        spec.full = Some(full.clone());
        spec.kind = ext_kind(Some(&full));

        let mut search = vec![full.clone()];

        if let Some(src) = self.load(filesystem.as_ref(), &full, os_paths) {
            return Resolution::found(spec, full, src).with_search(search);
        }

        let mut potentials: Vec<String> = Vec::new();

        // Support npm-linked references, as the canonical TypeScript
        // file resolver does: chase `node_modules/<path>` up through the
        // base's parents.
        if let Some(path) = &spec.path {
            let mut level = base.clone();
            let mut last: Option<String> = None;
            for _ in 0..NODE_MODULES_LEVELS {
                potentials.push(filesystem.canon(&filesystem.join(&[
                    &level,
                    "node_modules",
                    path,
                ])));
                let parent = filesystem.dir(&level);
                if last.as_deref() == Some(parent.as_str()) {
                    break;
                }
                last = Some(parent.clone());
                level = parent;
            }
        }

        if spec.kind == NONE {
            // `build_potentials` leads with the path itself, which was
            // tried above.
            potentials.extend(
                build_potentials(&full, &input.options.implicit_ext)
                    .into_iter()
                    .skip(1),
            );
        }

        search.extend(potentials.iter().cloned());

        for candidate in &potentials {
            if let Some(src) = self.load(filesystem.as_ref(), candidate, os_paths) {
                return Resolution::found(spec, candidate, src).with_search(search);
            }
        }

        Resolution::not_found(spec).with_search(search)
    }
}

// ---------------------------------------------------------------------
// The package resolver
// ---------------------------------------------------------------------

/// Resolve references inside `node_modules` folders.
///
/// Node's `require.resolve` has no Rust counterpart, so this is the
/// portable subset the Go port also implements: it walks `node_modules`
/// directories, honours a package's `package.json` `main` for a bare
/// reference, and tries implicit extensions and index files. Node's
/// conditional `exports` are not implemented.
#[derive(Clone, Default)]
pub struct PkgResolver {
    paths: Vec<String>,
    fs: Option<SharedFs>,
    root: Option<String>,
}

impl std::fmt::Debug for PkgResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PkgResolver")
            .field("paths", &self.paths)
            .field("fs", &self.fs.as_ref().map(|_| "<SourceFs>"))
            .field("root", &self.root)
            .finish()
    }
}

impl PkgResolver {
    /// A resolver that walks up from the working directory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Search these directories, and each of their parents, instead.
    pub fn with_paths<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.paths = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Read from `filesystem` instead of the one the plugin options name.
    pub fn with_fs(mut self, filesystem: impl SourceFs + 'static) -> Self {
        self.fs = Some(Arc::new(filesystem));
        self
    }

    /// Read from an already shared filesystem handle.
    pub fn with_shared_fs(mut self, filesystem: SharedFs) -> Self {
        self.fs = Some(filesystem);
        self
    }

    /// Confine every candidate path to `root`. See
    /// [`FileResolver::with_root`].
    pub fn with_root(mut self, root: impl Into<String>) -> Self {
        self.root = Some(root.into());
        self
    }

    fn filesystem(&self, options: &MultiSourceOptions) -> SharedFs {
        self.fs
            .clone()
            .or_else(|| options.fs.clone())
            .unwrap_or_else(|| Arc::new(OsFs))
    }

    fn allowed(&self, candidate: &str, os_paths: bool) -> bool {
        match &self.root {
            Some(root) => within_root(candidate, root, os_paths),
            None => true,
        }
    }
}

/// Is `reference` an explicit relative reference?
///
/// Such a reference is resolved against the containing source's
/// directory rather than treated as a package name.
fn is_relative_ref(reference: &str) -> bool {
    reference == "."
        || reference == ".."
        || reference.starts_with("./")
        || reference.starts_with("../")
        || reference.starts_with(".\\")
        || reference.starts_with("..\\")
}

/// `dir` followed by each of its parents.
fn ancestors(filesystem: &dyn SourceFs, dir: &str) -> Vec<String> {
    if dir.is_empty() {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    let mut current = dir.to_string();
    loop {
        dirs.push(current.clone());
        let parent = filesystem.dir(&current);
        if parent == current {
            break;
        }
        current = parent;
        // A malformed filesystem view that never reaches a fixed point
        // would loop forever; the real ones reach one in a handful of
        // steps, and a reference has no business climbing further.
        if dirs.len() > 64 {
            break;
        }
    }
    dirs
}

impl Resolver for PkgResolver {
    fn resolve(&self, reference: Option<&str>, input: &ResolverInput<'_>) -> Resolution {
        let filesystem = self.filesystem(input.options);
        let os_paths = filesystem.native_paths();

        let is_file = |path: &str| filesystem.read_file(path).is_some();
        let dir = |path: &str| filesystem.dir(path);
        let base = base_for(input, &is_file, &dir);

        let spec = resolve_path_spec(reference, &base);
        let Some(path) = spec.path.clone() else {
            return Resolution::not_found(spec);
        };
        if path.is_empty() {
            return Resolution::not_found(spec);
        }

        let load = |candidate: &str| -> Option<String> {
            if !self.allowed(candidate, os_paths) {
                return None;
            }
            filesystem.read_file(candidate)
        };

        // A relative reference found inside a source loaded from a
        // package is not a package name: resolve it against the
        // containing source's directory, exactly as the file resolver
        // does.
        if is_relative_ref(&path) {
            let Some(full) = spec.full.clone() else {
                return Resolution::not_found(spec);
            };
            let full = filesystem.canon(&full);
            let potentials = if ext_kind(Some(&full)) == NONE {
                build_potentials(&full, &input.options.implicit_ext)
            } else {
                vec![full]
            };
            for candidate in &potentials {
                if let Some(src) = load(candidate) {
                    return Resolution::found(spec, candidate, src).with_search(potentials.clone());
                }
            }
            return Resolution::not_found(spec).with_search(potentials);
        }

        let roots: Vec<String> = if !self.paths.is_empty() {
            self.paths.clone()
        } else if os_paths {
            std::env::current_dir()
                .map(|cwd| vec![cwd.to_string_lossy().into_owned()])
                .unwrap_or_default()
        } else {
            vec![".".to_string()]
        };

        let mut seen: Vec<String> = Vec::new();
        let mut search: Vec<String> = Vec::new();

        for root in &roots {
            for folder in ancestors(filesystem.as_ref(), &filesystem.canon(root)) {
                let node_modules = filesystem.join(&[&folder, "node_modules"]);
                if seen.contains(&node_modules) {
                    continue;
                }
                seen.push(node_modules.clone());

                if let Some((full, src)) = resolve_in_pkg_dir(
                    filesystem.as_ref(),
                    &load,
                    &node_modules,
                    &path,
                    &input.options.implicit_ext,
                    &mut search,
                ) {
                    return Resolution::found(spec, full, src).with_search(search);
                }
            }
        }

        Resolution::not_found(spec).with_search(search)
    }
}

/// Resolve a package reference inside one `node_modules` directory:
/// the reference itself, with implicit extensions and index files, then
/// the target package's `package.json` `main`.
fn resolve_in_pkg_dir(
    filesystem: &dyn SourceFs,
    load: &dyn Fn(&str) -> Option<String>,
    node_modules: &str,
    reference: &str,
    implicit_ext: &[String],
    search: &mut Vec<String>,
) -> Option<(String, String)> {
    let target = filesystem.join(&[node_modules, reference]);

    for candidate in build_potentials(&target, implicit_ext) {
        search.push(candidate.clone());
        if let Some(src) = load(&candidate) {
            return Some((candidate, src));
        }
    }

    let manifest = filesystem.join(&[&target, "package.json"]);
    search.push(manifest.clone());
    let text = load(&manifest)?;
    let main = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| {
            value
                .get("main")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })?;
    if main.is_empty() {
        return None;
    }
    let main_path = filesystem.join(&[&target, &main]);
    for candidate in build_potentials(&main_path, implicit_ext) {
        search.push(candidate.clone());
        if let Some(src) = load(&candidate) {
            return Some((candidate, src));
        }
    }
    None
}
