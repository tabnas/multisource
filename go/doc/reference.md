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
    jsonic "github.com/tabnas/jsonic/go"
)
```

## Constants and package metadata

```go
const Version = "0.3.1"   // Go module release version
const NONE = ""           // the unknown/empty kind (default-processor key)
const TOP = "\x00TOP"     // dependency-tree top marker (never a valid path)

var Meta = PluginMeta{Name: "MultiSource"}  // plugin metadata (TS: `meta`)
```

`TOP` is the `DependencyMap` target key used for sources referenced directly
by the top-level parse. It is the Go counterpart of the TypeScript `TOP`
symbol; the NUL byte guarantees it cannot collide with a real source path.

## Constructors

### `MakeJsonic`

```go
func MakeJsonic(opts ...MultiSourceOptions) *jsonic.Jsonic
```

Creates a `*jsonic.Jsonic` with the `MultiSource` plugin installed and
defaults applied. Pass zero or one `MultiSourceOptions`. The returned instance
is reusable; call `.Parse(src)` on it.

### `Parse`

```go
func Parse(src string, opts ...MultiSourceOptions) (any, error)
```

Convenience wrapper. With no options it reuses a cached default parser (safe
for concurrent use); with options it builds a fresh instance per call.

### `MultiSource`

```go
func MultiSource(j *jsonic.Jsonic, pluginOpts map[string]any) error
```

The raw plugin function, applied by `MakeJsonic`. Options are passed under the
`"_opts"` key as a `*MultiSourceOptions`. Most callers use `MakeJsonic`
instead of calling this directly.

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
| `Preload` | `*PreloadOptions` | `nil` | Folder-scanning preload configuration. As in TS, not consumed by the plugin directly — pass it to `PreloadFiles` and feed the result to `FileResolverOptions.Preload`. |
| `FS` | `fs.FS` | `nil` (OS) | Filesystem for the file/pkg resolvers. A per-parse override may be passed as `ctx.Meta["fs"]`. |

### Default processors

```go
map[string]Processor{
    NONE:     DefaultProcessor,   // ""  raw string passthrough
    "json":   JSONProcessor,
    "jsonic": JsonicProcessor,
    "jsc":    JsonicProcessor,
}
```

## Resolvers

```go
type Resolver func(spec PathSpec, opts *MultiSourceOptions, ctx *jsonic.Context) Resolution
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
`ctx.Meta["fs"]` when injected). The `Preload` map — typically built by
`PreloadFiles` — is consulted before any file I/O.

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
type Processor func(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic)
```

A processor reads `res.Src` and assigns `res.Val`. The `ctx` carries the parse
metadata for this load (`ctx.Meta`); the `j` argument is the engine, available
for re-parsing.

| Function | Kind | Behaviour |
| --- | --- | --- |
| `DefaultProcessor` | `NONE` | `res.Val = res.Src` (raw string). |
| `JSONProcessor` | `json` | `encoding/json` unmarshal; falls back to the raw string on error; `nil` on empty source. |
| `JsonicProcessor` | `jsonic`, `jsc` | Re-parses `res.Src` through the engine; falls back to the raw string on error; `nil` on empty source. |

```go
func DefaultProcessor(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic)
func JSONProcessor(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic)
func JsonicProcessor(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic)
```

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

Pass parse metadata with `Jsonic.ParseMeta(src, meta)`. The plugin honours a
`"multisource"` entry (a `map[string]any`), mirroring the TypeScript
`MultiSourceMeta`:

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

- `@a.jsonic` — a bare or quoted path string.
- `@{path:"a.jsonic"}` — an object with a `path` key (the action reads
  `spec["path"]`).

Placement determines splicing:

- as a pair value, `x: @a.jsonic`, the value nests under the key.
- alone in a map, `{@a.jsonic, c:3}`, the referenced map's keys are merged
  into the parent (deep merge; respects `cfg.MapMerge` / `cfg.MapExtend`).
- at the top level, `@a.jsonic`, the result is the referenced map, with any
  following pairs merged in.

## Behaviour notes

- A reference whose resolver returns `Found: false` yields `nil` for that
  value — it is not an error.
- Numbers parse to `float64` (the jsonic engine default), as in all jsonic Go
  output.
- `MakeJsonic` adds the mark character to the engine's ender chars so built-in
  matchers stop at it.
