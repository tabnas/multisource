/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! The read-only filesystem seam the `file` and `pkg` resolvers reach
//! through.
//!
//! The canonical TypeScript injects a filesystem per parse as
//! `ctx.meta.fs`, because a JavaScript parse metadata object can carry a
//! whole `node:fs` module. A Rust parse metadata value is a
//! [`tabnas::Value`] and cannot carry a trait object, so the seam is a
//! typed option instead: [`SourceFs`] on the plugin options, or on one
//! resolver. That is the Rust spelling of Go's `MultiSourceOptions.FS`,
//! and it is what lets a test resolve from a map with no disk at all and
//! a caller confine resolution to a directory it chooses.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// One entry of a directory listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    /// The entry's own name, without any directory part.
    pub name: String,
    /// Whether the entry is a directory.
    pub is_dir: bool,
}

/// A read-only view of a filesystem.
///
/// Every path a resolver touches goes through one of these, so a caller
/// decides what a document can reach. [`OsFs`] reads the real filesystem
/// with absolute paths; [`MapFs`] reads an in-memory map with relative,
/// slash-separated paths, the Rust counterpart of Go's
/// `testing/fstest.MapFS` and of the `memfs` the TypeScript tests inject.
///
/// Implementations must be `Send + Sync`: a `Tabnas` instance parses
/// through `&self` and may be shared between threads.
pub trait SourceFs: Send + Sync {
    /// Read the file at a canonical path. `None` means it is not there,
    /// or cannot be read, which resolution treats the same way.
    fn read_file(&self, path: &str) -> Option<String>;

    /// List a directory. `None` means it is not there or is unreadable.
    fn read_dir(&self, path: &str) -> Option<Vec<DirEntry>>;

    /// Join path elements in this filesystem's convention.
    fn join(&self, parts: &[&str]) -> String;

    /// The parent directory of `path`.
    fn dir(&self, path: &str) -> String;

    /// The canonical lookup form of a possibly relative path.
    fn canon(&self, path: &str) -> String;

    /// Whether this filesystem addresses files by NATIVE absolute
    /// paths, as [`OsFs`] does, rather than by relative,
    /// slash-separated ones.
    ///
    /// It decides only how a sandbox root is compared against a
    /// candidate, and the default is the conservative answer: an
    /// injected filesystem keys on slashes, so `/` is the separator and
    /// a Windows drive letter is not in play.
    fn native_paths(&self) -> bool {
        false
    }
}

impl fmt::Debug for dyn SourceFs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<SourceFs>")
    }
}

/// The real filesystem, addressed by absolute native paths.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsFs;

impl SourceFs for OsFs {
    fn read_file(&self, path: &str) -> Option<String> {
        std::fs::read(path)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    fn read_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path).ok()? {
            let Ok(entry) = entry else { continue };
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            entries.push(DirEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir,
            });
        }
        // `read_dir` yields entries in whatever order the platform
        // stores them, so a preload of one directory would key its map
        // differently from run to run. Sorting makes a scan reproducible.
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Some(entries)
    }

    fn join(&self, parts: &[&str]) -> String {
        let mut joined = PathBuf::new();
        for part in parts {
            joined.push(part);
        }
        lexical(&joined).to_string_lossy().into_owned()
    }

    fn dir(&self, path: &str) -> String {
        Path::new(path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .filter(|parent| !parent.is_empty())
            .unwrap_or_else(|| path.to_string())
    }

    fn native_paths(&self) -> bool {
        true
    }

    fn canon(&self, path: &str) -> String {
        let candidate = Path::new(path);
        let absolute = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            match std::env::current_dir() {
                Ok(cwd) => cwd.join(candidate),
                Err(_) => candidate.to_path_buf(),
            }
        };
        lexical(&absolute).to_string_lossy().into_owned()
    }
}

/// An in-memory filesystem: relative, slash-separated paths to content.
///
/// Directories are implied by the keys, so `MapFs::from([("a/b.jsonic",
/// "x:1")])` has a directory `a`. This is the hermetic resolver the
/// tests use, and the shape a caller supplies to sandbox a parse to
/// content it has already vetted.
#[derive(Clone, Debug, Default)]
pub struct MapFs {
    files: BTreeMap<String, String>,
}

impl MapFs {
    /// An empty filesystem.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one file, replacing any file already at that path.
    pub fn insert(&mut self, path: impl AsRef<str>, content: impl Into<String>) -> &mut Self {
        self.files.insert(clean(path.as_ref()), content.into());
        self
    }

    /// Add one file, by value, so a filesystem can be built in an
    /// expression.
    pub fn with(mut self, path: impl AsRef<str>, content: impl Into<String>) -> Self {
        self.insert(path, content);
        self
    }

    /// Every path this filesystem holds, in sorted order.
    pub fn paths(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }
}

impl<K, V> FromIterator<(K, V)> for MapFs
where
    K: AsRef<str>,
    V: Into<String>,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(entries: T) -> Self {
        let mut filesystem = MapFs::new();
        for (path, content) in entries {
            filesystem.insert(path, content);
        }
        filesystem
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for MapFs
where
    K: AsRef<str>,
    V: Into<String>,
{
    fn from(entries: [(K, V); N]) -> Self {
        entries.into_iter().collect()
    }
}

impl SourceFs for MapFs {
    fn read_file(&self, path: &str) -> Option<String> {
        self.files.get(&clean(path)).cloned()
    }

    fn read_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        let folder = clean(path);
        let prefix = if folder == "." {
            String::new()
        } else {
            format!("{folder}/")
        };
        let mut entries: BTreeMap<String, bool> = BTreeMap::new();
        for key in self.files.keys() {
            let Some(rest) = key.strip_prefix(prefix.as_str()) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            match rest.split_once('/') {
                Some((head, _)) => {
                    entries.insert(head.to_string(), true);
                }
                None => {
                    entries.insert(rest.to_string(), false);
                }
            }
        }
        if entries.is_empty() && folder != "." {
            return None;
        }
        Some(
            entries
                .into_iter()
                .map(|(name, is_dir)| DirEntry { name, is_dir })
                .collect(),
        )
    }

    fn join(&self, parts: &[&str]) -> String {
        clean(&parts.join("/"))
    }

    fn dir(&self, path: &str) -> String {
        let cleaned = clean(path);
        match cleaned.rsplit_once('/') {
            Some((head, _)) if !head.is_empty() => head.to_string(),
            Some(_) => ".".to_string(),
            None => ".".to_string(),
        }
    }

    fn canon(&self, path: &str) -> String {
        clean(path)
    }
}

/// Normalize a path into a map key: slash separated, `.` and `..`
/// resolved, no leading slash, and `.` for the root. The counterpart of
/// Go's `fsClean`.
pub(crate) fn clean(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for segment in unified.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                // A `..` that would climb above the root is dropped,
                // exactly as `path.Clean` drops it, so a reference can
                // never name a parent of the filesystem root.
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Resolve `.` and `..` without touching the filesystem.
///
/// Deliberately lexical: `std::fs::canonicalize` requires the path to
/// exist, and resolution asks about paths that mostly do not. It also
/// follows symbolic links, which would let a link inside a sandbox root
/// resolve to a target outside it while the confinement check, run on
/// the resolved path, still said yes.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Is `candidate` at or below `root`, comparing whole path segments?
///
/// Both are compared after lexical resolution, so `root/../etc` is
/// rejected rather than accepted on the strength of its prefix. Segment
/// comparison is what stops `/srv/appdata` passing for `/srv/app`.
pub(crate) fn within_root(candidate: &str, root: &str, os_paths: bool) -> bool {
    let (candidate, root) = if os_paths {
        (
            lexical(Path::new(candidate)).to_string_lossy().into_owned(),
            lexical(Path::new(root)).to_string_lossy().into_owned(),
        )
    } else {
        (clean(candidate), clean(root))
    };
    if root.is_empty() || root == "." {
        return true;
    }
    if candidate == root {
        return true;
    }
    let separator = if os_paths {
        std::path::MAIN_SEPARATOR
    } else {
        '/'
    };
    let mut prefix = root;
    if !prefix.ends_with(separator) {
        prefix.push(separator);
    }
    candidate.starts_with(&prefix)
}

/// A shared filesystem handle.
pub type SharedFs = Arc<dyn SourceFs>;

/// The real filesystem as a shared handle.
pub fn os_fs() -> SharedFs {
    Arc::new(OsFs)
}
