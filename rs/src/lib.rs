/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! Load partial values from multiple external sources into one parse
//! result, as a plugin for the [tabnas](https://github.com/tabnas/parser)
//! parsing engine over the
//! [jsonic](https://github.com/tabnas/jsonic) relaxed-JSON grammar.
//!
//! A directive character (`@` by default) marks a reference in the
//! input. The plugin **resolves** the reference to a source, **processes**
//! that source into a value, and splices the value into the output,
//! recursively, so a loaded source can reference more sources.
//!
//! ```
//! use tabnas_multisource::{make_with, MapResolver, MultiSourceOptions};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let sources = MapResolver::from([("a.jsonic", "a:1")]);
//!     let parser = make_with(MultiSourceOptions::new(sources));
//!     assert_eq!(parser.parse(r#"@"a.jsonic" b:2"#)?.to_string(), r#"{"a":1,"b":2}"#);
//!     Ok(())
//! }
//! ```
//!
//! TypeScript is canonical: `ts/src/multisource.ts` defines behaviour,
//! option names and defaults. The shared fixtures in `test/spec/*.tsv`
//! are the parity contract across TypeScript, Go and Rust. Differences
//! are recorded in `DIVERGENCE.md` at the repository root.
//!
//! # Untrusted input
//!
//! Following `@` references is this plugin's whole job, which is exactly
//! why the boundary matters. A reference reaches only what the configured
//! [`Resolver`] can see, so the resolver is the sandbox:
//!
//! - [`MapResolver`] and a [`FileResolver`] over a [`MapFs`] touch no
//!   disk at all.
//! - [`FileResolver::with_root`] confines every candidate path to one
//!   directory, so `@"../../etc/passwd"` resolves to nothing rather than
//!   to a file. The root and the candidate are both resolved through the
//!   filesystem before they are compared, so a symbolic link inside the
//!   root is judged by where it points; a link created between that
//!   check and the read is beyond what a resolver can see.
//! - A cycle between sources raises `multisource_cycle`, and a chain
//!   longer than [`MultiSourceOptions::max_depth`] raises
//!   `multisource_depth`, so neither hangs nor overflows the stack.
//!
//! A loaded value is still data, never instructions, and splicing is not
//! sanitising: escaping for SQL, HTML or a shell remains the caller's job.

#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::json;

use tabnas::{
    ActionError, Context, GrammarSetting, GrammarSpec, Plugin, PluginError, Rule, Tabnas,
    TabnasError, Token, Value,
};
use tabnas_directive::{set_node, DirectiveOptions, RuleMod, RulesOption};

mod preload;
mod processor;
mod resolver;
mod vfs;

pub use preload::{preload_files, preload_files_with, PreloadOptions};
pub use processor::{
    default_processor, json_processor, jsonic_processor, parse_nested, Processor, ProcessorInput,
};
pub use resolver::{FileResolver, MapResolver, PkgResolver};
pub use vfs::{os_fs, DirEntry, MapFs, OsFs, SharedFs, SourceFs};

/// VERSION is this crate's version. It MUST equal `ts/package.json`
/// "version" and the `version` field in `rs/Cargo.toml`: the release
/// orchestrator rewrites all of them, and `tests/version_test.rs` fails
/// the build if they drift. Mirrors `VERSION` in `ts/src/multisource.ts`
/// and `const VERSION` in `go/multisource.go`.
pub const VERSION: &str = "0.5.8";

/// An unknown or missing source-reference extension.
pub const NONE: &str = "";

/// The top of the dependency tree: the [`DependencyMap`] target key used
/// for sources referenced directly by the top-level parse.
///
/// The TypeScript export is a `Symbol`, which Rust has no counterpart
/// for, so this mirrors the Go port's sentinel. The leading NUL byte
/// guarantees it cannot collide with a real source path.
pub const TOP: &str = "\0TOP";

/// The plugin name, as the engine records it.
pub const PLUGIN_NAME: &str = "MultiSource";

/// The rule and directive this plugin installs.
pub const RULE_NAME: &str = "multisource";

/// The default directive character.
pub const DEFAULT_MARKCHAR: &str = "@";

/// How deep a chain of sources may nest before the parse is failed.
///
/// Each nested source is parsed inside the parse that referenced it, so
/// the chain runs on the call stack. Without a cap a chain of a few
/// thousand sources with no cycle in it overflows that stack and aborts
/// the process, which is why this is a cap rather than a note. See
/// `DIVERGENCE.md`.
pub const DEFAULT_MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------
// Path specs and resolutions
// ---------------------------------------------------------------------

/// The path to a source, including the base prefix when there is one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathSpec {
    /// The source kind, normally the extension of the last path segment.
    pub kind: String,
    /// The reference exactly as written, which may be relative.
    pub path: Option<String>,
    /// The normalized full path.
    pub full: Option<String>,
    /// The base the reference resolved against.
    pub base: String,
    /// Whether the reference was absolute.
    pub abs: bool,
}

/// A source and where it was found.
#[derive(Clone, Debug)]
pub struct Resolution {
    /// The normalized path, rewritten by the resolver when an implicit
    /// extension or an index file matched.
    pub spec: PathSpec,
    /// The source text. `None` when nothing was found.
    pub src: Option<String>,
    /// The processed value, filled by a [`Processor`].
    pub val: Value,
    /// Whether a source was found.
    pub found: bool,
    /// Every path that was tried, for the not-found report.
    pub search: Vec<String>,
    /// A failure while PROCESSING found source, for example a nested
    /// reference inside it that could not be resolved. The plugin
    /// propagates it so the parse fails, matching TypeScript, which lets
    /// a nested parse error escape rather than substituting raw text.
    pub err: Option<Box<TabnasError>>,
}

impl Default for Resolution {
    fn default() -> Self {
        Resolution::not_found(PathSpec::default())
    }
}

impl Resolution {
    /// A resolution that found nothing, for the spec it was built from.
    pub fn not_found(spec: PathSpec) -> Self {
        Resolution {
            spec,
            src: None,
            val: Value::Undefined,
            found: false,
            search: Vec::new(),
            err: None,
        }
    }

    /// A resolution that found `src` at `full`.
    pub fn found(mut spec: PathSpec, full: impl Into<String>, src: impl Into<String>) -> Self {
        let full = full.into();
        spec.kind = ext_kind(Some(&full));
        spec.full = Some(full);
        Resolution {
            spec,
            src: Some(src.into()),
            val: Value::Undefined,
            found: true,
            search: Vec::new(),
            err: None,
        }
    }

    /// Record the paths that were searched.
    pub fn with_search(mut self, search: Vec<String>) -> Self {
        self.search = search;
        self
    }

    /// The path this resolution is known by: the normalized full path,
    /// else the reference as written, else `no-path`. Mirrors the
    /// TypeScript `fullpath` fallback chain.
    pub fn full_path(&self) -> String {
        self.spec
            .full
            .clone()
            .or_else(|| self.spec.path.clone())
            .unwrap_or_else(|| "no-path".to_string())
    }
}

/// What a [`Resolver`] is given: the plugin options and the parse
/// metadata for the enclosing source.
///
/// The canonical TypeScript hands a resolver the whole parse `Context`,
/// of which resolvers read only `ctx.meta`. Narrowing it to the metadata
/// keeps a resolver callable, and therefore testable, outside a parse.
#[derive(Clone, Copy)]
pub struct ResolverInput<'a> {
    /// The plugin options in force for this parse.
    pub options: &'a MultiSourceOptions,
    /// The parse metadata. `multisource.path` names the source that
    /// contains this reference, when there is one.
    pub meta: &'a Value,
}

impl fmt::Debug for ResolverInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolverInput")
            .field("meta", self.meta)
            .finish_non_exhaustive()
    }
}

impl ResolverInput<'_> {
    /// The full path of the source that contains this reference, when
    /// the reference was found inside another source.
    pub fn source_path(&self) -> Option<String> {
        meta_multisource_string(self.meta, "path")
    }
}

/// Find the source content a reference names.
///
/// Implementations must be `Send + Sync`: a `Tabnas` instance parses
/// through `&self`, so the plugin, its options and every callback they
/// hold are shared across threads. Nothing is held in global state, so
/// two parsers configured differently never see each other's sources.
pub trait Resolver: Send + Sync {
    /// Resolve `reference` to a [`Resolution`]. A reference that names
    /// nothing returns [`Resolution::not_found`]; raising the error is
    /// the plugin's job, not the resolver's.
    fn resolve(&self, reference: Option<&str>, input: &ResolverInput<'_>) -> Resolution;
}

impl<F> Resolver for F
where
    F: Fn(Option<&str>, &ResolverInput<'_>) -> Resolution + Send + Sync,
{
    fn resolve(&self, reference: Option<&str>, input: &ResolverInput<'_>) -> Resolution {
        self(reference, input)
    }
}

/// A shared resolver handle.
pub type SharedResolver = Arc<dyn Resolver>;

// ---------------------------------------------------------------------
// Dependencies
// ---------------------------------------------------------------------

/// One record that target `tar` pulled in source `src` during a parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// The target that depends on the source; [`TOP`] at the top level.
    pub tar: String,
    /// The source the target depends on.
    pub src: String,
    /// When the source resolved, in Unix milliseconds.
    pub wen: u64,
}

/// A flattened dependency tree, keyed by target full path and then by
/// source full path.
///
/// The TypeScript plugin fills an object the caller passes in the parse
/// metadata. A Rust parse metadata value cannot be written back to, so
/// the sink is a typed option instead:
/// [`MultiSourceOptions::with_deps`].
pub type DependencyMap = BTreeMap<String, BTreeMap<String, Dependency>>;

// ---------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------

/// How one source kind becomes a value.
#[derive(Clone)]
pub enum ProcessorEntry {
    /// A processor callback.
    Call(Arc<dyn Processor>),
    /// Another kind's processor, by name. One level of aliasing, as in
    /// the canonical TypeScript, where a processor entry may be a string
    /// naming another kind.
    Alias(String),
}

impl fmt::Debug for ProcessorEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(_) => formatter.write_str("ProcessorEntry::Call(<function>)"),
            Self::Alias(kind) => write!(formatter, "ProcessorEntry::Alias({kind:?})"),
        }
    }
}

/// Configuration for the plugin.
#[derive(Clone)]
pub struct MultiSourceOptions {
    /// Resolves a reference to a source.
    pub resolver: SharedResolver,
    /// The base path references resolve against at the top level.
    pub path: String,
    /// The character that starts a reference.
    pub markchar: String,
    /// Per-kind processors. The empty kind is the fallback.
    pub processor: IndexMap<String, ProcessorEntry>,
    /// Extensions tried for a reference with none, in order.
    pub implicit_ext: Vec<String>,
    /// A declarative record of a folder preload. As in TypeScript and
    /// Go, the plugin does not consume it: call [`preload_files`] and
    /// hand the result to [`FileResolver::with_preload`].
    pub preload: Option<PreloadOptions>,
    /// The filesystem the `file` and `pkg` resolvers read, when they
    /// carry none of their own. `None` is the real filesystem.
    ///
    /// This is the Rust spelling of the TypeScript `ctx.meta.fs`
    /// injection point and of Go's `MultiSourceOptions.FS`.
    pub fs: Option<SharedFs>,
    /// Where to record the `tar` to `src` dependency tree, when the
    /// caller wants one.
    pub deps: Option<Arc<Mutex<DependencyMap>>>,
    /// How deep a chain of sources may nest. See [`DEFAULT_MAX_DEPTH`].
    pub max_depth: usize,
}

impl fmt::Debug for MultiSourceOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiSourceOptions")
            .field("path", &self.path)
            .field("markchar", &self.markchar)
            .field("processor", &self.processor)
            .field("implicit_ext", &self.implicit_ext)
            .field("preload", &self.preload)
            .field("fs", &self.fs.as_ref().map(|_| "<SourceFs>"))
            .field("deps", &self.deps.as_ref().map(|_| "<DependencyMap>"))
            .field("max_depth", &self.max_depth)
            .finish_non_exhaustive()
    }
}

impl Default for MultiSourceOptions {
    fn default() -> Self {
        Self::new(MapResolver::new())
    }
}

impl MultiSourceOptions {
    /// Options resolving through `resolver`, everything else default.
    pub fn new(resolver: impl Resolver + 'static) -> Self {
        MultiSourceOptions {
            resolver: Arc::new(resolver),
            path: String::new(),
            markchar: DEFAULT_MARKCHAR.to_string(),
            processor: default_processors(),
            implicit_ext: default_implicit_ext(),
            preload: None,
            fs: None,
            deps: None,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Options resolving through an already shared resolver.
    pub fn with_resolver(mut self, resolver: SharedResolver) -> Self {
        self.resolver = resolver;
        self
    }

    /// Set the base path references resolve against.
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the directive character.
    pub fn with_markchar(mut self, markchar: impl Into<String>) -> Self {
        self.markchar = markchar.into();
        self
    }

    /// Register a processor for one kind, replacing any entry for it.
    pub fn with_processor(
        mut self,
        kind: impl Into<String>,
        processor: impl Processor + 'static,
    ) -> Self {
        self.processor
            .insert(kind.into(), ProcessorEntry::Call(Arc::new(processor)));
        self
    }

    /// Point one kind at another kind's processor.
    pub fn with_processor_alias(
        mut self,
        kind: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        self.processor
            .insert(kind.into(), ProcessorEntry::Alias(target.into()));
        self
    }

    /// Replace the whole processor table.
    pub fn with_processors(mut self, processors: IndexMap<String, ProcessorEntry>) -> Self {
        self.processor = processors;
        self
    }

    /// Replace the implicit extension list. A missing leading dot is
    /// added, as the TypeScript plugin does when it installs.
    pub fn with_implicit_ext<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.implicit_ext = extensions
            .into_iter()
            .map(|ext| dotted(ext.as_ref()))
            .collect();
        self
    }

    /// Record a preload configuration. The plugin does not act on it.
    pub fn with_preload(mut self, preload: PreloadOptions) -> Self {
        self.preload = Some(preload);
        self
    }

    /// Set the filesystem the file and pkg resolvers read.
    pub fn with_fs(mut self, filesystem: impl SourceFs + 'static) -> Self {
        self.fs = Some(Arc::new(filesystem));
        self
    }

    /// Set the filesystem from an already shared handle.
    pub fn with_shared_fs(mut self, filesystem: SharedFs) -> Self {
        self.fs = Some(filesystem);
        self
    }

    /// Collect the dependency tree into `deps` as the parse runs.
    pub fn with_deps(mut self, deps: Arc<Mutex<DependencyMap>>) -> Self {
        self.deps = Some(deps);
        self
    }

    /// Set how deep a chain of sources may nest.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// The processor for `kind`, resolving one level of aliasing and
    /// falling back to the [`NONE`] entry, as `getProcessor` does.
    pub fn processor_for(&self, kind: &str) -> Option<Arc<dyn Processor>> {
        let fallback = match self.processor.get(NONE) {
            Some(ProcessorEntry::Call(processor)) => Some(Arc::clone(processor)),
            _ => None,
        };
        match self.processor.get(kind) {
            Some(ProcessorEntry::Call(processor)) => Some(Arc::clone(processor)),
            Some(ProcessorEntry::Alias(target)) => match self.processor.get(target.as_str()) {
                Some(ProcessorEntry::Call(processor)) => Some(Arc::clone(processor)),
                // An alias to an alias is one level too many, exactly as
                // in TypeScript, where the second lookup yields a string
                // and nothing callable comes back.
                _ => fallback,
            },
            None => fallback,
        }
    }
}

/// The default processor table: raw text for an unknown kind, the strict
/// JSON reader for `json`, and a re-parse with the live engine for
/// `jsonic` and `jsc`.
pub fn default_processors() -> IndexMap<String, ProcessorEntry> {
    let mut processors = IndexMap::new();
    processors.insert(
        NONE.to_string(),
        ProcessorEntry::Call(Arc::new(default_processor)),
    );
    processors.insert(
        "jsonic".to_string(),
        ProcessorEntry::Call(Arc::new(jsonic_processor)),
    );
    processors.insert(
        "jsc".to_string(),
        ProcessorEntry::Alias("jsonic".to_string()),
    );
    processors.insert(
        "json".to_string(),
        ProcessorEntry::Call(Arc::new(json_processor)),
    );
    processors
}

/// The default implicit extensions.
///
/// `.js` is deliberately absent: the TypeScript `js` kind `require`s,
/// which is to say executes, the module, and Rust has no JavaScript
/// runtime to do that in. The Go port leaves it out for the same reason.
/// See `DIVERGENCE.md`.
pub fn default_implicit_ext() -> Vec<String> {
    vec![
        ".jsonic".to_string(),
        ".jsc".to_string(),
        ".json".to_string(),
    ]
}

fn dotted(ext: &str) -> String {
    if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{ext}")
    }
}

// ---------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------

/// The source kind: the extension of the LAST path segment, without the
/// dot, or [`NONE`] when that segment has no dot.
///
/// It must be segment-scoped. Matching the whole path makes a dot in a
/// parent folder (`a.d/foo`) produce a kind that is not [`NONE`], so the
/// implicit-extension search is skipped and an extensionless reference
/// under a dotted folder is wrongly reported as missing.
pub fn ext_kind(full: Option<&str>) -> String {
    let Some(full) = full else {
        return NONE.to_string();
    };
    let segment = match full.rfind(['/', '\\']) {
        Some(index) => &full[index + 1..],
        None => full,
    };
    match segment.rfind('.') {
        Some(index) => segment[index + 1..].to_string(),
        None => NONE.to_string(),
    }
}

/// Normalize a reference into a [`PathSpec`] against `base`.
///
/// The counterpart of the TypeScript `resolvePathSpec` and Go's
/// `ResolvePathSpec`. An absolute reference ignores the base.
pub fn resolve_path_spec(reference: Option<&str>, base: &str) -> PathSpec {
    let path = reference.map(str::to_string);
    let abs = path
        .as_deref()
        .map(|path| path.starts_with('/') || path.starts_with('\\'))
        .unwrap_or(false);
    let full = match path.as_deref() {
        Some(path) if abs => Some(path.to_string()),
        Some(path) if !path.is_empty() => Some(if base.is_empty() {
            path.to_string()
        } else {
            format!("{base}/{path}")
        }),
        _ => None,
    };
    PathSpec {
        kind: ext_kind(full.as_deref()),
        path,
        full,
        base: base.to_string(),
        abs,
    }
}

/// Every path a reference could name: the path itself, then the implicit
/// extensions, then a folder index file, then a folder index file that
/// repeats the folder name.
///
/// Whether a reference already has an extension is a property of its LAST
/// segment, not of the whole path, for the reason [`ext_kind`] gives.
pub fn build_potentials(full: &str, implicit_ext: &[String]) -> Vec<String> {
    if full.is_empty() {
        return Vec::new();
    }
    let mut potentials = vec![full.to_string()];
    let segment = match full.rfind(['/', '\\']) {
        Some(index) => &full[index + 1..],
        None => full,
    };
    if segment.contains('.') {
        return potentials;
    }
    for ext in implicit_ext {
        potentials.push(format!("{full}{ext}"));
    }
    for ext in implicit_ext {
        potentials.push(format!("{full}/index{ext}"));
    }
    if !segment.is_empty() && segment != "." {
        for ext in implicit_ext {
            potentials.push(format!("{full}/index.{segment}{ext}"));
        }
    }
    potentials
}

/// The directory a relative reference inside `path` resolves against.
///
/// A path with no separator (an in-memory key such as `a.jsonic`) yields
/// the empty string, so a bare nested reference resolves plainly; a
/// separator at index zero yields the root separator. Mirrors Go's
/// `sourceDir` and the TypeScript mem resolver's `resolvefolder`.
pub fn source_dir(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match trimmed.rfind(['/', '\\']) {
        None => String::new(),
        Some(0) => trimmed[..1].to_string(),
        Some(index) => trimmed[..index].to_string(),
    }
}

// ---------------------------------------------------------------------
// Parse metadata
// ---------------------------------------------------------------------

fn meta_object(meta: &Value) -> Option<&IndexMap<String, Value>> {
    match meta {
        Value::Object(map) => Some(map),
        Value::MapRef(map) => Some(&map.value),
        _ => None,
    }
}

fn meta_get<'a>(meta: &'a Value, key: &str) -> Option<&'a Value> {
    meta_object(meta).and_then(|map| map.get(key))
}

fn meta_multisource_string(meta: &Value, key: &str) -> Option<String> {
    match meta_get(meta, RULE_NAME).and_then(|entry| meta_get(entry, key)) {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Text(text)) => Some(text.string.clone()),
        _ => None,
    }
}

/// Was this metadata built by [`child_meta`], rather than handed in by
/// a caller?
fn is_nested_meta(meta: &Value) -> bool {
    matches!(
        meta_get(meta, RULE_NAME).and_then(|entry| meta_get(entry, NESTED_MARK)),
        Some(Value::Bool(true))
    )
}

fn meta_parents(meta: &Value) -> Vec<String> {
    match meta_get(meta, RULE_NAME).and_then(|entry| meta_get(entry, "parents")) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.clone()),
                Value::Text(text) => Some(text.string.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The key marking metadata this plugin built for a nested parse.
///
/// The prepare hook needs to tell a nested parse from a top-level one,
/// and `multisource.path` cannot do it: a caller may legitimately set
/// that at the top level to choose the base a relative reference
/// resolves against, exactly as the canonical TypeScript tests do.
const NESTED_MARK: &str = "nested";

/// The metadata a nested parse gets: the parent's metadata with the
/// multisource entry updated to name this source and its ancestors.
/// Mirrors the TypeScript action's `meta` construction and Go's
/// `childMeta`; the parent metadata is never mutated.
fn child_meta(
    parent: &Value,
    resolution: &Resolution,
    parents: &[String],
    dive: Option<&Value>,
    ticket: &str,
) -> Value {
    let mut child = meta_object(parent).cloned().unwrap_or_default();

    let previous = meta_get(parent, RULE_NAME)
        .and_then(meta_object)
        .cloned()
        .unwrap_or_default();
    let mut entry = previous;
    entry.insert(
        "parents".to_string(),
        Value::array(parents.iter().cloned().map(Value::String).collect()),
    );
    entry.insert(
        "path".to_string(),
        match &resolution.spec.full {
            Some(full) => Value::String(full.clone()),
            None => Value::Undefined,
        },
    );
    entry.insert(NESTED_MARK.to_string(), Value::Bool(true));
    // The slot this nested parse records a diagnostic under, so the
    // load below reads back its own report and no other parse's.
    entry.insert(REPORT_TICKET.to_string(), Value::String(ticket.to_string()));
    child.insert(RULE_NAME.to_string(), Value::object(entry));

    if let Some(path) = &resolution.spec.path {
        child.insert("fileName".to_string(), Value::String(path.clone()));
    }

    // The path-diving plugin reads `meta.path.base` to seed the path of
    // the root value. Carrying the current dive down means a reference
    // that lands at a dived key keeps its place in the tree.
    if let Some(Value::Array(segments)) = dive {
        let mut path = IndexMap::new();
        path.insert("base".to_string(), Value::array(segments.as_ref().clone()));
        child.insert("path".to_string(), Value::object(path));
    }

    Value::object(child)
}

// ---------------------------------------------------------------------
// The nested-parser slot
// ---------------------------------------------------------------------

/// The live parser, published for the duration of a top-level parse so
/// that a processor which re-parses its source uses the instance the
/// document is being parsed by, plugins installed after this one
/// included.
///
/// The canonical TypeScript closes over the instance, and a JavaScript
/// closure sees every later change to it. A Rust callback cannot hold a
/// borrow across a parse, so the instance is cloned once per top-level
/// parse that could contain a reference at all, and nested parses reuse
/// what the top-level parse published.
#[derive(Default)]
struct NestedParser {
    current: Mutex<Option<Arc<Tabnas>>>,
}

impl NestedParser {
    fn publish(&self, parser: &Tabnas) {
        if let Ok(mut slot) = self.current.lock() {
            *slot = Some(Arc::new(parser.clone()));
        }
    }

    fn get(&self) -> Option<Arc<Tabnas>> {
        self.current.lock().ok().and_then(|slot| slot.clone())
    }
}

/// A raised diagnostic: its code and the detail bag its message and
/// hint are rendered from.
type Report = (String, Vec<(String, Value)>);

/// The diagnostics this plugin has raised inside nested parses, so that
/// an outer load can re-raise one unchanged.
///
/// In the canonical TypeScript a nested parse throws and the exception
/// travels out untouched, so the caller reads the innermost report: the
/// loop of a cycle, the search paths of a missing source. Here a nested
/// parse RETURNS its error, whose rendered text an outer level can read
/// but whose detail bag it cannot, so the bag is recorded as it is
/// raised and taken again when the same code comes back.
///
/// Each nested load gets its OWN slot, keyed by a ticket the loading
/// action mints and threads to the nested parse through its metadata.
/// One slot per instance was not enough: this type is shared by every
/// parse of an instance, and an instance is explicitly shareable
/// between threads, so two parses failing at the same time overwrote
/// each other and an error could come back naming the other document's
/// path, search list or loop. The ticket also survives the segment
/// thread `processor::parse_nested` runs a level on, which a
/// thread-local slot would not.
///
/// A slot is TAKEN, not read, so a report is used once, and the loading
/// action drops its ticket whether or not the load failed.
#[derive(Default)]
struct LastReport {
    slots: Mutex<BTreeMap<String, Report>>,
    next: std::sync::atomic::AtomicU64,
}

impl LastReport {
    /// A ticket for one nested load, unique within this instance.
    fn ticket(&self) -> String {
        let count = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("{count}")
    }

    fn record(&self, ticket: &str, code: &str, details: &[(String, Value)]) {
        if let Ok(mut slots) = self.slots.lock() {
            slots.insert(ticket.to_string(), (code.to_string(), details.to_vec()));
        }
    }

    /// The report a nested parse left under `ticket`, if it carries the
    /// code that came back. The slot is freed either way.
    fn take(&self, ticket: &str, code: &str) -> Option<Vec<(String, Value)>> {
        let mut slots = self.slots.lock().ok()?;
        match slots.remove(ticket) {
            Some((recorded, details)) if recorded == code => Some(details),
            _ => None,
        }
    }

    /// Drop a ticket whose load did not fail, so a slot cannot outlive
    /// the parse that minted it.
    fn discard(&self, ticket: &str) {
        if let Ok(mut slots) = self.slots.lock() {
            slots.remove(ticket);
        }
    }
}

/// The metadata key carrying the ticket a nested parse records its
/// report under. The loading action mints it; the nested parse only
/// reads it back.
const REPORT_TICKET: &str = "report";

// ---------------------------------------------------------------------
// The plugin
// ---------------------------------------------------------------------

/// Build the plugin for `options`.
///
/// The returned [`Plugin`] installs through [`Tabnas::use_plugin`], so a
/// derived instance rebuilds it. Typed option data lives in the returned
/// closure rather than in the engine's serialized plugin-option bag,
/// because a resolver and a processor are Rust callbacks and no
/// [`Value`] can carry one.
pub fn plugin_with(options: MultiSourceOptions) -> Plugin {
    let options = Arc::new(normalize(options));
    Plugin::new(PLUGIN_NAME, move |parser, _plugin_options| {
        install(parser, Arc::clone(&options)).map_err(PluginError::from)
    })
}

/// Build the plugin with default options: no sources, so every reference
/// raises `multisource_not_found`.
pub fn plugin() -> Plugin {
    plugin_with(MultiSourceOptions::default())
}

/// Install the plugin on `parser`.
///
/// The convenience constructor mirroring `tn.use(MultiSource, options)`.
/// The host grammar must already supply `val`, `map` and `pair`: install
/// [`tabnas_jsonic`] first, or use [`make_with`].
pub fn multisource(
    parser: &mut Tabnas,
    options: MultiSourceOptions,
) -> Result<(), MultiSourceError> {
    parser
        .use_plugin(plugin_with(options), None)
        .map(|_| ())
        .map_err(MultiSourceError::from)
}

/// A registration failure: a host grammar without the rules the plugin
/// modifies, a directive character already taken, or a grammar the
/// engine refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiSourceError(pub String);

impl fmt::Display for MultiSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MultiSourceError {}

impl From<PluginError> for MultiSourceError {
    fn from(error: PluginError) -> Self {
        Self(error.0)
    }
}

impl From<MultiSourceError> for PluginError {
    fn from(error: MultiSourceError) -> Self {
        Self(error.0)
    }
}

/// Fill in what the plugin would otherwise have to guess at parse time.
fn normalize(mut options: MultiSourceOptions) -> MultiSourceOptions {
    if options.markchar.is_empty() {
        options.markchar = DEFAULT_MARKCHAR.to_string();
    }
    for ext in &mut options.implicit_ext {
        if !ext.starts_with('.') {
            *ext = format!(".{ext}");
        }
    }
    if options.max_depth == 0 {
        options.max_depth = DEFAULT_MAX_DEPTH;
    }
    options
}

fn install(parser: &mut Tabnas, options: Arc<MultiSourceOptions>) -> Result<(), MultiSourceError> {
    // The plugin modifies `val`, `map` and `pair`: it defines no value
    // grammar of its own. Installing it on a bare engine would register
    // a directive that can never match, so say so instead.
    let rules = parser.rule_names();
    for required in ["val", "map", "pair"] {
        if !rules.iter().any(|name| name == required) {
            return Err(MultiSourceError(format!(
                "MultiSource needs a host grammar supplying the {required:?} rule: \
                 install tabnas_jsonic first, or use make_with"
            )));
        }
    }

    // One slot per install, so two instances never share a published
    // parser, and a derived instance gets its own.
    let nested = Arc::new(NestedParser::default());
    let reports = Arc::new(LastReport::default());

    parser
        .set_options(|engine| {
            engine.error.insert(
                "multisource_not_found".to_string(),
                "source not found: {path}".to_string(),
            );
            engine.error.insert(
                "multisource_cycle".to_string(),
                "source includes itself: {path}".to_string(),
            );
            engine.error.insert(
                "multisource_depth".to_string(),
                "source nesting is too deep: {path}".to_string(),
            );
            engine.hint.insert(
                "multisource_not_found".to_string(),
                "The source path {path} was not found.\n\nSearch paths:\n{searchstr}".to_string(),
            );
            engine.hint.insert(
                "multisource_cycle".to_string(),
                "Including {path} here would loop forever:\n\n{loop}\n\n\
                 A source may be included from more than one place, but it cannot \
                 be an ancestor of itself."
                    .to_string(),
            );
            engine.hint.insert(
                "multisource_depth".to_string(),
                "Loading {path} would nest sources more than {max} deep.\n\n\
                 Each source is loaded inside the parse that referenced it, so an \
                 unbounded chain would exhaust the stack. Raise max_depth if the \
                 chain is genuine."
                    .to_string(),
            );
        })
        .map_err(|error| MultiSourceError(error.to_string()))?;

    // Publish the live instance for nested parses, once per top-level
    // parse of a document that could hold a reference at all. A nested
    // parse carries `multisource.path` in its metadata and skips.
    {
        let options = Arc::clone(&options);
        let nested = Arc::clone(&nested);
        parser.parse_prepare_with_instance(move |instance, context, _meta| {
            if !context.source.contains(options.markchar.as_str()) {
                return;
            }
            if is_nested_meta(&context.meta) {
                return;
            }
            nested.publish(instance);
        });
    }

    let name = RULE_NAME.to_string();
    let markchar = options.markchar.clone();
    let action_options = Arc::clone(&options);
    let action_nested = Arc::clone(&nested);
    let action_reports = Arc::clone(&reports);

    let directive = DirectiveOptions::new(name.clone(), markchar)
        .with_rules(
            RulesOption::new()
                .open_rule("val", RuleMod::new())
                // A `@` in key position only opens a directive while the
                // rule is not already diving into a key, which is what
                // `pk` counts.
                .open_rule("pair", RuleMod::when(|rule, _context| rule.lte("pk", 0))),
        )
        .with_token_action(move |rule, context| {
            directive_action(
                &action_options,
                &action_nested,
                &action_reports,
                rule,
                context,
            )
        })
        .with_custom(move |parser, config| {
            if let Err(error) = install_grammar(parser, &config.name) {
                // The custom hook cannot return, so a refusal here is a
                // programming error in this crate rather than a caller
                // mistake: the document below is a constant.
                panic!("MultiSource grammar: {error}");
            }
        });

    tabnas_directive::apply(parser, directive).map_err(|error| MultiSourceError(error.to_string()))
}

/// The grammar that lets a reference stand where a value, a bare
/// top-level key, or a pair value can.
fn install_grammar(parser: &mut Tabnas, name: &str) -> Result<(), MultiSourceError> {
    let open_token = format!("#OD_{name}");
    let top_counter = format!("{name}_top");

    // Back-track the mark up a path dive only when the rule is inside a
    // dive AND the mark is not already a pair's value. In a colon chain
    // (`a: b: @"f"`) the value-position `val` has `pk > 0` but its
    // parent IS the pair for that key, so this stays false and the load
    // lands under the key instead of unwinding to depth zero.
    parser.alt_condition("@ms-pk-val", |rule, _context| {
        rule.gt("pk", 0)
            && rule
                .parent_rule
                .as_ref()
                .map(|parent| parent.name.as_ref() != "pair")
                .unwrap_or(true)
    });
    parser.alt_condition("@ms-pk", |rule, _context| rule.gt("pk", 0));
    parser.alt_condition("@ms-top", |rule, _context| rule.d == 0);
    let counter = top_counter.clone();
    parser.alt_condition("@ms-top-map", move |rule, _context| {
        rule.d == 1 && rule.eq(&counter, 1)
    });

    let document = json!({
        "rule": {
            "val": {
                "open": [
                    { "s": open_token, "c": "@ms-pk-val", "b": 1 },
                    {
                        "s": open_token,
                        "c": "@ms-top",
                        "p": "map",
                        "b": 1,
                        "n": { top_counter.clone(): 1 },
                    },
                ],
            },
            "map": {
                "open": [{
                    "s": open_token,
                    "c": "@ms-top-map",
                    "p": "pair",
                    "b": 1,
                    // Allocate the implicit top-level map here. The core
                    // only auto-allocates for `{`, so without this the
                    // pushed `pair` is seeded with an undefined node and
                    // a pair following a leading `@` has nowhere to go.
                    // The action's merge writes into this same node.
                    "a": "@object$",
                    "k": { "object$": { "implicit": true } },
                }],
                "close": [{ "s": open_token, "c": "@ms-pk", "b": 1 }],
            },
            "pair": {
                "close": [{ "s": open_token, "c": "@ms-pk", "b": 1 }],
            },
        },
    });

    let spec =
        GrammarSpec::from_value(document).map_err(|error| MultiSourceError(error.to_string()))?;
    parser
        .grammar_with_setting(&spec, &GrammarSetting::groups(name))
        .map(|_| ())
        .map_err(|error| MultiSourceError(error.to_string()))
}

// ---------------------------------------------------------------------
// The directive action
// ---------------------------------------------------------------------

/// The reference a directive body names: a bare string, or an object
/// with a `path` key (`@{path:"a.jsonic"}`).
///
/// Neither the canonical nor this port requires the value to be a
/// string, so the coercion decides which source is named and has to be
/// the canonical one: `'' + spec.path`, whose rules are JavaScript's.
/// A number goes through [`format_number`]; a boolean is `true` or
/// `false`; a null is an absent path, which is what the canonical's
/// `null != spec.path` guard makes of it.
fn reference_of(value: &Value) -> Option<String> {
    match value {
        Value::Object(_) | Value::MapRef(_) => coerce_reference(meta_get(value, "path").as_ref()?),
        other => coerce_reference(other),
    }
}

/// `'' + value` for the one value a directive body can carry.
fn coerce_reference(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Text(text) => Some(text.string.clone()),
        Value::Number(number) => Some(format_number(*number)),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// JavaScript's `Number::toString` (ECMA-262 6.1.6.1.20), which is what
/// the canonical `'' + spec.path` spells for a numeric reference, and so
/// what such a reference names.
///
/// Rust's own `f64` formatting differs from it in ways that reach a map
/// key. `number as i64` SATURATES, so every integral value above
/// `i64::MAX` became `9223372036854775807` and named the wrong source.
/// Plain `{}` keeps the sign of negative zero (`-0`, where JavaScript
/// says `0`) and never switches to exponent form, where JavaScript does
/// so at `1e21` and at `1e-7`.
///
/// The same implementation, for the same reason, is in `tabnas-csv` and
/// `tabnas-xml`; keep the three in step.
fn format_number(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_string();
    }
    // Catches -0.0 as well: JavaScript spells both zeros "0".
    if number == 0.0 {
        return "0".to_string();
    }
    if number < 0.0 {
        return format!("-{}", format_number(-number));
    }
    if number.is_infinite() {
        return "Infinity".to_string();
    }

    // The specification wants the shortest digit string `s` that
    // round-trips (length `k`), and `n`, the position of the decimal
    // point relative to it. Rust's `{:e}` yields digits of exactly that
    // shortest length.
    let shortest = format!("{number:e}");
    let shortest_k = shortest
        .split_once('e')
        .map(|(mantissa, _)| mantissa.chars().filter(char::is_ascii_digit).count())
        .expect("a finite f64 always formats with an exponent");

    // Re-render to that same length to settle a tie. Where two digit
    // strings of length `k` are equally close to `number`, the
    // specification takes the one ending in an even digit; Rust's
    // shortest form does not, but its exactly-rounded fixed-precision
    // form does.
    let exponential = format!("{:.*e}", shortest_k - 1, number);
    let (mantissa, exponent) = exponential
        .split_once('e')
        .expect("a finite f64 always formats with an exponent");
    // Rounding can leave trailing zeros (and, on a carry, one digit too
    // many); dropping them keeps `s` shortest, which is what `k` means.
    let digits = mantissa
        .chars()
        .filter(|digit| *digit != '.')
        .collect::<String>();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let k = digits.len() as i32;
    let n = exponent
        .parse::<i32>()
        .expect("a formatted exponent is an integer")
        + 1;

    // The four cases of the specification, in its order. The range
    // bounds are `k <= n <= 21`, `0 < n <= 21` and `-6 < n <= 0`.
    if (k..=21).contains(&n) {
        // Integral, with n - k trailing zeros to restore.
        let mut text = digits.to_string();
        text.push_str(&"0".repeat((n - k) as usize));
        text
    } else if (1..=21).contains(&n) {
        let point = n as usize;
        format!("{}.{}", &digits[..point], &digits[point..])
    } else if (-5..=0).contains(&n) {
        format!("0.{}{}", "0".repeat(-n as usize), digits)
    } else {
        // Exponent form. `n - 1` is never 0 here, so the sign is never
        // "+0".
        let sign = if n - 1 < 0 { '-' } else { '+' };
        let power = (n - 1).abs();
        if k == 1 {
            format!("{digits}e{sign}{power}")
        } else {
            format!("{}.{}e{sign}{power}", &digits[..1], &digits[1..])
        }
    }
}

/// The token a diagnostic is reported against: the enclosing rule's
/// opening token, as `rule.parent?.o0` names in TypeScript, then this
/// rule's own, then whatever the lexer is looking at.
fn error_token(rule: &Rule, context: &Context) -> Option<Token> {
    rule.parent_rule
        .as_ref()
        .and_then(|parent| parent.o.first())
        .or_else(|| rule.o.first())
        .cloned()
        .or_else(|| context.t0().cloned())
}

fn fail(
    reports: &LastReport,
    rule: &Rule,
    context: &Context,
    code: &str,
    details: Vec<(String, Value)>,
) -> Result<Option<Token>, ActionError> {
    // Only a NESTED parse has a ticket, and only a nested report is
    // ever taken: a top-level failure is raised to the caller whole.
    if let Some(ticket) = meta_multisource_string(&context.meta, REPORT_TICKET) {
        reports.record(&ticket, code, &details);
    }
    match error_token(rule, context) {
        Some(mut token) => {
            token.bad_with_details(code, details);
            Ok(Some(token))
        }
        // No token to hang the report on, which the engine's own error
        // path still has to be told about.
        None => Err(ActionError::new(code, format!("[{code}]"))),
    }
}

fn directive_action(
    options: &Arc<MultiSourceOptions>,
    nested: &Arc<NestedParser>,
    reports: &Arc<LastReport>,
    rule: &mut Rule,
    context: &mut Context,
) -> Result<Option<Token>, ActionError> {
    let from = rule
        .parent_rule
        .as_ref()
        .map(|parent| parent.name.to_string())
        .unwrap_or_default();

    let reference = reference_of(&rule.child_node);

    let mut resolution = {
        let input = ResolverInput {
            options,
            meta: &context.meta,
        };
        options.resolver.resolve(reference.as_deref(), &input)
    };

    if !resolution.found {
        let mut search = resolution.search.clone();
        if search.is_empty() {
            if let Some(full) = &resolution.spec.full {
                search.push(full.clone());
            }
        }
        let named = resolution
            .spec
            .path
            .clone()
            .or_else(|| reference.clone())
            .unwrap_or_default();
        return fail(
            reports,
            rule,
            context,
            "multisource_not_found",
            vec![
                ("path".to_string(), Value::String(named)),
                (
                    "full".to_string(),
                    Value::String(resolution.spec.full.clone().unwrap_or_default()),
                ),
                ("searchstr".to_string(), Value::String(search.join("\n"))),
            ],
        );
    }

    let fullpath = resolution.full_path();

    if resolution.spec.kind.is_empty() {
        resolution.spec.kind = NONE.to_string();
    }

    // Copy rather than extend in place: the inherited chain is shared
    // with every sibling include at this level, so mutating it would
    // make `parents` an accumulating visit log rather than the ancestor
    // chain, and the cycle check below would then fire on a diamond.
    let mut parents = meta_parents(&context.meta);
    if let Some(path) = meta_multisource_string(&context.meta, "path") {
        parents.push(path);
    }

    // Cycle check. Without it, a -> b -> a recurses until the stack
    // overflows, with no source position and no indication of which
    // sources are at fault. Compared against the ancestor chain, so a
    // source included from two branches is reuse, not a cycle.
    if let Some(index) = parents.iter().position(|parent| parent == &fullpath) {
        let mut loop_path: Vec<String> = parents[index..].to_vec();
        loop_path.push(fullpath.clone());
        return fail(
            reports,
            rule,
            context,
            "multisource_cycle",
            vec![
                ("path".to_string(), Value::String(fullpath)),
                ("loop".to_string(), Value::String(loop_path.join(" -> "))),
            ],
        );
    }

    // An acyclic chain can still be arbitrarily long, and each link runs
    // inside the parse that referenced it.
    if parents.len() >= options.max_depth {
        return fail(
            reports,
            rule,
            context,
            "multisource_depth",
            vec![
                ("path".to_string(), Value::String(fullpath)),
                ("max".to_string(), Value::Number(options.max_depth as f64)),
            ],
        );
    }

    if let Some(deps) = &options.deps {
        let target = parents.last().cloned().unwrap_or_else(|| TOP.to_string());
        if let Ok(mut deps) = deps.lock() {
            deps.entry(target.clone()).or_default().insert(
                fullpath.clone(),
                Dependency {
                    tar: target,
                    src: fullpath.clone(),
                    wen: now_millis(),
                },
            );
        }
    }

    let dive = rule.k.get("path").cloned();
    let ticket = reports.ticket();
    let meta = child_meta(&context.meta, &resolution, &parents, dive.as_ref(), &ticket);

    let Some(parser) = nested.get() else {
        // The prepare hook publishes the instance for every parse whose
        // source holds the mark character, and a directive cannot fire
        // on a source that does not, so this is unreachable rather than
        // a condition a document can provoke.
        return Err(ActionError::new(
            "internal",
            "MultiSource: no parser was published for the nested parse",
        ));
    };

    if let Some(processor) = options.processor_for(&resolution.spec.kind) {
        let input = ProcessorInput {
            options,
            meta: &meta,
            parser: &parser,
            depth: parents.len(),
        };
        processor.process(&mut resolution, &input);
    }

    if let Some(error) = resolution.err.take() {
        // A source resolved but failed to process, typically a nested
        // reference inside it. Re-raise the nested code rather than
        // silently substituting the raw source text.
        let code = if error.code.is_empty() {
            "unexpected".to_string()
        } else {
            error.code.clone()
        };
        // Re-raise the nested report as it was raised, so the caller
        // reads the loop of a cycle or the search paths of a missing
        // source rather than a summary of them, exactly as the
        // canonical TypeScript exception carries them out. The ticket
        // is what makes it THIS load's report and no other parse's.
        let details = reports.take(&ticket, &code).unwrap_or_else(|| {
            vec![
                ("path".to_string(), Value::String(fullpath)),
                ("searchstr".to_string(), Value::String(error.detail.clone())),
            ]
        });
        return fail(reports, rule, context, &code, details);
    }

    // The load succeeded, so nothing will ever ask for its slot.
    reports.discard(&ticket);

    let value = std::mem::replace(&mut resolution.val, Value::Undefined);

    if from == "pair" {
        splice_into_parent(rule, context, value);
    } else {
        set_node(rule, value);
    }

    Ok(None)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0)
}

/// Merge a loaded map into the enclosing map, for the `{@foo}` form.
///
/// TypeScript assigns the merged object back to `rule.parent.parent.node`
/// and its `deep` mutates the base in place, so the enclosing map keeps
/// its identity and a pair that follows the directive writes into the
/// same node. A Rust rule reaches its grandparent only through a
/// snapshot, which cannot be assigned to, so the merge is written
/// through the shared node cell instead. The cell is the same one, so
/// the identity the following pair depends on is preserved.
fn splice_into_parent(rule: &mut Rule, context: &mut Context, value: Value) {
    let Some(target) = rule
        .parent_rule
        .as_ref()
        .and_then(|parent| parent.parent_rule.as_ref())
        .map(|grandparent| grandparent.node.clone())
    else {
        return;
    };

    let merge = context.options.map.merge.clone();
    let extend = context.options.map.extend;

    if let Some(merge) = merge {
        let base = target.borrow().clone();
        let merged = merge(base, value, rule, context);
        *target.borrow_mut() = merged;
        return;
    }

    if extend {
        let base = target.borrow().clone();
        let merged = tabnas_jsonic::deep_merge(base, value);
        *target.borrow_mut() = merged;
        return;
    }

    // `Object.assign`: a shallow per-key overwrite.
    let Some(entries) = object_entries(&value) else {
        return;
    };
    let mut node = target.borrow_mut();
    match &mut *node {
        Value::Object(map) => {
            let map = Arc::make_mut(map);
            for (key, item) in entries {
                map.insert(key, item);
            }
        }
        Value::MapRef(map) => {
            let map = Arc::make_mut(map);
            for (key, item) in entries {
                map.value.insert(key, item);
            }
        }
        _ => {}
    }
}

fn object_entries(value: &Value) -> Option<Vec<(String, Value)>> {
    match value {
        Value::Object(map) => Some(
            map.iter()
                .map(|(key, item)| (key.clone(), item.clone()))
                .collect(),
        ),
        Value::MapRef(map) => Some(
            map.value
                .iter()
                .map(|(key, item)| (key.clone(), item.clone()))
                .collect(),
        ),
        _ => None,
    }
}

// ---------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------

/// A jsonic parser with the plugin installed over `options`.
pub fn make_with(options: MultiSourceOptions) -> Tabnas {
    let mut parser = tabnas_jsonic::make();
    multisource(&mut parser, options).expect("the MultiSource plugin installs on jsonic");
    parser
}

/// A jsonic parser with the plugin installed over default options: no
/// sources, so every reference raises `multisource_not_found`.
pub fn make() -> Tabnas {
    make_with(MultiSourceOptions::default())
}

/// Parse `src` with `options`.
///
/// Every call builds an instance. Building the grammar dominates a
/// parse, so a loop should call [`make_with`] once and reuse the
/// instance; `tests/perf_test.rs` measures the difference.
pub fn parse(src: &str, options: MultiSourceOptions) -> Result<Value, TabnasError> {
    make_with(options).parse(src)
}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}
