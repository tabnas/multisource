/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! Reading folders into memory before a parse starts.
//!
//! A preload turns per-reference filesystem reads into map lookups, and
//! is also how a caller vets what a parse may reach: scan once, inspect
//! the map, and give the resolver nothing else.

use std::collections::BTreeMap;

use crate::vfs::{OsFs, SourceFs};

/// Which folders to scan, and for what.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreloadOptions {
    /// Folders to scan.
    pub folders: Vec<String>,
    /// Extensions to load. Empty selects `.jsonic` and `.json`. A
    /// missing leading dot is added.
    pub ext: Vec<String>,
    /// Descend into subfolders.
    pub recursive: bool,
}

impl PreloadOptions {
    /// A scan of `folders`, non-recursive, with the default extensions.
    pub fn new<I, S>(folders: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        PreloadOptions {
            folders: folders.into_iter().map(Into::into).collect(),
            ext: Vec::new(),
            recursive: false,
        }
    }

    /// Load only files with these extensions.
    pub fn with_ext<I, S>(mut self, ext: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.ext = ext.into_iter().map(Into::into).collect();
        self
    }

    /// Descend into subfolders.
    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// The extension list with a leading dot on every entry, and the
    /// default when none was given.
    fn extensions(&self) -> Vec<String> {
        let raw: Vec<String> = if self.ext.is_empty() {
            vec![".jsonic".to_string(), ".json".to_string()]
        } else {
            self.ext.clone()
        };
        raw.into_iter()
            .map(|ext| {
                if ext.starts_with('.') {
                    ext
                } else {
                    format!(".{ext}")
                }
            })
            .collect()
    }
}

/// Scan the folders in `options` on the real filesystem, returning full
/// path to content for every file matching one of the extensions.
///
/// Folders that do not exist, and files that cannot be read, are
/// skipped. The result feeds [`crate::FileResolver::with_preload`].
///
/// ```
/// use tabnas_multisource::{preload_files_with, MapFs, PreloadOptions};
///
/// let disk = MapFs::from([("conf/a.jsonic", "a:1"), ("conf/note.txt", "skip")]);
/// let loaded = preload_files_with(&PreloadOptions::new(["conf"]), &disk);
/// assert_eq!(loaded.keys().collect::<Vec<_>>(), vec!["conf/a.jsonic"]);
/// ```
pub fn preload_files(options: &PreloadOptions) -> BTreeMap<String, String> {
    preload_files_with(options, &OsFs)
}

/// Scan the folders in `options` on `filesystem`.
pub fn preload_files_with(
    options: &PreloadOptions,
    filesystem: &dyn SourceFs,
) -> BTreeMap<String, String> {
    let extensions = options.extensions();
    let mut filemap = BTreeMap::new();
    for folder in &options.folders {
        scan(
            filesystem,
            &filesystem.canon(folder),
            &extensions,
            options.recursive,
            &mut filemap,
            0,
        );
    }
    filemap
}

/// How deep a recursive scan descends. A folder tree deep enough to
/// exhaust the stack is not a tree anyone means to preload, and the
/// walk is recursive.
const MAX_SCAN_DEPTH: usize = 64;

fn scan(
    filesystem: &dyn SourceFs,
    folder: &str,
    extensions: &[String],
    recursive: bool,
    filemap: &mut BTreeMap<String, String>,
    depth: usize,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Some(entries) = filesystem.read_dir(folder) else {
        return;
    };
    for entry in entries {
        let full = filesystem.join(&[folder, &entry.name]);
        if entry.is_dir {
            if recursive {
                scan(filesystem, &full, extensions, recursive, filemap, depth + 1);
            }
            continue;
        }
        if extensions
            .iter()
            .any(|ext| entry.name.ends_with(ext.as_str()))
        {
            if let Some(src) = filesystem.read_file(&full) {
                filemap.insert(full, src);
            }
        }
    }
}
