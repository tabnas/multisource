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

## Constants

```go
const Version = "0.1.4"   // Go module release version
const NONE = ""           // the unknown/empty kind (default-processor key)
```

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
}
```

| Field | Type | Default | Purpose |
| --- | --- | --- | --- |
| `Resolver` | `Resolver` | empty mem resolver | Resolves a `PathSpec` to source. |
| `Path` | `string` | `""` | Base path prefixed to relative references. |
| `MarkChar` | `string` | `"@"` | Single character that opens a reference. |
| `Processor` | `map[string]Processor` | see below | Per-kind source transformers. |
| `ImplicitExt` | `[]string` | `[".jsonic", ".jsc", ".json"]` | Extensions tried when a reference has none. Normalised to begin with `.`. |

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
type Resolver func(spec PathSpec, opts *MultiSourceOptions) Resolution
```

### `MakeMemResolver`

```go
func MakeMemResolver(files map[string]string) Resolver
```

Resolves references against an in-memory `path → content` map. Tries implicit
extensions and `index` files when the reference has no extension (via
`buildPotentials`), and records the searched paths in `Resolution.Search`.

## Processors

```go
type Processor func(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic)
```

A processor reads `res.Src` and assigns `res.Val`. The `j` argument is the
engine, available for re-parsing.

| Function | Kind | Behaviour |
| --- | --- | --- |
| `DefaultProcessor` | `NONE` | `res.Val = res.Src` (raw string). |
| `JSONProcessor` | `json` | `encoding/json` unmarshal; falls back to the raw string on error; `nil` on empty source. |
| `JsonicProcessor` | `jsonic`, `jsc` | Re-parses `res.Src` through the engine; falls back to the raw string on error; `nil` on empty source. |

```go
func DefaultProcessor(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic)
func JSONProcessor(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic)
func JsonicProcessor(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic)
```

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
