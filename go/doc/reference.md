# Reference (Go)

Complete API surface of the `github.com/tabnas/multisource/go` package. The
package identifier is `tabnasmultisource`.

## Install

```sh
go get github.com/tabnas/multisource/go
```

```go
import (
    tabnasmultisource "github.com/tabnas/multisource/go"
    tabnas "github.com/tabnas/parser/go"
)
```

`tabnas` is the parser engine. Its `*tabnas.Tabnas` and `*tabnas.Context`
types appear in the plugin, resolver, and processor signatures. The package
does not build a parser itself: you build the host parser, for example with
`jsonic.Make()` from `github.com/tabnas/jsonic/go`, and install the plugin on
it.

## Constants and package metadata

```go
const VERSION = "x.y.z"   // module version; must equal ts/package.json
const NONE = ""           // the unknown/empty kind (default-processor key)
const TOP = "\x00TOP"     // dependency-tree top marker (never a valid path)

var Meta = PluginMeta{Name: "MultiSource"}  // plugin metadata (TS: `meta`)
```

`TOP` is the `DependencyMap` target key used for sources referenced directly
by the top-level parse. It is the Go counterpart of the TypeScript `TOP`
symbol; the NUL byte guarantees it cannot collide with a real source path.

## The plugin

### `MultiSource`

```go
func MultiSource(j *tabnas.Tabnas, pluginOpts map[string]any) error
```

The plugin. Install it on a host parser you have built, with its options:

```go
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})
out, err := j.Parse(src)
```

The instance is reusable; call `.Parse(src)` or `.ParseMeta(src, meta)` on it.

## Plugin options

The plugin takes its options as plain keys, mirroring the TypeScript plugin
options, or as one typed `*MultiSourceOptions` under the `"_opts"` key.

| Key | Type | Effect |
| --- | --- | --- |
| `resolver` | `Resolver` | Sets `Resolver`. The value must have the `Resolver` type; the plugin ignores a plain function value. |
| `path` | `string` | Sets `Path`. |
| `markchar`, `markChar` | `string` | Sets `MarkChar`. |
| `processor` | `map[string]Processor` | Adds or replaces entries in the default processor map. |
| `implictExt`, `implicitExt` | `[]string` or `[]any` | Replaces `ImplicitExt`, adding a missing leading `.`. |
| `fs` | `fs.FS` | Sets `FS`. |
| `_opts` | `*MultiSourceOptions` | All options as a struct. Its unset `MarkChar`, `Processor`, `ImplicitExt`, and `Resolver` fields take the defaults, and its `Processor` map replaces the default map. When `_opts` is present the plugin ignores the plain keys. |

Fields not set by any key take the defaults in the table below.

## `MultiSourceOptions`

```go
type MultiSourceOptions struct {
    Resolver    Resolver
    Path        string
    MarkChar    string
    Processor   map[string]Processor
    ImplicitExt []string
    Preload     *PreloadOptions
    FS          fs.FS
}
```

| Field | Type | Default | Purpose |
| --- | --- | --- | --- |
| `Resolver` | `Resolver` | empty mem resolver | Resolves a `PathSpec` to source. |
| `Path` | `string` | `""` | Base path prefixed to relative references. |
| `MarkChar` | `string` | `"@"` | Single character that opens a reference. |
| `Processor` | `map[string]Processor` | see below | Per-kind source transformers. |
| `ImplicitExt` | `[]string` | `[".jsonic", ".jsc", ".json"]` | Extensions tried when a reference has none. Normalised to begin with `.`. |
| `Preload` | `*PreloadOptions` | `nil` | Folder-scanning preload configuration. As in TS, not consumed by the plugin directly; pass it to `PreloadFiles` and feed the result to `FileResolverOptions.Preload`. |
| `FS` | `fs.FS` | `nil` (OS) | Filesystem for the file/pkg resolvers. A per-parse override may be passed as `ctx.Meta["fs"]`. |

### Default processors

```go
map[string]Processor{
    NONE:     DefaultProcessor,   // ""  raw string passthrough
    "jsonic": JsonicProcessor,
    "jsc":    JsonicProcessor,
}
```

There is no `json` entry. `NONE` handles a `.json` source, so its value is the
raw text, until you register a processor for the `json` kind. The
[how-to guide](guide.md#load-json-sources) has one built on `encoding/json`.

## Resolvers

```go
type Resolver func(spec PathSpec, opts *MultiSourceOptions, ctx *tabnas.Context) Resolution
```

The `ctx` carries the parse metadata (`ctx.Meta`); resolvers may read
`ctx.Meta["fs"]` for a per-parse filesystem override.

### `MakeMemResolver`

```go
func MakeMemResolver(files map[string]string) Resolver
```

Resolves references against an in-memory `path → content` map. Tries implicit
extensions and `index` files when the reference has no extension (via
`buildPotentials`), and records the searched paths in `Resolution.Search`.

### `MakeFileResolver`

```go
type FileResolverOptions struct {
    PathFinder func(spec string) string // transform the raw reference path
    Preload    map[string]string        // full path -> content, checked before disk
}

func MakeFileResolver(opts ...FileResolverOptions) Resolver
```

Loads sources from the filesystem (OS by default; `MultiSourceOptions.FS` or
`ctx.Meta["fs"]` when injected). The `Preload` map (typically built by
`PreloadFiles`) is consulted before any file I/O.

### `MakePkgResolver`

```go
type PkgResolverOptions struct {
    Paths []string // directories whose node_modules are searched (walked upwards)
}

func MakePkgResolver(opts ...PkgResolverOptions) Resolver
```

Resolves references inside `node_modules` folders, honouring a package's
`package.json` `"main"` and implicit extensions/index files. Implements the
portable subset of Node resolution (no conditional `exports`).

## Preload

```go
type PreloadOptions struct {
    Folders   []string // folders to scan (non-recursive by default)
    Ext       []string // extensions to load (default: ".jsonic", ".json")
    Recursive bool     // recurse into subfolders (default: false)
}

func PreloadFiles(opts PreloadOptions, fsys ...fs.FS) map[string]string
```

Scans the folders for files matching the extensions (a missing leading `.` is
added) and returns a flat `full path → content` map, mirroring the TypeScript
`preloadFiles`. Missing folders and unreadable files are silently skipped. By
default files are read from the OS and keyed by absolute path (matching
`MakeFileResolver` lookups); pass an `io/fs.FS` to read from it instead, with
relative slash-separated keys. Feed the result to
`FileResolverOptions.Preload` to avoid per-file I/O during parse.

## Processors

```go
type Processor func(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas)
```

A processor reads `res.Src` and assigns `res.Val`. The `ctx` carries the parse
metadata for this load (`ctx.Meta`); the `j` argument is the host parser,
available for re-parsing. A processor that sets `res.Err` fails the enclosing
parse.

| Function | Kind | Behaviour |
| --- | --- | --- |
| `DefaultProcessor` | `NONE` | `res.Val = res.Src` (raw string). |
| `JsonicProcessor` | `jsonic`, `jsc` | Re-parses `res.Src` through the host parser; `nil` on empty source. A parse failure inside the source (for example a nested `@missing`) fails the parse, reported through `Resolution.Err`. |

On that failure `res.Val` still holds the raw source text, for callers that
invoke the processor directly and inspect the `Resolution`; it is `res.Err`
that makes the enclosing parse fail.

```go
func DefaultProcessor(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas)
func JsonicProcessor(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas)
```

There is no `json` processor. Register your own for the `json` kind through
the `processor` option.

There is no `js` processor: Go cannot execute a JavaScript module, so `.js`
sources are unsupported (see the
[differences section](concepts.md#differences-from-the-typescript-implementation)).

`getProcessor` selects `Processor[kind]`, falling back to `Processor[NONE]`,
then `DefaultProcessor`.

## Path utilities

### `ResolvePathSpec`

```go
func ResolvePathSpec(specPath string, base string) PathSpec
```

Normalises a reference string into a `PathSpec`: detects absolute paths
(leading `/` or `\`), joins `base`, and extracts `Kind` from the extension.

## Types

```go
type PathSpec struct {
    Kind string // source kind (extension without the dot), or ""
    Path string // original (possibly relative) path
    Full string // normalised full path
    Base string // current base path
    Abs  bool   // true if the path was absolute
}

type Resolution struct {
    PathSpec
    Src    string   // loaded source content
    Val    any      // processed value
    Found  bool     // true if a source was found
    Search []string // paths the resolver tried
}

type Dependency struct {
    Tar string `json:"tar"` // target that depends on Src; TOP at the top level
    Src string `json:"src"` // source that Tar depends on
    Wen int64  `json:"wen"` // time of resolution (Unix milliseconds)
}

// Flattened dependency tree: target full path -> source full path -> record.
type DependencyMap map[string]map[string]Dependency

type PluginMeta struct {
    Name string
}
```

## Dependency tracking and parse meta

Pass parse metadata with `j.ParseMeta(src, meta)` on the host parser. The
plugin honours a `"multisource"` entry (a `map[string]any`), mirroring the
TypeScript `MultiSourceMeta`:

| Key | Type | Purpose |
| --- | --- | --- |
| `path` | `string` | Base path for this parse run (full path of the enclosing source for nested loads). |
| `parents` | `[]string` | Enclosing source paths, maintained by the plugin. |
| `deps` | `DependencyMap` | Pass an empty map to be filled with the dependency tree. |

A per-parse filesystem override may be passed as `meta["fs"]` (`fs.FS`).

```go
deps := tabnasmultisource.DependencyMap{}
j.ParseMeta(`@"app.jsonic"`, map[string]any{
    "multisource": map[string]any{"deps": deps},
})
// deps now maps each source's full path (or TOP for the top level) to the
// sources it pulled in, with resolution timestamps.
```

## Reference syntax

In parsed input, a reference is the mark character followed by a path:

- `@a.jsonic`. A bare or quoted path string.
- `@{path:"a.jsonic"}`. An object with a `path` key (the action reads
  `spec["path"]`).

Placement determines splicing:

- as a pair value, `x: @a.jsonic`, the value nests under the key.
- alone in a map, `{@a.jsonic, c:3}`, the referenced map's keys are merged
  into the parent (deep merge; respects `cfg.MapMerge` / `cfg.MapExtend`).
- at the top level, `@a.jsonic`, the result is the referenced map, with any
  following pairs merged in.

## Behaviour notes

- A reference whose resolver returns `Found: false` fails the parse with
  `multisource_not_found` ("source not found: {path}"), listing the searched
  paths, the same contract as the TypeScript plugin.
- A source that is an ancestor of itself fails the parse with
  `multisource_cycle` ("source includes itself: {path}"), naming the loop.
  Including one source from two different branches (a diamond) is reuse, not a
  cycle, and is allowed.
- Numbers parse to `float64` (the jsonic engine default), as in all jsonic Go
  output.
- The plugin adds the mark character to the ender chars of the host parser,
  so built-in matchers stop at it.
